# LIFEBOAT threat model

Status: P0 baseline. Security objective: preserve owner data while minimizing disclosure, preventing unauthorized commands, and avoiding false recovery or wipe claims.

## Assets and trust boundaries

Assets: plaintext on source devices and during restore; device/content/recovery keys; encrypted blobs and manifests; OAuth tokens; command authorization state; audit records; wipe evidence.

Boundaries: device OS and permission prompts; agent/local queue; network and push providers; vault API/object storage; desktop console; third-party OAuth providers; recovery-kit storage and trusted contacts.

## Threats and controls

| Threat | Required controls | Residual risk |
|---|---|---|
| Curious/compromised vault operator | Client-side encryption; server stores ciphertext; minimize metadata | Sizes, timing, traffic patterns, and account metadata may remain visible. |
| Stolen phone | OS secure storage, device revocation, visible lock guidance | Compromised/unlocked OS may expose plaintext. |
| SIM swap/SMS interception | Signed device-bound commands; SMS is notification/wakeup only | SMS never authenticates authority. |
| Phishing/OAuth abuse | PKCE/state, exact provider domains, narrow scopes, scope display | User may approve malicious consent; document revoke flow. |
| Forged/replayed command | Ed25519 signatures, nonce, monotonic counter, expiry, device binding, allowlist | Offline delivery and clock skew need bounded rejection rules. |
| Corrupt storage | AEAD, content hashes, signed manifests, integrity scrubs | Hashes prove byte integrity, not semantic correctness. |
| Endpoint malware | Least privilege, signed releases, no plaintext logs | Compromised OS can read data before encryption/after decryption. |
| Malicious connector | Isolated modules, scoped tokens, dry-run, audit, no arbitrary shell | Connector bugs may expose imported data. |
| Accidental wipe | Multi-stage gate, manifest verification, real restore rehearsal, typed confirmation, cooldown | Offline command remains queued, never reported complete. |
| Insider/trusted-contact abuse | No unilateral trusted-contact wipe authority | Social engineering remains possible. |
| Denial of service/ransomware | Versioned manifests, retention, rate limits, separate credentials | Availability depends on host maintenance and independent backups. |

## Security requirements

- Encrypt on the source device before upload; vault never receives plaintext content keys.
- Use authenticated encryption with unique nonces and a documented key hierarchy. Proposed XChaCha20-Poly1305, Argon2id, Ed25519, and BLAKE3 require design review before production.
- Separate device identity, content encryption, vault authentication, and recovery-kit keys.
- Never log plaintext, tokens, recovery phrases, keys, message bodies, or sensitive filenames by default.
- Remote commands must be signed, fresh, device-bound, and policy-authorized. Reject expired, replayed, unsigned, or unauthorized commands.
- Push and SMS are delivery hints only.
- Make backup, pause, revoke, export, and delete actions visible.
- Provide export/delete paths and document retention/metadata exposure.
- Append-only audit logs need externally anchored checkpoints for strong tamper evidence; a local hash chain alone can be rewritten by a privileged attacker.
- Never store Apple ID, Google, WhatsApp, or provider passwords.

## Wipe gate

Required progression: ASSESS → LOCK → BACKUP/VERIFY → RESTORE_REHEARSAL → CONFIRM → WIPE → POST_VERIFY.

Block wipe unless the platform capability was provisioned before loss; target/scope are shown; all required manifests and object checksums/byte counts verify; a real rehearsal restores and parses/opens a sample plus required contacts; owner types device name, sees irreversible warning, waits through cooldown, and confirms; command is signed, fresh, bound, and authorized.

An offline command is queued, not executed. Say wiped only after authoritative confirmation. Ordinary Android consumer installs use Google Find Hub; iOS uses Apple Find My. Never remove the device from its owner account or disable FRP/Activation Lock.

## Out of scope

Covert surveillance, stalkerware, keylogging, credential theft, exploit chains, lock bypass, rooting/jailbreaking, chip-off, MDM bypass, unauthorized account access, and guarantees for data with no surviving copy.
