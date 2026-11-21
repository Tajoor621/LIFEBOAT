# Android permission and capability matrix

| Data class / capability | Android mechanism | P2 status | Rules |
|---|---|---|---|
| Contacts | Contacts Provider + READ_CONTACTS | P2.2 implemented | Just-in-time permission; read-only VCF export; destination integrity check. |
| Photos | MediaStore | P2.3 metadata inventory | Android 13 granular permission; Android 14 selected-only is partial; no content copied. |
| Videos | MediaStore | P2.3 metadata inventory | Separate permission; selected-only is partial; no content copied. |
| Documents | Storage Access Framework | Later | Owner selects files/folders; persist URI grants only with disclosure. |
| SMS/MMS and call log | Restricted provider permissions | Deferred | Separate distribution eligibility review. |
| App-private data | Android sandbox | Unsupported | No root, exploit or sandbox bypass. |
| WhatsApp | Owner-provided official export/backup | Later | No private database/key extraction or E2EE bypass. |
| Location, notifications, accessibility, device admin/wipe | Special access / device policy | Not requested | Not needed for current backup scope. |

Permission states include not_requested, granted, denied, permanently_denied, limited, revoked, not_applicable and unknown. A grant is not proof of complete data coverage.
