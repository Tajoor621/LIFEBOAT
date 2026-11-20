# ADR 0001: Architecture baseline

- Status: Accepted for P0 planning; implementation review required
- Date: 2026-11-20

## Decision

- Android agent: Kotlin, minSdk 26+.
- Shared cryptographic/manifest core: Rust.
- Vault: Rust REST API.
- Desktop console: Tauri + React; CLI-first recovery path where practical.
- Local metadata, queues, and audit: SQLite.
- Object storage: local filesystem for development; S3-compatible storage including MinIO for deployment.
- Deployment: Docker Compose; Raspberry Pi and VPS are targets, not guaranteed performance tiers.
- Connectors are independently testable modules with explicit scopes, limits, evidence, and dry-run mode.
- Monorepo: agent-android, vault, console, connectors, core-crypto, decision-engine, docs, e2e.

## Consequences

Shared formats and test vectors prevent duplicated crypto implementations. Server stores ciphertext and minimum operational metadata. Transfers are chunked, resumable, and crash-safe. iOS is a later user-authorized workflow, not an equivalent full-device agent. Battery, throughput, deduplication, and resume goals are measured targets, not universal promises.
