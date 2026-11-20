//! Evidence-based P0 recoverability semantics.
//! Evaluate one data class at a time. Device presence, account existence,
//! and backup timestamps alone are not proof of a usable copy.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Recoverability { Yes, Partial, No, Unknown }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyCoverage { Complete, Partial, Absent, Unknown, Conflicting }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceState {
    pub local: CopyCoverage,
    pub cloud: CopyCoverage,
    /// True only if all relevant source probes completed authoritatively.
    pub probes_complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    pub status: Recoverability,
    pub reason: &'static str,
}

pub fn decide(state: EvidenceState) -> Decision {
    use CopyCoverage::*;

    if state.local == Conflicting || state.cloud == Conflicting {
        return Decision { status: Recoverability::Unknown, reason: "conflicting_copy_evidence" };
    }
    if state.local == Complete || state.cloud == Complete {
        return Decision { status: Recoverability::Yes, reason: "complete_verified_copy" };
    }
    if state.local == Partial || state.cloud == Partial {
        return Decision { status: Recoverability::Partial, reason: "partial_verified_copy" };
    }
    if state.local == CopyCoverage::Unknown || state.cloud == CopyCoverage::Unknown || !state.probes_complete {
        return Decision { status: Recoverability::Unknown, reason: "insufficient_or_incomplete_evidence" };
    }
    if state.local == Absent && state.cloud == Absent {
        return Decision { status: Recoverability::No, reason: "all_relevant_sources_confirmed_absent" };
    }
    Decision { status: Recoverability::Unknown, reason: "unclassified_evidence" }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(local: CopyCoverage, cloud: CopyCoverage, probes_complete: bool) -> EvidenceState {
        EvidenceState { local, cloud, probes_complete }
    }

    #[test]
    fn complete_local_or_cloud_copy_is_yes() {
        assert_eq!(decide(ev(CopyCoverage::Complete, CopyCoverage::Absent, true)).status, Recoverability::Yes);
        assert_eq!(decide(ev(CopyCoverage::Absent, CopyCoverage::Complete, true)).status, Recoverability::Yes);
    }

    #[test]
    fn partial_local_or_cloud_copy_is_partial() {
        assert_eq!(decide(ev(CopyCoverage::Partial, CopyCoverage::Absent, true)).status, Recoverability::Partial);
        assert_eq!(decide(ev(CopyCoverage::Absent, CopyCoverage::Partial, true)).status, Recoverability::Partial);
    }

    #[test]
    fn confirmed_absence_is_no() {
        assert_eq!(decide(ev(CopyCoverage::Absent, CopyCoverage::Absent, true)).status, Recoverability::No);
    }

    #[test]
    fn failed_probe_is_unknown_not_no() {
        assert_eq!(decide(ev(CopyCoverage::Absent, CopyCoverage::Absent, false)).status, Recoverability::Unknown);
    }

    #[test]
    fn unprobed_source_is_unknown() {
        assert_eq!(decide(ev(CopyCoverage::Unknown, CopyCoverage::Absent, true)).status, Recoverability::Unknown);
    }

    #[test]
    fn contradictory_evidence_is_unknown_even_if_other_source_is_complete() {
        assert_eq!(decide(ev(CopyCoverage::Complete, CopyCoverage::Conflicting, true)).status, Recoverability::Unknown);
    }
}
