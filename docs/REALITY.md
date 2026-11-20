# Platform reality register

Status: P0 baseline, 2026-11-20. API availability is not equivalent to permission, policy approval, or reliable access on every device.

## Non-negotiable invariants

1. Device data is readable only when the app is installed and the owner has granted the required OS permission, entitlement, unlocked state, or explicit content selection.
2. If a device is gone, no prior backup exists, and no independent cloud copy survives, its local-only data is not recoverable.
3. No bypass of locks, Android file-based encryption, Apple Data Protection, Activation Lock, FRP, MDM, or account authentication.
4. Collection is visible, owner-consented, least-privilege, and auditable.
5. Unknown is distinct from no. A failed, stale, incomplete, or unrun probe is not proof of absence.
6. Assess each data class independently. Contacts coverage says nothing about SMS, WhatsApp, photos, or app data.
7. Upload completion is not backup verification. Integrity checking and restore rehearsal are separate steps.
8. No destructive action may proceed from guessed or incomplete evidence.

## Android

| Area | Verified constraint | Design consequence |
|---|---|---|
| Background work | WorkManager is the default for persistent deferrable work. Long-running workers use foreground-service behavior and remain subject to OS restrictions. | Chunk, persist, resume, honor constraints; never promise uninterrupted background execution. |
| Android 15+ data sync | Apps targeting API 35+ have a six-hour total limit per 24 hours for dataSync foreground services. Android 16 further changes job quota behavior. | Bounded resumable transfers; review current user-initiated transfer guidance per release. |
| Media | Media access depends on runtime permissions, OS version, and Android 14 selected-photo access. | Record exact granted scope; selected media is not the whole library. |
| Contacts | Contacts Provider access requires runtime permission and only exposes provider-available records. | Capture permission state, query outcome, count, and timestamp. |
| SMS/call log | Telephony access is permission- and distribution-policy-sensitive. Google Play restricts SMS and Call Log permissions to eligible use cases. | Optional capability; no assumption of Play approval. |
| Wipe | DevicePolicyManager wipe requires appropriate device/profile-owner or administrator provisioning; it is not a general consumer-app remote-wipe API. | Ordinary installs use Google Find Hub guidance, not an app wipe promise. |
| USB/MTP/ADB | Access depends on unlock/trust, debugging authorization, OEM behavior, and exposed interfaces. | In-hand rescue is best-effort; no locked-device or private-app-data extraction promise. |

## Google services

- Google Photos API changes effective in 2025 restrict Library API access to app-created content. Picker supports owner-selected media; it is not unrestricted full-library access. Full-library ingestion must support owner-initiated Takeout archives.
- Google People API supports authenticated contacts access with contacts.readonly or contacts scopes, pagination, and sync tokens. OAuth consent and verification rules apply.
- OAuth scopes vary in breadth and may require sensitive/restricted-scope verification. Use least privilege.
- Google Takeout is a user-initiated archive export/import, not continuous sync. Record export date, included products, parser errors, and unknown coverage.

## Apple platforms

- PhotoKit authorization can be denied, restricted, authorized, or limited. Limited authorization exposes only selected assets.
- Third-party iOS apps cannot silently create a full-device backup or act as an always-on remote-wipe agent.
- Use owner-authorized app data access, user-operated Finder/Apple Devices encrypted local backups, and Apple Find My.
- Apple documents encrypted computer backups may include saved passwords, Wi-Fi settings, website history, Health data, and call history. Face ID, Touch ID, and device passcode data are not included.
- iCloud data export is an owner-operated privacy workflow, not an unrestricted third-party Photos API.
- CardDAV/CalDAV availability depends on account/provider configuration; validate each connector before shipping.

## Third-party apps and providers

WhatsApp recovery is limited to owner-provided, legitimately accessible backups and official restore mechanisms. LIFEBOAT does not bypass end-to-end encryption or arbitrary app sandboxes. Samsung Cloud, OneDrive, Dropbox, Nextcloud, and Immich each require documented API scopes, authentication, quotas, terms, and manual fallback. No scraping private web interfaces or storing provider passwords.

## Status semantics

- yes: complete usable copy for the declared scope, supported by evidence.
- partial: known subset, time range, selected asset set, or incomplete archive.
- no: authoritative checks establish no supported surviving copy for the declared scope.
- unknown: evidence is insufficient, stale, failed, contradictory, or not checked.

Counts and timestamps are observed values, never estimates. Missing values remain null/unknown.

## Official references

- Android WorkManager long-running workers: https://developer.android.com/develop/background-work/background-tasks/persistent/how-to/long-running
- Android foreground service timeouts: https://developer.android.com/develop/background-work/services/fgs/timeout
- Android foreground service changes: https://developer.android.com/develop/background-work/services/fgs/changes
- DevicePolicyManager: https://developer.android.com/reference/android/app/admin/DevicePolicyManager
- Google Photos API updates: https://developers.google.com/photos/support/updates
- Google Photos release notes: https://developers.google.com/photos/support/release-notes
- Google People API connections: https://developers.google.com/people/api/rest/v1/people.connections/list
- Google OAuth policies: https://developers.google.com/identity/protocols/oauth2/policies
- Apple PhotoKit authorization: https://developer.apple.com/documentation/photos/phauthorizationstatus
- Apple encrypted local backups: https://support.apple.com/en-gb/108353
- Apple privacy export: https://privacy.apple.com/

## Open verification before feature release

Verify Android MediaStore behavior across API 26-current, Android 14 selected media, current Play SMS/Call Log eligibility, Android 16 transfer quotas, Apple CardDAV/CalDAV behavior, iOS backup manifest/keybag formats, WhatsApp formats and key availability, each provider's current OAuth/rate limits/terms, and independent cryptographic review. These are release gates, not assumptions.

## Explicit non-promises

No recovery of data that was never copied; no extraction from permanently locked/destroyed devices; no universal background execution; no universal SMS/call-log access; no full iOS backup from an ordinary app; no remote wipe on an ordinary consumer installation.
