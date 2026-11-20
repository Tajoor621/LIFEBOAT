# LIFEBOAT

**Honest, owner-controlled device backup, recovery, and loss response.**

LIFEBOAT helps people preserve data before device loss, assess what is genuinely recoverable, restore verified backups, and respond safely when a device is lost.

## Non-negotiable reality contract

- Device data is readable only when software is installed and the owner has granted the required permissions.
- If a device is gone, backup was disabled, and no independent cloud copy survives, its local-only data is not recoverable.
- No bypass of device locks, file-based encryption, Activation Lock, FRP, MDM, or account authentication.
- Collection is visible, owner-consented, and auditable. No covert monitoring or stalkerware.
- Recovery claims are evidence-based. No simulated scans, fabricated progress, or unsupported promises.
- Destructive actions require verified backup evidence, an actual restore rehearsal, explicit confirmation, and a safe alternative.

## Roadmap

1. **P0 — Reality and architecture:** platform constraints, truth table, threat model, and ADRs.
2. **P1 — Core crypto, manifests, and vault:** client-side encryption primitives, versioned manifest encoding, immutable ciphertext storage, and the first authenticated HTTP transport layer.
3. P2 — Android backup agent and restore.
4. P3 — Recovery decision engine and evidence-based reports.
5. P4 — Cloud connectors and export ingestion.
6. P5 — In-hand rescue workflows.
7. P6 — Authenticated remote commands and guarded wipe state machine.
8. P7 — Rehydration, restore rehearsal, scheduler, and documentation.

## Current implementation

- `decision-engine/`: four-state evidence-based recoverability evaluator.
- `core-crypto/`: XChaCha20-Poly1305 envelopes, Argon2id key derivation, dual passphrase/recovery-code vault-key wrapping, keyed BLAKE3 object identifiers, Ed25519 signing/strict verification, versioned snapshot manifests, and a scratch restore-rehearsal verifier that checks every contact item plus a deterministic sample using caller-supplied format parsers.
- `vault/`: local immutable ciphertext storage plus an Axum HTTP API with pre-body per-device bearer authentication, per-device rate/storage limits, immutable object and manifest writes, digest-verified reads, verified HEAD probes, and persistent resumable chunk uploads.
- `docs/VAULT_API.md`: API protocol, credential configuration, deployment constraints, and explicit security limitations.
- `docs/RECOVERY_KIT.md`: offline key recovery, owner-handling rules, and restore-rehearsal evidence boundaries.
- `docs/REALITY.md`: platform constraints and release gates.
- `THREAT_MODEL.md`: threats, controls, and wipe safety gate.
- `docs/RecoverabilityTruthTable.md`: normative per-data-class status rules.

This is an early P1 foundation, **not a production-ready backup service**. P1.2/P1.3 add an authenticated API, per-device request/storage limits, immutable storage, and resumable chunk sessions. P1.4 adds client-side dual key wrapping, an offline recovery code, and an item-level restore-rehearsal primitive. TLS termination, device enrollment, online token revocation, durable distributed rate limiting, tamper-evident audit storage, committed-object retention controls, S3 storage, and independent security review remain outstanding. The Recovery Kit is currently a printable text format, not yet a BIP-39 mnemonic, QR/PDF UI, or proven cross-device restore workflow. Do not expose the API directly to the public Internet. The API accepts opaque encrypted bytes only; it does not decrypt backups.

## Local development

```sh
cargo fmt --all -- --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```

## Self-hosted container

See `docs/VAULT_API.md` for secure credential generation and limitations. The Compose file binds the published port to host loopback only. Configure `LIFEBOAT_DEVICE_TOKENS` in the host environment before running `docker compose up --build -d`. Use a TLS reverse proxy and restricted network access for remote clients.

## Architecture

- Android agent: Kotlin, minSdk 26+
- Shared core and vault: Rust
- Desktop console: Tauri + React
- Local metadata and queues: SQLite
- Encrypted blob storage: local filesystem or S3-compatible storage (S3 adapter not implemented)
- Self-hosting: Docker Compose; Raspberry Pi and VPS are targets subject to measured resource limits

## Safety and licensing

Use only for devices and accounts you own or are explicitly authorized to administer. Server and agent applications are planned AGPL-3.0-or-later; reusable core libraries are MIT. License boundaries and files must be finalized before release.
