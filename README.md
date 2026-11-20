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
2. **P1 — Core crypto, manifests, and vault:** client-side encryption primitives, versioned manifest encoding, immutable ciphertext storage. Network API and production key lifecycle remain gated follow-up work.
3. P2 — Android backup agent and restore.
4. P3 — Recovery decision engine and evidence-based reports.
5. P4 — Cloud connectors and export ingestion.
6. P5 — In-hand rescue workflows.
7. P6 — Authenticated remote commands and guarded wipe state machine.
8. P7 — Rehydration, restore rehearsal, scheduler, and documentation.

## Current implementation

- decision-engine/: four-state evidence-based recoverability evaluator.
- core-crypto/: XChaCha20-Poly1305 envelope, Argon2id key derivation, keyed BLAKE3 object identifiers, Ed25519 signing/strict verification, and versioned snapshot manifest encoding.
- vault/: local immutable ciphertext object and encrypted-manifest storage with size and integrity checks.
- docs/REALITY.md: platform constraints and release gates.
- THREAT_MODEL.md: threats, controls, and wipe safety gate.
- docs/RecoverabilityTruthTable.md: normative per-data-class status rules.

This is an early P1 foundation, not a production-ready backup service. The vault crate is currently a local storage engine, not a network service. Streaming large objects, authenticated HTTP endpoints, quotas, retention, key wrapping/recovery, crash reconciliation, fuzzing, interoperability vectors, and independent security review are not yet complete.

## Architecture

- Android agent: Kotlin, minSdk 26+
- Shared core and vault: Rust
- Desktop console: Tauri + React
- Local metadata and queues: SQLite
- Encrypted blob storage: local filesystem or S3-compatible storage
- Self-hosting: Docker Compose; Raspberry Pi and VPS are targets subject to measured resource limits

## Safety and licensing

Use only for devices and accounts you own or are explicitly authorized to administer. Server and agent applications are planned AGPL-3.0-or-later; reusable core libraries are planned MIT. License boundaries and files must be finalized before release.
