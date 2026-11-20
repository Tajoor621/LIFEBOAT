# Recoverability truth table

Normative P0 behavior. Evaluate each data class independently.

## Evidence dimensions

- Device access: in-hand, gone, or unknown.
- Local authorization: installed and permissioned, denied/not installed, or unknown.
- Local copy: complete, partial, absent, unknown, or conflicting.
- Cloud/export copy: complete, partial, absent, unknown, or conflicting.
- Probe quality: all relevant sources authoritatively checked, or incomplete/failed/not run.

## Decision table

| Local copy | Cloud copy | Probe quality | Result | Meaning |
|---|---|---|---|---|
| Complete | Any | Valid evidence | yes | Complete usable copy for declared scope. |
| Partial | Any | Valid evidence | partial | Known subset or range; disclose boundary. |
| Absent | Complete | Valid evidence | yes | Complete independent cloud copy. |
| Absent | Partial | Valid evidence | partial | Partial independent cloud copy. |
| Absent | Absent | All relevant probes complete | no | No supported surviving copy found for this scope. |
| Absent | Absent | Probe failed/not run/stale | unknown | Absence not established. |
| Unknown | Any | Any | unknown | Evidence insufficient. |
| Conflicting | Any | Any | unknown | Preserve conflict for manual verification. |

## Device-state guidance

| Device state | Backup history | Access/permission | Independent cloud | Result |
|---|---|---|---|---|
| Gone | Off | Irrelevant now | Confirmed absent after complete probes | no; explain local-only loss. |
| Gone | Off | Irrelevant now | Not checked or probe failed | unknown. |
| Gone | Complete verified backup | Irrelevant now | Any | yes only for classes in the verified manifest. |
| Gone | Partial backup | Irrelevant now | Any | partial for covered classes; assess others independently. |
| In hand | Off | Granted and data actually readable | No cloud copy | partial/yes only after actual copy and verification. |
| In hand | Off | Denied, locked, or not installed | No cloud copy | unknown/no based on authoritative evidence; possession alone is not access. |
| In hand | Any | USB trusted/unlocked and data exposed | Any | partial/yes only for data actually read and verified. |
| Any | Any | Cloud not authenticated/probed | Unknown | unknown. |

## Rules

1. Contacts evidence cannot set SMS or call-log status.
2. An account existing does not prove a data class is backed up.
3. A last-backup timestamp is not proof of content coverage; require manifest/item evidence.
4. Partial coverage includes an observed coverage boundary; otherwise boundary is null with explanation.
5. Counts are observed, not estimated.
6. Confidence measures evidence quality, not chance of magical recovery.
7. A failed connector preserves prior evidence and adds probe failure; it never overwrites status with no.
8. Negative results list sources checked, time, and limitations.
