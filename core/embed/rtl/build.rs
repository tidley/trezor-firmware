use std::path::PathBuf;

fn main() {
    let mut lib = cbuild::CLibrary::new();

    lib.use_lib("models");

    lib.add_public_include("inc");

    lib.add_public_define("SCM_REVISION_INIT",
        Some("{0x7b,0xeb,0x0e,0xca,0x3e,0x76,0xcb,0x5c,0xd0,0x05,0xaf,0xce,0x9b,0xcb,0x05,0xcc,0x86,0xe0,0x25,0x19,}")
    ); // !@# TODO

    lib.add_sources([
        "cli.c",
        "error_handling.c",
        "printf.c",
        "scm_revision.c",
        "strutils.c",
        "unit_test.c",
    ]);

    add_crypto(&mut lib);

    add_uzlib(&mut lib);

    lib.build();
}

fn add_crypto(lib: &mut cbuild::CLibrary) {
    let crypto_path = "../../vendor/trezor-crypto";

    lib.add_public_include(crypto_path);

    lib.add_public_define("ED25519_NO_PRECOMP", None); // TODO!@# conditional??
    lib.add_public_define("USE_BIP32_CACHE", Some("0")); // TODO!@# conditional??

    lib.add_public_defines([
        ("AES_128", None),
        ("AES_192", None),
        ("AES_GCM", None), // TODO!@# conditional
        ("AES_VAR", None), // TODO!@# conditional
        ("USE_KECCAK", Some("1")),
    ]);

    lib.add_sources_from_folder(
        crypto_path,
        [
            "address.c",
            "aes/aes_modes.c",
            "aes/aesccm.c",
            "aes/aescrypt.c",
            "aes/aeskey.c",
            "aes/aestab.c",
            "base32.c",
            "base58.c",
            "bignum.c",
            "bip32.c",
            "bip39.c",
            "bip39_english.c",
            "blake256.c",
            "blake2b.c",
            "blake2s.c",
            "buffer.c",
            "chacha20poly1305/chacha20poly1305.c",
            "chacha20poly1305/chacha_merged.c",
            "chacha20poly1305/poly1305-donna.c",
            "chacha20poly1305/rfc7539.c",
            "chacha_drbg.c",
            "curves.c",
            "der.c",
            "ecdsa.c",
            "ed25519-donna/curve25519-donna-32bit.c",
            "ed25519-donna/curve25519-donna-helpers.c",
            "ed25519-donna/curve25519-donna-scalarmult-base.c",
            "ed25519-donna/ed25519-donna-32bit-tables.c",
            "ed25519-donna/ed25519-donna-basepoint-table.c",
            "ed25519-donna/ed25519-donna-impl-base.c",
            "ed25519-donna/ed25519-keccak.c",
            "ed25519-donna/ed25519-sha3.c",
            "ed25519-donna/ed25519.c",
            "ed25519-donna/modm-donna-32bit.c",
            "groestl.c",
            "hash_to_curve.c",
            "hasher.c",
            "hmac.c",
            "hmac_drbg.c",
            "memzero.c",
            "nem.c",
            "nist256p1.c",
            "pbkdf2.c",
            "rand.c",
            "rfc6979.c",
            "ripemd160.c",
            "secp256k1.c",
            "segwit_addr.c",
            "sha2.c",
            "sha3.c",
            "shamir.c",
            "slip39.c",
            "slip39_english.c",
            "tls_prf.c",
        ],
    );

    if cfg!(feature = "emulator") {
        // !@# use "insecure_prng" feature
        lib.add_public_define("USE_INSECURE_PRNG", Some("1"));
        lib.add_source(PathBuf::from(crypto_path).join("rand_insecure.c"));
    }

    if true
    /*secp256k1_zkp */
    {
        let secp256k1_path = "../../vendor/secp256k1-zkp";

        lib.add_public_defines([
            ("USE_SECP256K1_ZKP", None),
            ("USE_SECP256K1_ZKP_ECDSA", None),
            ("USE_ASM_ARM", None),      // !@# pouze hw
            ("USE_EXTERNAL_ASM", None), // !@# pouze hw
            ("USE_EXTERNAL_DEFAULT_CALLBACKS", None),
            ("ECMULT_GEN_PREC_BITS", Some("2")),
            ("ECMULT_WINDOW_SIZE", Some("2")),
            ("ENABLE_MODULE_GENERATOR", None),
            ("ENABLE_MODULE_RECOVERY", None),
            ("ENABLE_MODULE_SCHNORRSIG", None),
            ("ENABLE_MODULE_EXTRAKEYS", None),
            ("ENABLE_MODULE_ECDH", None),
        ]);

        lib.add_public_include(PathBuf::from(secp256k1_path).join("include"));

        if cfg!(feature = "emulator") {
            lib.add_public_define("SECP256K1_CONTEXT_SIZE", Some("208"));

            lib.add_source(PathBuf::from(crypto_path).join("rand_insecure.c"));
        } else {
            lib.add_public_define("SECP256K1_CONTEXT_SIZE", Some("180"));

            lib.add_source(PathBuf::from(secp256k1_path).join("src/asm/field_10x26_arm.s"));
        }

        lib.add_sources_from_folder(
            secp256k1_path,
            [
                "src/secp256k1.c",
                "src/precomputed_ecmult.c",
                "src/precomputed_ecmult_gen.c",
            ],
        );

        lib.add_sources_from_folder(
            crypto_path,
            ["zkp_context.c", "zkp_ecdsa.c", "zkp_bip340.c"],
        );
    }

    if true
    /*cfg!(feature = "sphincsplus")*/
    {
        let sphincsplus_path = "../../vendor/sphincsplus/ref";

        lib.add_public_include(sphincsplus_path);

        lib.add_public_define("PARAMS", Some("sphincs-sha2-128s"));

        lib.add_sources_from_folder(
            sphincsplus_path,
            [
                "address.c",
                "fors.c",
                "hash_sha2.c",
                "sha2.c",
                "sign.c",
                "thash_sha2_simple.c",
                "utils.c",
                "wots.c",
            ],
        );
    }
}

fn add_uzlib(lib: &mut cbuild::CLibrary) {
    let uzlib_path = "../../vendor/micropython/lib/uzlib";

    lib.add_public_include(uzlib_path);

    lib.add_sources_from_folder(uzlib_path, ["adler32.c", "crc32.c", "tinflate.c"]);
}
