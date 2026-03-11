/*
 * Nostr handlers for legacy firmware (T1B1)
 */

#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#include "gettext.h"
#include "layout2.h"
#include "messages.h"
#include "messages.pb.h"
#include "protect.h"
#include "sha2.h"
#include "util.h"
#include "zkp_bip340.h"

#define NOSTR_JSON_MAX 4096

static bool nostr_json_append_char(char *dst, size_t dst_size, size_t *pos,
                                   char c) {
  if (*pos + 1 >= dst_size) return false;
  dst[*pos] = c;
  *pos += 1;
  dst[*pos] = '\0';
  return true;
}

static bool nostr_json_append_str(char *dst, size_t dst_size, size_t *pos,
                                  const char *s) {
  while (*s) {
    if (!nostr_json_append_char(dst, dst_size, pos, *s++)) return false;
  }
  return true;
}

static bool nostr_json_append_u32(char *dst, size_t dst_size, size_t *pos,
                                  uint32_t v) {
  char tmp[16];
  mini_snprintf(tmp, sizeof(tmp), "%lu", (unsigned long)v);
  return nostr_json_append_str(dst, dst_size, pos, tmp);
}

static bool nostr_json_append_hex(char *dst, size_t dst_size, size_t *pos,
                                  const uint8_t *bytes, size_t n) {
  static const char *HEX = "0123456789abcdef";
  for (size_t i = 0; i < n; i++) {
    if (!nostr_json_append_char(dst, dst_size, pos, HEX[bytes[i] >> 4]))
      return false;
    if (!nostr_json_append_char(dst, dst_size, pos, HEX[bytes[i] & 0x0f]))
      return false;
  }
  return true;
}

static bool nostr_json_append_escaped(char *dst, size_t dst_size, size_t *pos,
                                      const char *s) {
  while (*s) {
    unsigned char c = (unsigned char)*s++;
    switch (c) {
      case '"':
        if (!nostr_json_append_str(dst, dst_size, pos, "\\\"")) return false;
        break;
      case '\\':
        if (!nostr_json_append_str(dst, dst_size, pos, "\\\\")) return false;
        break;
      case '\b':
        if (!nostr_json_append_str(dst, dst_size, pos, "\\b")) return false;
        break;
      case '\f':
        if (!nostr_json_append_str(dst, dst_size, pos, "\\f")) return false;
        break;
      case '\n':
        if (!nostr_json_append_str(dst, dst_size, pos, "\\n")) return false;
        break;
      case '\r':
        if (!nostr_json_append_str(dst, dst_size, pos, "\\r")) return false;
        break;
      case '\t':
        if (!nostr_json_append_str(dst, dst_size, pos, "\\t")) return false;
        break;
      default:
        if (c < 0x20) {
          char esc[7];
          mini_snprintf(esc, sizeof(esc), "\\u%04x", c);
          if (!nostr_json_append_str(dst, dst_size, pos, esc)) return false;
        } else {
          if (!nostr_json_append_char(dst, dst_size, pos, (char)c)) return false;
        }
    }
  }
  return true;
}

void fsm_msgNostrGetPubkey(const NostrGetPubkey *msg) {
  RESP_INIT(NostrPubkey);

  CHECK_INITIALIZED
  CHECK_PIN

  const HDNode *node =
      fsm_getDerivedNode(SECP256K1_NAME, msg->address_n, msg->address_n_count, NULL);
  if (!node) return;

  if (zkp_bip340_get_public_key(node->private_key, resp->pubkey.bytes) != 0) {
    fsm_sendFailure(FailureType_Failure_ProcessError, _("Failed to derive pubkey"));
    layoutHome();
    return;
  }
  resp->pubkey.size = 32;

  msg_write(MessageType_MessageType_NostrPubkey, resp);
  layoutHome();
}

