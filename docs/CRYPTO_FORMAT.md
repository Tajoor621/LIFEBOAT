# P1 cryptographic and storage format notes

Status: implementation draft, version 1. Not yet a stable public interoperability contract.

## Key hierarchy

- A random 256-bit vault master key is the root secret.
- Passphrase unlock keys use Argon2id v1.3 with a random salt and explicit persisted parameters.
- Purpose-specific keys are derived from the master key with distinct, fixed BLAKE3 derive-key contexts.
- The object identifier key is distinct from encryption and signing keys.
- Device signing identity uses Ed25519 and is not reused as an encryption key.

Do not store the passphrase or plaintext key in the vault. The server must not receive the vault master key.

## AEAD envelope

The current Rust type contains a version byte (currently 1), a 24-byte random XChaCha20-Poly1305 nonce, and ciphertext with its 16-byte authentication tag.

Callers must supply associated data that binds protocol version, object type, vault/device identity, and other required context using an unambiguous length-prefixed encoding. Do not concatenate ambiguous strings. AAD conventions must be finalized in a later protocol ADR before cross-client interoperability.

Generate a fresh nonce for each seal operation. Never reuse a nonce with the same key. Authentication failure returns an error; never expose partial plaintext.

## Password KDF

Default Argon2id profile is 65,536 KiB memory, 3 iterations, 4 lanes, and 32-byte output. The current implementation accepts 8–128 MiB, 1–6 iterations, and 1–4 lanes to bound attacker-controlled wrapper parameters. Passphrases are capped at 4 KiB. Salt must be at least 16 random bytes. Persist the exact algorithm/version/parameters/salt with the encrypted key wrapper. Benchmark on supported low-memory Android devices; future changes require versioned metadata and migration planning. Validate persisted KDF parameters before execution.

## Object identifiers and deduplication

Object IDs are keyed BLAKE3 over plaintext chunks using a per-vault derived key. This avoids exposing raw unkeyed plaintext hashes and prevents straightforward cross-vault correlation. It still reveals equality of repeated content within one vault. The server stores ciphertext only and cannot verify that an opaque ID corresponds to plaintext.

## Snapshot manifest

The current manifest uses a fixed-schema Postcard encoding with a format version and sorted entries. Entries refer to data class, opaque item ID, chunk index, keyed object ID, plaintext/ciphertext lengths, and optional source modification time. Names and paths must remain inside the encrypted manifest. Decoders reject unknown versions, unsorted/duplicate logical chunk keys, chunks above 16 MiB, and ciphertext lengths that do not include exactly one AEAD tag.

## Vault storage

The local store accepts lowercase hexadecimal opaque IDs, uses separate object and manifest namespaces, writes to a temporary file and syncs it, then uses create-if-absent hard-link semantics. It refuses overwrites, enforces a configured size limit, and verifies BLAKE3 on reads.

The filesystem store is a storage primitive. The separate `vault/` Axum service adds per-device bearer authentication, per-device quotas/rate limits, bounded request bodies, and resumable chunk transfers. TLS termination, device enrollment, token revocation, durable distributed rate limits, retention/deletion controls, multi-node coordination, and protection from a malicious local administrator remain deployment/release gates.


## Vault key wrapper and offline recovery kit (P1.4)

The client-side `KeyWrapper` contains two independent version-1 XChaCha20-Poly1305 envelopes for the same random 256-bit vault master key: one under an Argon2id passphrase-derived key, and one under a random 256-bit offline recovery secret. Both use slot- and vault-ID-bound associated data. The wrapper stores the KDF parameters and random 16-byte salt. The serialized wrapper never contains the recovery secret.

The printable recovery code is `LB1-<64 lowercase hex>-<8 lowercase hex checksum>`. It contains 256 bits of random secret material plus a 4-byte BLAKE3 transcription checksum. It is not a BIP-39 mnemonic. The application must display/export it only in an explicit owner-controlled flow and must never send it to the server or logs. See `docs/RECOVERY_KIT.md`.

## Restore rehearsal

The core restore rehearsal helper validates the manifest, resolves/decrypts all chunks for every contact item and a deterministic sample of other logical items, checks plaintext lengths and keyed object IDs, writes opaque-named scratch files, and requires a caller-supplied parser/validator for each restored item. It is an item-level evidence primitive, not proof of complete backup coverage or a full device restore.