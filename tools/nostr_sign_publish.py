#!/usr/bin/env python3
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import socket
import ssl
import struct
import time
from contextlib import closing
from typing import Any
from urllib.parse import urlparse

from trezorlib import messages, nostr
from trezorlib.client import get_default_client, get_default_session
from trezorlib.tools import parse_path


PATH_TEMPLATE = "m/44h/1237h/{}h/0/0"
USER_AGENT = "trezor-nostr-publisher/1"


def _read_exact(sock: socket.socket, size: int) -> bytes:
    chunks: list[bytes] = []
    remaining = size
    while remaining > 0:
        chunk = sock.recv(remaining)
        if not chunk:
            raise RuntimeError("unexpected EOF from relay")
        chunks.append(chunk)
        remaining -= len(chunk)
    return b"".join(chunks)


def _read_http_response(sock: socket.socket) -> tuple[str, dict[str, str]]:
    data = b""
    while b"\r\n\r\n" not in data:
        chunk = sock.recv(4096)
        if not chunk:
            raise RuntimeError("relay closed during websocket handshake")
        data += chunk

    head, _ = data.split(b"\r\n\r\n", 1)
    lines = head.decode("utf-8").split("\r\n")
    status = lines[0]
    headers: dict[str, str] = {}
    for line in lines[1:]:
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        headers[key.strip().lower()] = value.strip()
    return status, headers


def _ws_send_text(sock: socket.socket, text: str) -> None:
    payload = text.encode("utf-8")
    mask = os.urandom(4)
    frame = bytearray()
    frame.append(0x81)

    length = len(payload)
    if length < 126:
        frame.append(0x80 | length)
    elif length < 65536:
        frame.append(0x80 | 126)
        frame.extend(struct.pack("!H", length))
    else:
        frame.append(0x80 | 127)
        frame.extend(struct.pack("!Q", length))

    frame.extend(mask)
    frame.extend(payload[i] ^ mask[i % 4] for i in range(length))
    sock.sendall(frame)


def _ws_send_pong(sock: socket.socket, payload: bytes) -> None:
    frame = bytearray([0x8A])
    length = len(payload)
    if length < 126:
        frame.append(0x80 | length)
    elif length < 65536:
        frame.append(0x80 | 126)
        frame.extend(struct.pack("!H", length))
    else:
        frame.append(0x80 | 127)
        frame.extend(struct.pack("!Q", length))

    mask = os.urandom(4)
    frame.extend(mask)
    frame.extend(payload[i] ^ mask[i % 4] for i in range(length))
    sock.sendall(frame)


def _ws_recv_text(sock: socket.socket) -> str:
    while True:
        first, second = _read_exact(sock, 2)
        opcode = first & 0x0F
        masked = (second & 0x80) != 0
        length = second & 0x7F

        if length == 126:
            length = struct.unpack("!H", _read_exact(sock, 2))[0]
        elif length == 127:
            length = struct.unpack("!Q", _read_exact(sock, 8))[0]

        mask = _read_exact(sock, 4) if masked else b""
        payload = _read_exact(sock, length)

        if masked:
            payload = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))

        if opcode == 0x1:
            return payload.decode("utf-8")
        if opcode == 0x8:
            raise RuntimeError("relay closed websocket")
        if opcode == 0x9:
            _ws_send_pong(sock, payload)
            continue


def publish_event(relay_url: str, event: dict[str, Any], timeout: float) -> str:
    parsed = urlparse(relay_url)
    if parsed.scheme not in ("ws", "wss"):
        raise ValueError("relay URL must start with ws:// or wss://")

    host = parsed.hostname
    if host is None:
        raise ValueError("relay URL is missing host")

    port = parsed.port or (443 if parsed.scheme == "wss" else 80)
    path = parsed.path or "/"
    if parsed.query:
        path += f"?{parsed.query}"

    raw_sock = socket.create_connection((host, port), timeout=timeout)
    with closing(raw_sock):
        sock: socket.socket
        if parsed.scheme == "wss":
            context = ssl.create_default_context()
            sock = context.wrap_socket(raw_sock, server_hostname=host)
        else:
            sock = raw_sock

        with closing(sock):
            key = base64.b64encode(os.urandom(16)).decode("ascii")
            request = (
                f"GET {path} HTTP/1.1\r\n"
                f"Host: {host}:{port}\r\n"
                "Upgrade: websocket\r\n"
                "Connection: Upgrade\r\n"
                f"Sec-WebSocket-Key: {key}\r\n"
                "Sec-WebSocket-Version: 13\r\n"
                f"User-Agent: {USER_AGENT}\r\n"
                "\r\n"
            )
            sock.sendall(request.encode("utf-8"))

            status, headers = _read_http_response(sock)
            if "101" not in status:
                raise RuntimeError(f"websocket handshake failed: {status}")

            accept_expected = base64.b64encode(
                hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()
            ).decode("ascii")
            if headers.get("sec-websocket-accept") != accept_expected:
                raise RuntimeError("relay websocket accept header mismatch")

            _ws_send_text(sock, json.dumps(["EVENT", event], separators=(",", ":")))
            return _ws_recv_text(sock)


def sign_note(content: str, account: int, tags: list[list[str]]) -> dict[str, Any]:
    client = get_default_client("trezor-nostr-publisher")
    session = None
    try:
        session = get_default_session(client)
        address_n = parse_path(PATH_TEMPLATE.format(account))
        unsigned = messages.NostrSignEvent(
            address_n=address_n,
            created_at=int(time.time()),
            kind=1,
            tags=[
                messages.NostrTag(
                    key=tag[0],
                    value=tag[1] if len(tag) > 1 else None,
                    extra=tag[2:],
                )
                for tag in tags
            ],
            content=content,
        )
        signed = nostr.sign_event(session, unsigned)
        return {
            "id": signed.id.hex(),
            "pubkey": signed.pubkey.hex(),
            "created_at": unsigned.created_at,
            "kind": 1,
            "tags": tags,
            "content": content,
            "sig": signed.signature.hex(),
        }
    finally:
        if session is not None:
            session.close()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Sign a kind 1 Nostr note with Trezor and publish it to a relay."
    )
    parser.add_argument("content", help="note content")
    parser.add_argument("--relay", required=True, help="relay websocket URL, e.g. wss://relay.damus.io")
    parser.add_argument("--account", type=int, default=0, help="Nostr account index, default: 0")
    parser.add_argument(
        "--tag",
        action="append",
        default=[],
        help='JSON array tag, repeatable, e.g. --tag \'["t","nostr"]\'',
    )
    parser.add_argument("--timeout", type=float, default=10.0, help="relay timeout in seconds")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    tags = [json.loads(tag) for tag in args.tag]
    for tag in tags:
        if not isinstance(tag, list) or not tag or not all(isinstance(item, str) for item in tag):
            raise SystemExit(f"invalid tag: {tag!r}")

    event = sign_note(args.content, args.account, tags)
    response = publish_event(args.relay, event, args.timeout)
    print(json.dumps({"event": event, "relay_response": json.loads(response)}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
