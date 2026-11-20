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
3. **P1.5 — Integration and security validation:** cross-layer failure tests, restore integrity checks, API abuse tests, and container build verification. Automated CI passed on `863474b`; manual validation and independent review remain required.
4. **P1.6 — Vault hardening:** route-level body limits, versioned credential rotation, hash-linked audit ledger, locked builds and container smoke tests (implementation and automated CI accepted; operator acceptance pending).
5. P2 — Android backup agent and restore.
6. P3 — Recovery decision engine and evidence-based reports.
7. P4 — Cloud connectors and export ingestion.
8. P5 — In-hand rescue workflows.
9. P6 — Authenticated remote commands and guarded wipe state machine.
10. P7 — Rehydration, restore rehearsal, scheduler, and documentation.

## Current implementation

- `decision-engine/`: four-state evidence-based recoverability evaluator.
- `core-crypto/`: XChaCha20-Poly1305 envelopes, Argon2id key derivation, dual passphrase/recovery-code vault-key wrapping, keyed BLAKE3 object identifiers, Ed25519 signing/strict verification, versioned snapshot manifests, and a scratch restore-rehearsal verifier that checks every contact item plus a deterministic sample using caller-supplied format parsers.
- `vault/`: local immutable ciphertext storage plus an Axum HTTP API with pre-body per-device bearer authentication, route-level body limits, versioned active/previous token digests, per-device rate/storage limits, immutable object and manifest writes, digest-verified reads, and persistent resumable chunk uploads.
- `vault/src/audit.rs`: startup-verified, hash-linked JSONL audit ledger for successful write transitions.
- `docs/VAULT_API.md` and `docs/P1.6_HARDENING.md`: API protocol, versioned credential rotation, audit ledger, deployment constraints, and explicit security limitations.
- `docs/RECOVERY_KIT.md`: offline key recovery, owner-handling rules, and restore-rehearsal evidence boundaries.
- `docs/P1.5_VALIDATION_PLAN.md`: CI gates, failure matrix, manual release checks, and P1.5 exit criteria.
- `docs/REALITY.md`: platform constraints and release gates.
- `THREAT_MODEL.md`: threats, controls, and wipe safety gate.
- `docs/RecoverabilityTruthTable.md`: normative per-data-class status rules.

This is an early P1 foundation, **not a production-ready backup service**. P1.2/P1.3 provide an authenticated API, per-device request/storage limits, immutable storage, and resumable chunk sessions. P1.4 provides client-side dual key wrapping, an offline recovery code, and an item-level restore-rehearsal primitive. P1.5 adds integration coverage and container build checks. P1.6 now implements route-level body limits, versioned credential digests with bounded previous-token overlap, startup-verified hash-linked audit records, loopback-by-default binding, locked Rust/Docker builds, and a container smoke test in CI. All automated P1.6 gates passed in [run 44733561934](https://github.com/Tajoor621/LIFEBOAT/actions/runs/44733561934), and the generated `Cargo.lock` was committed to `main` as `ef34e36`. Operator deployment, interrupted-upload and independent-restore rehearsals remain required before production use. Remaining limitations include no online credential hot reload, no multi-node coordination, no built-in TLS, no external audit checkpoint anchoring, no committed-object retention controls, no S3 adapter, and no independent security review. The Recovery Kit is currently a printable text format, not yet a BIP-39 mnemonic, QR/PDF UI, or proven cross-device restore workflow. Do not expose the API directly to the public Internet. The API accepts opaque encrypted bytes only; it does not decrypt backups.

## Local development

```sh
cargo fmt --all -- --check
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace --release
```

## Self-hosted container

See `docs/VAULT_API.md` and `docs/P1.6_HARDENING.md`. Create `secrets/credentials.json` from `config/credentials.example.json`, replace the placeholder digest, set owner UID 10001 and mode 0600, then run `docker compose up --build -d`. The Compose file publishes only host loopback. Use a TLS reverse proxy and restricted network access for remote clients.

## Architecture

- Android agent: Kotlin, minSdk 26+
- Shared core and vault: Rust
- Desktop console: Tauri + React
- Local metadata and queues: SQLite
- Encrypted blob storage: local filesystem or S3-compatible storage (S3 adapter not implemented)
- Self-hosting: Docker Compose; Raspberry Pi and VPS are targets subject to measured resource limits

## Safety and licensing

Use only for devices and accounts you own or are explicitly authorized to administer. Server and agent applications are planned AGPL-3.0-or-later; reusable core libraries are MIT. License boundaries and files must be finalized before release.
