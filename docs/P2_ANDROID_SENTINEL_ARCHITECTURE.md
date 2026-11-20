# P2 — Android Sentinel architecture

Status: P2.0 foundation; implementation is intentionally limited to a visible, permission-free shell and a testable consent/scope model.

## Objective

Build an owner-operated Android agent that can create an evidence-bearing local snapshot, encrypt data on-device, and later upload resumable ciphertext to the LIFEBOAT vault. Every data class is independently scoped and reported. The agent must never imply that a permission, provider, or backup exists when it has not been verified.

## P2 sequencing

1. P2.0 — Android project, build gate, permission model, and safety contract (this change).
2. P2.1 — onboarding, privacy disclosure, explicit per-class consent, permission-state inventory.
3. P2.2 — read-only Contacts Provider snapshot with pagination, stable IDs, timestamps, and parser-backed VCF export.
4. P2.3 — MediaStore inventory and owner-selected media support; Android 13/14 permission variants and partial-selection evidence.
5. P2.4 — local encrypted staging and manifest generation using reviewed shared-core bindings; no plaintext temporary files.
6. P2.5 — resumable upload client, network/battery constraints, interruption/reboot tests.
7. P2.6 — device restore/export rehearsal and evidence-backed status reporting.

SMS/call-log, notifications, app-private data, remote commands, and wipe are not in the first Android release. They require separate policy, entitlement, and threat reviews.

## Runtime invariants

- No data collection starts on install, launch, boot, or permission grant. The owner must explicitly start a snapshot.
- Explain each data class and destination before requesting its permission.
- Permission is requested just-in-time and per capability. Denied, limited, revoked, unavailable, and not-yet-requested are distinct states.
- Snapshot operations are read-only. No contacts/media modification or deletion.
- No hidden service, accessibility service, notification listener, device-admin enrollment, root, ADB, or lock bypass.
- Any future long-running operation must show a persistent foreground notification with a stop action and truthful progress.
- Store the smallest possible local metadata. Never log contact names, phone numbers, media paths, tokens, or plaintext.
- No remote wipe or remote lock code in P2.
- Network access is absent from the P2.0 shell. It will be added only when the encrypted upload client is implemented and reviewed.

## Android support baseline

- minSdk 26; target/compile SDK 35 for the initial reproducible build.
- Use Activity Result APIs for runtime permissions.
- Contacts: READ_CONTACTS, requested only after the owner selects Contacts.
- Shared media: versioned access. Android 13+ uses READ_MEDIA_IMAGES / READ_MEDIA_VIDEO; Android 14+ may grant selected visual media only. Older supported releases use READ_EXTERNAL_STORAGE with the applicable maxSdk boundary. Do not request broad access when a Photo Picker selection is sufficient.
- SMS and call log: deferred. Google Play restricted-permission eligibility is a separate release decision; sideloading does not override Android permission or user-consent requirements.
- WorkManager is for deferrable, resumable work. Foreground work is bounded and visibly notified; never promise uninterrupted execution.
- Android 15/16 transfer and job quotas must be revalidated against the target SDK and OS version at implementation time.

## Snapshot evidence model

Every data-class result must record:
- capability and requested scope;
- permission state observed before and after collection;
- collection start/end timestamps;
- source/provider and API version;
- item count and bytes only when measured;
- coverage boundary and known exclusions;
- errors and pagination completion;
- content digest and manifest reference after encryption;
- status: yes, partial, no, or unknown under docs/RecoverabilityTruthTable.md.

A failed query is unknown, not no. Limited photo access is partial, never full-library coverage. An empty provider result is not proof of absence unless the query completed successfully and scope is explicit.

## Local data and key handling

- Encrypt before any network transfer. The vault receives opaque ciphertext only.
- Use the existing core-crypto format only through a reviewed Android-compatible binding; do not independently reimplement cryptography in Kotlin.
- Keep content keys in Android Keystore-backed wrapping where supported. Recovery material remains owner-held and separate.
- Avoid plaintext disk staging. If unavoidable during a future reviewed parser flow, use app-private storage, strict lifecycle cleanup, and documented residual-risk handling.
- Bind snapshots to a device identity and monotonically increasing generation; never reuse encryption nonces or object IDs incorrectly.

## Distribution decision

The first CI artifact is a debug APK for build verification only. It is not a release-signed or store-ready package. Before distribution, choose F-Droid, Google Play, or direct GitHub release and perform a policy, signing, privacy, and reproducible-build review. Play availability for restricted data classes must never be assumed.

## P2.0 acceptance

- Android Gradle project builds on JDK 17 / Gradle 8.9 / AGP 8.7.3.
- Unit tests verify the consent/scope policy defaults to no collection and rejects an empty consent scope.
- Manifest contains no data permissions, network permission, services, receivers, or device-admin components.
- CI runs Android unit tests, lint, and debug assembly on changes under android-agent/.
- P2.0 does not claim backup, upload, restore, or recovery functionality.
