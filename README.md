# LIFEBOAT

**Honest, owner-controlled device backup, recovery, and loss response.**

LIFEBOAT is an open-source project to help people preserve data before device loss, assess what is genuinely recoverable, restore verified backups, and respond safely when a device is lost.

## Non-negotiable reality contract

- Software can read device data only when it is installed and has the required user-granted permissions.
- If backup was disabled and the device is physically unavailable, data that exists nowhere else is not recoverable by LIFEBOAT.
- LIFEBOAT will not bypass device locks, file-based encryption, Activation Lock, Factory Reset Protection, mobile-device management, or platform security.
- All collection is visible, owner-consented, and auditable. No covert monitoring or stalkerware behavior.
- Recovery status is evidence-based. No simulated scans, fabricated progress, or unsupported recovery promises.
- Destructive actions require verified backup evidence, an actual restore rehearsal, explicit confirmation, and a safe alternative.

## Initial delivery order

1. **P0 — Reality and architecture:** verify platform constraints; publish the recoverability truth table, threat model, and architecture decisions.
2. P1 — Core cryptography, manifests, and vault MVP.
3. P2 — Android backup agent and restore.
4. P3 — Recovery decision engine and evidence-based reports.
5. P4 — Cloud connectors and export ingestion.
6. P5 — In-hand rescue workflows.
7. P6 — Authenticated remote commands and guarded wipe state machine.
8. P7 — Rehydration, restore rehearsal, scheduler, and user documentation.

## Planned architecture

- Android agent: Kotlin, Android API 26+
- Shared core and vault: Rust
- Desktop recovery console: Tauri + React
- Local metadata and queues: SQLite
- Encrypted blob storage: local filesystem or S3-compatible storage
- Self-hosting: Docker Compose, including Raspberry Pi and VPS deployments where resources permit

These are architectural choices, not claims that every feature is already implemented or validated.

## Project status

The repository is at the P0 foundation stage. Platform capabilities, limitations, and open verification items are recorded in [docs/REALITY.md](docs/REALITY.md). No backup, extraction, remote command, or wipe functionality is claimed to exist until implemented and tested.

## License

Planned licensing: AGPL-3.0 for server and agent applications; MIT for independently reusable core libraries. License files and per-component boundaries will be added before the first distributable release.

## Safety

Use LIFEBOAT only for devices and accounts you own or are explicitly authorized to administer. See [THREAT_MODEL.md](THREAT_MODEL.md) and [docs/REALITY.md](docs/REALITY.md).
