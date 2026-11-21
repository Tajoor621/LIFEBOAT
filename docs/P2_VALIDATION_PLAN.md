# P2 validation plan

## P2.0–P2.2 automated gates

- JDK 17 and Gradle 8.9 build the Android module.
- Unit tests verify explicit opt-in, non-empty scope, conservative permission-state handling, and VCF 4.0 escaping/Unicode.
- Android lint runs and the debug APK assembles.
- Manifest validation allows only `READ_CONTACTS` in P2.2; no network permission, services, receivers, providers, device-admin, accessibility, or notification-listener components.
- Fresh-install defaults, permission request timing, saved scope, withdrawal, and local export are manually reviewed.

## P2.2 build artifact

After unit tests, lint, APK assembly, and manifest checks pass, CI uploads
`app-debug.apk` as a 14-day workflow artifact named
`lifeboat-sentinel-debug-<commit-sha>`. This is a debug/test build, not a
production-signed release. Download it from the successful Android workflow's
Artifacts section and install it only on an owner-controlled test device.

## P2.2 device validation required

- Empty address book, 50k contacts, duplicate numbers, Unicode names, organizations, multiple accounts, and provider cursor behavior.
- Deny permission; deny twice; revoke in Settings; grant after denial; verify no query without a current grant.
- Revoke permission while a large query is running; snapshot must fail and remove its incomplete staging file.
- Force-stop/restart during export; no incomplete artifact may be marked complete.
- Export through Android document picker; compare destination SHA-256 and byte count with the in-app evidence.
- Verify the VCF in two independent contact importers; test multiline, comma, semicolon, backslash, and Unicode values.
- Confirm no personal data appears in logs, shared preferences, evidence JSON, crash reports, or network requests.

## P2.3–P2.6 required tests

- Media: Android 8–12 legacy permissions; Android 13 granular permissions; Android 14 selected-only/full visual-media grants; revoked grants; cloud placeholders; HEIC.
- Reliability: airplane mode, flaky 2G, reboot/process death mid-operation, low battery, battery saver, disk full, quota exhaustion, and clock skew.
- Privacy: no plaintext in logs, crash reports, analytics, or network requests; no upload before encryption and manifest verification.
- Restore: scratch restore, format/content digest validation, item counts, and coverage comparison. Upload alone is not backup.
- Device matrix before release: Pixel/AOSP, Samsung One UI, and Android 13/14.

## Release blockers

Any permission overreach, unannounced collection, manifest mismatch, plaintext upload, false yes/no status, missing stop control, or unverified restore blocks release.
