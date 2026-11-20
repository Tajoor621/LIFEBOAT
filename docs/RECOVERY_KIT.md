# LIFEBOAT Recovery Kit — P1.4

Status: version-1 implementation draft. This is not yet a stable cross-platform interoperability contract.

## What is wrapped

The vault master key is 256 random bits. The client stores a versioned `KeyWrapper` containing two independent XChaCha20-Poly1305 envelopes of that same master key:

1. A passphrase envelope, protected by an Argon2id-derived key with a random 16-byte salt and persisted KDF parameters.
2. An offline recovery envelope, protected by an independently generated 256-bit recovery secret.

The server receives neither the master key, passphrase, nor recovery secret. The wrapper stores the KDF parameters and random 16-byte salt. The accepted Argon2id bounds are 8–128 MiB, 1–6 iterations, and 1–4 lanes; passphrases are limited to 4 KiB. Passphrase unlock validates and applies the bounded KDF metadata; recovery-code unlock validates the wrapper structure without depending on those KDF settings, so damaged passphrase metadata cannot disable the independent offline recovery path. Store the wrapper in owner-controlled client storage and back it up separately from the encrypted data. The wrapper binds both envelopes to the vault ID and slot-specific associated data.

## Recovery code format

Version 1 recovery codes use this printable form:

`LB1-<64 lowercase hex characters>-<8 lowercase hex checksum characters>`

The 64 hex characters encode 256 bits of cryptographically random secret material. The final 4-byte BLAKE3 checksum detects common transcription errors; it is not additional secret entropy. The code is not a BIP-39 mnemonic and does not currently have a QR/PDF renderer. The exact string may be used as the payload by a future QR presentation layer.

## Owner handling requirements

- Show the recovery code only once during explicit setup/export.
- Print it or transfer it offline to a trusted password manager or other secure medium.
- Never send it to the vault API, logs, analytics, support tickets, email, chat, or cloud clipboard.
- Anyone holding the code can unlock the vault. Treat it like the vault master key.
- Verify recovery on a clean second environment before trusting the backup.
- Do not store the recovery code beside the encrypted backup or on the same device.
- If the code is lost, passphrase unlock may still work; if both are lost, the encrypted backup is not recoverable.

The core `RecoveryKit` deliberately has no Debug or serialization implementation and zeroizes its in-memory string when dropped. The application must still avoid copying it into logs, crash reports, clipboard history, or persistent UI state.

## Restore rehearsal primitive

`run_restore_rehearsal` accepts a validated snapshot manifest, the vault's deduplication key, a scratch directory, a deterministic count of non-contact items to sample, a chunk resolver/decryptor, and a format-aware item validator.

- Every logical contact item in the manifest is selected.
- The requested number of other logical items is selected deterministically.
- The rehearsal is capped at 100,000 logical items, 512 MiB per item, and 2 GiB total restored bytes.
- All chunks for each selected item must be present in canonical contiguous order.
- The resolver must decrypt each object. Plaintext length and keyed object ID are verified against the manifest.
- Restored items are written under opaque generated filenames in the scratch directory and synced.
- The caller's validator must parse each restored item (for example, a real VCF parser for contacts); merely opening a file is not sufficient.
- Any failed rehearsal returns an error and removes files created by that run.

A successful report is evidence only for the selected snapshot items and validators. It does not prove that every data class is complete, that omitted items can be restored, or that a device-wide restore will succeed. A rehearsal must never be represented as full backup verification unless all relevant coverage requirements are separately met.

## Outstanding work

- A production UI for one-time display, QR rendering, PDF printing, and safe recovery confirmation.
- A word-based mnemonic only if a vetted word list and encoding standard are adopted; do not invent a word list.
- Persistent encrypted wrapper storage and migration/version policy on Android, desktop, and iOS.
- Device-independent end-to-end test using a newly installed client and only the printed Recovery Kit.
- Real format validators for VCF, ICS, media containers, documents, WhatsApp exports, and platform-specific data.
- A restore orchestration layer that fetches ciphertext, authenticates/decrypts it, restores original metadata and relationships, and emits signed evidence.
- Independent cryptographic and application security review.

Do not claim LIFEBOAT can recover data for which no valid backup or independent source exists.
