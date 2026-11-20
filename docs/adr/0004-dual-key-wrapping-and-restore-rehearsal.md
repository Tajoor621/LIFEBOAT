# ADR 0004: Dual key wrapping and restore rehearsal evidence

- Status: accepted as P1.4 implementation baseline; independent review pending
- Date: 2026-11-20

## Context

A backup is not recoverable if the only device-held key is lost. A single passphrase-derived wrapping path is also fragile: forgotten passphrases can strand otherwise intact ciphertext. LIFEBOAT needs an offline recovery route that does not give the server access to the vault master key. The project also needs a repeatable way to prove that selected backup content can actually be decrypted and restored, rather than treating uploaded bytes as proof of recoverability.

## Decision

1. Generate a random 256-bit vault master key on the client.
2. Store two independent XChaCha20-Poly1305 wraps of that master key:
   - Argon2id-derived passphrase key, with a random 16-byte salt and persisted KDF parameters.
   - Independent random 256-bit recovery secret, represented by a versioned printable code with a 4-byte BLAKE3 transcription checksum.
3. Bound untrusted Argon2id parameters to 8–128 MiB, 1–6 iterations, and 1–4 lanes; cap passphrases at 4 KiB.
4. Bind each envelope's AAD to the protocol version, vault ID, and distinct wrapping slot.
5. Never transmit the passphrase, recovery secret, or plaintext master key to the vault server.
6. The recovery code is a high-entropy hexadecimal code, not a BIP-39 mnemonic. QR/PDF presentation is a later UI concern and must not invent a mnemonic vocabulary.
7. Restore rehearsal must decrypt selected chunks, verify manifest plaintext sizes and keyed object IDs, write restored logical items into an isolated scratch directory, and require caller-supplied format-aware parsers before reporting success.
8. Every contact item is included in a rehearsal; a deterministic configurable sample of non-contact items is included. The report is explicitly item-level evidence, not proof of complete snapshot coverage or a full device restore.

## Consequences

- Loss of the passphrase does not prevent recovery if the offline code and wrapper survive.
- Loss of both passphrase and recovery code makes the encrypted backup unrecoverable; this must be explained at setup.
- The recovery code is equivalent to a master-key unlock credential and must be displayed/exported only through an explicit owner-controlled flow.
- The server stores only opaque encrypted objects and must never receive recovery material.
- The current implementation provides a text print layout and core verification primitive. It does not yet provide QR/PDF UI, cross-device restore orchestration, production format parsers for every data class, or independent cryptographic review.
- Any successful rehearsal is scoped to the exact manifest and items reported. No blanket "backup verified" or "fully recoverable" status may be inferred.