void fsm_msgNostrSignEvent(const NostrSignEvent *msg) {
  RESP_INIT(NostrEventSignature);

  CHECK_INITIALIZED
  CHECK_PIN

  const HDNode *node =
      fsm_getDerivedNode(SECP256K1_NAME, msg->address_n, msg->address_n_count, NULL);
  if (!node) return;

  layoutDialogSwipe(&bmp_icon_question, _("Cancel"), _("Confirm"), NULL,
                    _("Sign Nostr event"), _("on this device?"), NULL,
                    _("Kind and content"), _("will be signed"), NULL);
  if (!protectButton(ButtonRequestType_ButtonRequest_ProtectCall, false)) {
    fsm_sendFailure(FailureType_Failure_ActionCancelled, NULL);
    layoutHome();
    return;
  }

  uint8_t pub32[32];
  if (zkp_bip340_get_public_key(node->private_key, pub32) != 0) {
    fsm_sendFailure(FailureType_Failure_ProcessError, _("Failed to derive pubkey"));
    layoutHome();
    return;
  }

  char ev[NOSTR_JSON_MAX];
  size_t p = 0;
  ev[0] = '\0';

  bool ok = true;
  ok &= nostr_json_append_char(ev, sizeof(ev), &p, '[');
  ok &= nostr_json_append_char(ev, sizeof(ev), &p, '0');
  ok &= nostr_json_append_char(ev, sizeof(ev), &p, ',');
  ok &= nostr_json_append_char(ev, sizeof(ev), &p, '"');
  ok &= nostr_json_append_hex(ev, sizeof(ev), &p, pub32, 32);
  ok &= nostr_json_append_char(ev, sizeof(ev), &p, '"');
  ok &= nostr_json_append_char(ev, sizeof(ev), &p, ',');
  ok &= nostr_json_append_u32(ev, sizeof(ev), &p, msg->created_at);
  ok &= nostr_json_append_char(ev, sizeof(ev), &p, ',');
  ok &= nostr_json_append_u32(ev, sizeof(ev), &p, msg->kind);
  ok &= nostr_json_append_char(ev, sizeof(ev), &p, ',');
  ok &= nostr_json_append_char(ev, sizeof(ev), &p, '[');

  for (size_t i = 0; ok && i < msg->tags_count; i++) {
    const NostrTag *tag = &msg->tags[i];
    if (i) ok &= nostr_json_append_char(ev, sizeof(ev), &p, ',');
    ok &= nostr_json_append_char(ev, sizeof(ev), &p, '[');

    ok &= nostr_json_append_char(ev, sizeof(ev), &p, '"');
    ok &= nostr_json_append_escaped(ev, sizeof(ev), &p, tag->key);
    ok &= nostr_json_append_char(ev, sizeof(ev), &p, '"');

    if (tag->has_value) {
      ok &= nostr_json_append_char(ev, sizeof(ev), &p, ',');
      ok &= nostr_json_append_char(ev, sizeof(ev), &p, '"');
      ok &= nostr_json_append_escaped(ev, sizeof(ev), &p, tag->value);
      ok &= nostr_json_append_char(ev, sizeof(ev), &p, '"');
    }

    for (size_t j = 0; j < tag->extra_count; j++) {
      ok &= nostr_json_append_char(ev, sizeof(ev), &p, ',');
      ok &= nostr_json_append_char(ev, sizeof(ev), &p, '"');
      ok &= nostr_json_append_escaped(ev, sizeof(ev), &p, tag->extra[j]);
      ok &= nostr_json_append_char(ev, sizeof(ev), &p, '"');
    }

    ok &= nostr_json_append_char(ev, sizeof(ev), &p, ']');
  }

  ok &= nostr_json_append_char(ev, sizeof(ev), &p, ']');
  ok &= nostr_json_append_char(ev, sizeof(ev), &p, ',');
  ok &= nostr_json_append_char(ev, sizeof(ev), &p, '"');
  ok &= nostr_json_append_escaped(ev, sizeof(ev), &p, msg->content);
  ok &= nostr_json_append_char(ev, sizeof(ev), &p, '"');
  ok &= nostr_json_append_char(ev, sizeof(ev), &p, ']');

  if (!ok) {
    fsm_sendFailure(FailureType_Failure_DataError, _("Event too large"));
    layoutHome();
    return;
  }

  uint8_t event_id[32];
  sha256_Raw((const uint8_t *)ev, p, event_id);

  uint8_t sig64[64];
  uint8_t aux[32] = {0};
  if (zkp_bip340_sign_digest(node->private_key, event_id, sig64, aux) != 0) {
    fsm_sendFailure(FailureType_Failure_ProcessError, _("Failed to sign event"));
    layoutHome();
    return;
  }

  memcpy(resp->pubkey.bytes, pub32, 32);
  resp->pubkey.size = 32;
  memcpy(resp->id.bytes, event_id, 32);
  resp->id.size = 32;
  memcpy(resp->signature.bytes, sig64, 64);
  resp->signature.size = 64;

  msg_write(MessageType_MessageType_NostrEventSignature, resp);
  layoutHome();
}
