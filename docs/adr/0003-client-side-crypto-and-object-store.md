# ADR 0003: Client-side encryption and immutable object store

- Status: Accepted as a P1 implementation baseline; independent security review required
- Date: 2026-11-20

## Decisions

- Use XChaCha20-Poly1305 AEAD with a fresh 192-bit random nonce for each encryption operation. Bind protocol/version/object context through associated data. Never reuse a nonce with the same key.
- Derive passphrase unlock keys with Argon2id v1.3. Initial profile: 64 MiB, 3 iterations, 4 lanes, 16-byte-or-larger random salt. Benchmark on minimum supported devices and version all persisted KDF parameters. Do not silently change KDF settings for existing vaults.
- Use BLAKE3 for ciphertext integrity and keyed BLAKE3 for opaque per-vault object identifiers. The object-ID key is separately derived from the vault master key. Keyed identifiers reduce raw plaintext-hash disclosure but still reveal equality within the same vault.
- Use Ed25519 signatures for device identity and signed metadata. Verify strictly; never enable legacy compatibility.
- Zeroize key buffers on drop where supported. This does not guarantee erasure of copies made by the OS, compiler, swap, crash dumps, or hardware.
- Encode manifests as a versioned fixed-schema Postcard structure. Require canonical entry order and reject unsupported versions, duplicate logical chunk keys, oversized chunks, and inconsistent AEAD tag lengths.
- Store only opaque ciphertext in the initial vault storage layer. Objects and manifests are immutable; create-if-absent prevents silent replacement. Verify stored bytes by BLAKE3 on read.

## Consequences and limitations

- The initial storage crate is a local storage engine, not yet a network service. Authentication, authorization, quotas, retention, TLS deployment, and HTTP upload/download endpoints remain P1 work.
- The first implementation buffers one object in memory. Streaming/chunked I/O and crash-recovery reconciliation are required before large-library production use.
- Deduplication leaks equality within one vault to the storage operator. It must never use global unkeyed plaintext hashes as object names.
- Cryptographic library selection is not a substitute for protocol review, interoperability vectors, fuzzing, dependency audit, and independent audit.
- No production recovery-kit or key-wrapping protocol is finalized in this ADR.
