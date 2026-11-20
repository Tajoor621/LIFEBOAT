# Android permission and capability matrix

This matrix is a design guardrail, not a claim that a permission is approved for every distribution channel.

| Data class / capability | Android mechanism | P2 status | Rules |
|---|---|---|---|
| Contacts | Contacts Provider + READ_CONTACTS | P2.2 planned | Request only after explicit Contacts opt-in; read-only; report provider errors and pagination. |
| Photos | MediaStore / Photo Picker | P2.3 planned | Prefer Photo Picker; Android 14 selected-media grants are partial scope. |
| Videos | MediaStore / Photo Picker | P2.3 planned | Same as photos; track image/video scopes separately. |
| Documents | Storage Access Framework | Later | Owner selects files/folders; persist URI grants only with clear disclosure. |
| SMS/MMS | Telephony Provider + restricted permissions | Deferred | Separate Play eligibility review; never imply universal access. |
| Call log | CallLog Provider + restricted permission | Deferred | Separate Play eligibility review; never imply universal access. |
| App-private data | Android sandbox | Unsupported for ordinary app | No root, exploit, backup-abuse, or sandbox bypass. |
| WhatsApp | Owner-provided official export/backup | Later | No private app database/key extraction or E2EE bypass. |
| Device location | Android location APIs | Out of P2 scope | No tracking; not needed for backup. |
| Notifications / accessibility | Special access | Not requested | Not required for backup; no notification scraping or UI automation. |
| Device administration / wipe | DevicePolicyManager | Out of P2 scope | No enrollment or wipe implementation in P2. |

## Permission states

Represent at least: not_requested, granted, denied, permanently_denied, limited, revoked, not_applicable, and unknown.

A permission grant is not evidence that a complete data class was collected. Collection evidence must separately record successful query completion, selected scope, item count, and coverage boundary.
