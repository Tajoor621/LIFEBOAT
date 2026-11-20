# P2 validation plan

## P2.0–P2.1 automated gates

- JDK 17 and Gradle 8.9 build the Android module.
- Unit tests verify explicit opt-in, non-empty scope, and conservative permission-state handling.
- Android lint runs.
- Debug APK assembly succeeds.
- Manifest review confirms no data permissions, network access, services, receivers, device-admin, accessibility, or notification-listener components.
- Review consent-screen fresh-install defaults, persistence, and withdrawal behavior.

## P2.2–P2.6 required tests

### Permission and consent
- Fresh install performs zero collection and makes zero permission requests.
- Deny, permanently deny, revoke in Settings, and grant after denial are represented distinctly.
- Selecting one data class never implicitly enables another.
- App restart and device reboot do not silently start collection.

### Contacts
- Empty address book, 50k contacts, duplicate numbers, Unicode names, organizations, multiple accounts, pagination, provider exception, permission revoked mid-query.
- Export round-trip through VCF parser; counts and bytes measured, not estimated.

### Media
- Android 8–12 legacy permission behavior; Android 13 granular permissions; Android 14 selected-only and full visual-media grants; permission revoked mid-scan; cloud-backed/placeholder media; HEIC and Live Photo-like pairs.
- Selected media is never reported as full-library coverage.

### Reliability and privacy
- Airplane mode, flaky 2G, reboot mid-upload, process death, 2% battery, battery saver, disk full, quota exhausted, clock skew, storage permission revoked.
- No plaintext in logs, crash reports, analytics, or network requests.
- No upload until client-side encryption and manifest verification succeed.

### Restore
- Restore to a scratch destination; validate format and content digest; compare item counts and coverage. A successful upload alone is not a successful backup.
- Manual device matrix required before release: Pixel/AOSP, Samsung One UI, and at least one Android 13/14 device.

## Release blockers

Any permission overreach, unannounced collection, manifest mismatch, plaintext upload, false yes/no status, missing stop control, or unverified restore blocks release.
