//! Independent, on-demand memory observations; never an allocation decision.
//!
//! Sources and their measurement limitations belong to the existing observation
//! modules. No raw paths or arbitrary IO error text enter this report. Missing
//! data are states, never zero counters. There is no monitor, fallback, cgroup
//! effective-limit calculation, pool/cache connection or manual capacity knob.
//! IO byte bounds do not impose an execution-time bound. Linux native sampling
//! has not been validated by the current Windows tests and Linux crosscheck.

use crate::{
    cgroup_membership_observation::{
        self, CgroupV2LevelState, ObservationConsistency, UpperAncestors,
        VisibleCgroupV2Observation,
    },
    memory_observation::{self, PhysicalMemoryObservation},
    process_memory_observation::{self, ProcessRssObservation},
};
use serde::Serialize;
use std::{fmt, io};

const MAX_LEVELS: usize = 128;
const MAX_REPORT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryDiagnosticError {
    HierarchyBounds,
    ReportLimit,
    Serialization,
}
impl fmt::Display for MemoryDiagnosticError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str(match self {
            Self::HierarchyBounds => "memory diagnostic hierarchy exceeds report bounds",
            Self::ReportLimit => "memory diagnostic exceeds report byte bound",
            Self::Serialization => "memory diagnostic serialization failed",
        })
    }
}
impl std::error::Error for MemoryDiagnosticError {}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Source {
    HostMeminfo,
    SelfSmapsRollup,
    SelfVisibleCgroupV2,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum ErrorKind {
    Unsupported,
    NotFound,
    PermissionDenied,
    InvalidData,
    Interrupted,
    TimedOut,
    Other,
}
impl From<io::ErrorKind> for ErrorKind {
    fn from(kind: io::ErrorKind) -> Self {
        match kind {
            io::ErrorKind::Unsupported => Self::Unsupported,
            io::ErrorKind::NotFound => Self::NotFound,
            io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            io::ErrorKind::InvalidData => Self::InvalidData,
            io::ErrorKind::Interrupted => Self::Interrupted,
            io::ErrorKind::TimedOut => Self::TimedOut,
            _ => Self::Other,
        }
    }
}
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Outcome<T> {
    Observed { data: T },
    Unsupported { kind: ErrorKind },
    Unavailable { kind: ErrorKind },
}
#[derive(Debug, Serialize)]
struct Section<T> {
    source: Source,
    #[serde(flatten)]
    outcome: Outcome<T>,
}
impl<T> Section<T> {
    fn from_result(source: Source, value: io::Result<T>) -> Self {
        let outcome = match value {
            Ok(data) => Outcome::Observed { data },
            Err(error) if error.kind() == io::ErrorKind::Unsupported => Outcome::Unsupported {
                kind: ErrorKind::Unsupported,
            },
            Err(error) => Outcome::Unavailable {
                kind: error.kind().into(),
            },
        };
        Self { source, outcome }
    }
    fn observed(&self) -> bool {
        matches!(self.outcome, Outcome::Observed { .. })
    }
}
#[derive(Debug, Serialize)]
struct HostCounters {
    total_bytes: u64,
    available_bytes: u64,
}
#[derive(Debug, Serialize)]
struct RssCounter {
    resident_bytes: u64,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Consistency {
    NonAtomic,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Ancestors {
    Unknown,
}
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum Limit {
    Bytes { bytes: u64 },
    Max,
}
impl From<Option<u64>> for Limit {
    fn from(value: Option<u64>) -> Self {
        value.map_or(Self::Max, |bytes| Self::Bytes { bytes })
    }
}
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum LevelState {
    Observed {
        current_bytes: u64,
        max: Limit,
        high: Limit,
    },
    BoundaryUnobserved,
}
#[derive(Debug, Serialize)]
struct Level {
    leaf_ordinal: usize,
    #[serde(flatten)]
    state: LevelState,
}
#[derive(Debug, Serialize)]
struct Hierarchy {
    upper_ancestors: Ancestors,
    consistency: Consistency,
    levels: Vec<Level>,
}

/// A closed, content-free development report. Fields cannot be replaced by
/// arbitrary paths, error strings, labels or unbounded caller collections.
#[derive(Debug, Serialize)]
pub struct MemoryDiagnosticReport {
    format_version: u8,
    consistency: Consistency,
    host_physical: Section<HostCounters>,
    self_rss: Section<RssCounter>,
    visible_cgroup_v2: Section<Hierarchy>,
}
impl MemoryDiagnosticReport {
    /// True for any failed section or a visible boundary with no memory data.
    /// False means the three requested APIs succeeded, not full visibility:
    /// upper ancestors remain unknown and all observations remain non-atomic.
    pub fn is_partial(&self) -> bool {
        !self.host_physical.observed()
            || !self.self_rss.observed()
            || match &self.visible_cgroup_v2.outcome {
                Outcome::Observed { data } => data
                    .levels
                    .iter()
                    .any(|level| matches!(level.state, LevelState::BoundaryUnobserved)),
                _ => true,
            }
    }
    /// Serialize one JSON value. The caller adds LF; the 64 KiB cap includes it.
    /// Serialization/overbound errors return no partial output. This cap is not
    /// a process RSS or all-allocation limit; inputs/IO/allocator work are unpooled.
    pub fn to_json(&self) -> Result<String, MemoryDiagnosticError> {
        self.encode_with_limit(MAX_REPORT_BYTES)
    }
    fn encode_with_limit(&self, limit: usize) -> Result<String, MemoryDiagnosticError> {
        let wire = serde_json::to_string(self).map_err(|_| MemoryDiagnosticError::Serialization)?;
        if wire
            .len()
            .checked_add(1)
            .is_none_or(|length| length > limit)
        {
            return Err(MemoryDiagnosticError::ReportLimit);
        }
        Ok(wire)
    }
}

/// Invoke the three existing APIs exactly once, independently and sequentially.
/// Only the self-visible hierarchy is requested; no caller path is accepted.
/// Errors in one section do not prevent the other two observations from running.
pub fn inspect_memory() -> Result<MemoryDiagnosticReport, MemoryDiagnosticError> {
    let host = memory_observation::observe_physical_memory();
    let rss = process_memory_observation::observe_self_smaps_rollup_rss();
    let cgroup = cgroup_membership_observation::observe_self_cgroup_v2();
    build_report(host, rss, cgroup)
}

fn build_report(
    host: io::Result<PhysicalMemoryObservation>,
    rss: io::Result<ProcessRssObservation>,
    cgroup: io::Result<VisibleCgroupV2Observation>,
) -> Result<MemoryDiagnosticReport, MemoryDiagnosticError> {
    let hierarchy = match cgroup {
        Err(error) => Err(error),
        Ok(observation) => {
            if observation.levels.is_empty() || observation.levels.len() > MAX_LEVELS {
                return Err(MemoryDiagnosticError::HierarchyBounds);
            }
            let upper_ancestors = match observation.upper_ancestors {
                UpperAncestors::Unknown => Ancestors::Unknown,
            };
            let consistency = match observation.consistency {
                ObservationConsistency::NonAtomic => Consistency::NonAtomic,
            };
            // Move only selected counters. Raw membership, mountpoint and each
            // directory are dropped without serialization, display or hashing.
            let levels = observation
                .levels
                .into_iter()
                .enumerate()
                .map(|(leaf_ordinal, level)| {
                    let state = match level.state {
                        CgroupV2LevelState::Observed(value) => LevelState::Observed {
                            current_bytes: value.current_bytes,
                            max: value.max_bytes.into(),
                            high: value.high_bytes.into(),
                        },
                        CgroupV2LevelState::BoundaryUnobserved => LevelState::BoundaryUnobserved,
                    };
                    Level {
                        leaf_ordinal,
                        state,
                    }
                })
                .collect();
            Ok(Hierarchy {
                upper_ancestors,
                consistency,
                levels,
            })
        }
    };
    Ok(MemoryDiagnosticReport {
        format_version: 1,
        consistency: Consistency::NonAtomic,
        host_physical: Section::from_result(
            Source::HostMeminfo,
            host.map(|value| HostCounters {
                total_bytes: value.total_bytes,
                available_bytes: value.available_bytes,
            }),
        ),
        self_rss: Section::from_result(
            Source::SelfSmapsRollup,
            rss.map(|value| RssCounter {
                resident_bytes: value.resident_bytes,
            }),
        ),
        visible_cgroup_v2: Section::from_result(Source::SelfVisibleCgroupV2, hierarchy),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cgroup_membership_observation::VisibleCgroupV2Level;
    use crate::cgroup_memory_observation::CgroupV2LevelObservation;
    use std::path::PathBuf;

    fn hierarchy(states: Vec<CgroupV2LevelState>) -> VisibleCgroupV2Observation {
        VisibleCgroupV2Observation {
            membership: b"/SECRET_MEMBERSHIP".to_vec(),
            mountpoint: b"/SECRET_MOUNT".to_vec(),
            levels: states
                .into_iter()
                .map(|state| VisibleCgroupV2Level {
                    directory: PathBuf::from("/SECRET_DIRECTORY"),
                    state,
                })
                .collect(),
            upper_ancestors: UpperAncestors::Unknown,
            consistency: ObservationConsistency::NonAtomic,
        }
    }
    fn observed(state: Vec<CgroupV2LevelState>) -> MemoryDiagnosticReport {
        build_report(
            Ok(PhysicalMemoryObservation {
                total_bytes: 100,
                available_bytes: 0,
            }),
            Ok(ProcessRssObservation { resident_bytes: 0 }),
            Ok(hierarchy(state)),
        )
        .unwrap()
    }
    fn level(
        current_bytes: u64,
        max_bytes: Option<u64>,
        high_bytes: Option<u64>,
    ) -> CgroupV2LevelState {
        CgroupV2LevelState::Observed(CgroupV2LevelObservation {
            current_bytes,
            max_bytes,
            high_bytes,
        })
    }
    #[test]
    fn sections_preserve_zero_non_atomic_unknown_ancestors_and_unordered_limits_without_paths() {
        let report = observed(vec![
            level(u64::MAX, Some(0), None),
            level(0, None, Some(1)),
        ]);
        assert!(!report.is_partial());
        let wire = report.to_json().unwrap();
        assert!(!wire.contains("SECRET"));
        let value: serde_json::Value = serde_json::from_str(&wire).unwrap();
        assert_eq!(value["consistency"], "non_atomic");
        assert_eq!(value["host_physical"]["data"]["available_bytes"], 0);
        assert_eq!(value["self_rss"]["status"], "observed");
        assert_eq!(value["self_rss"]["data"]["resident_bytes"], 0);
        let data = &value["visible_cgroup_v2"]["data"];
        assert_eq!(data["upper_ancestors"], "unknown");
        assert_eq!(data["consistency"], "non_atomic");
        assert_eq!(data["levels"][0]["current_bytes"], u64::MAX);
        assert_eq!(
            data["levels"][0]["max"],
            serde_json::json!({"state":"bytes","bytes":0})
        );
        assert_eq!(
            data["levels"][0]["high"],
            serde_json::json!({"state":"max"})
        );
        assert_eq!(data["levels"][1]["leaf_ordinal"], 1);
    }
    #[test]
    fn errors_are_fixed_classes_without_error_text_or_fake_data_and_other_sections_survive() {
        for (kind, expected) in [
            (io::ErrorKind::Unsupported, "unsupported"),
            (io::ErrorKind::NotFound, "not_found"),
            (io::ErrorKind::PermissionDenied, "permission_denied"),
            (io::ErrorKind::InvalidData, "invalid_data"),
            (io::ErrorKind::Interrupted, "interrupted"),
            (io::ErrorKind::TimedOut, "timed_out"),
            (io::ErrorKind::Other, "other"),
        ] {
            let report = build_report(
                Err(io::Error::new(kind, "/SECRET_ERROR credentials")),
                Ok(ProcessRssObservation { resident_bytes: 7 }),
                Err(io::Error::new(kind, "SECRET_CGROUP")),
            )
            .unwrap();
            assert!(report.is_partial());
            let wire = report.to_json().unwrap();
            assert!(!wire.contains("SECRET"));
            let value: serde_json::Value = serde_json::from_str(&wire).unwrap();
            for section in ["host_physical", "visible_cgroup_v2"] {
                assert_eq!(value[section]["kind"], expected);
                assert!(value[section].get("data").is_none());
                assert_eq!(
                    value[section]["status"],
                    if kind == io::ErrorKind::Unsupported {
                        "unsupported"
                    } else {
                        "unavailable"
                    }
                );
            }
            assert_eq!(value["self_rss"]["data"]["resident_bytes"], 7);
        }
    }
    #[test]
    fn visible_boundary_unobserved_is_partial_without_fabricating_limits_or_ancestor_visibility() {
        let report = observed(vec![
            level(1, None, None),
            CgroupV2LevelState::BoundaryUnobserved,
        ]);
        assert!(report.is_partial());
        let value: serde_json::Value = serde_json::from_str(&report.to_json().unwrap()).unwrap();
        assert_eq!(
            value["visible_cgroup_v2"]["data"]["levels"][1],
            serde_json::json!({"leaf_ordinal":1,"state":"boundary_unobserved"})
        );
        assert_eq!(
            value["visible_cgroup_v2"]["data"]["upper_ancestors"],
            "unknown"
        );
    }
    #[test]
    fn hierarchy_and_serialized_byte_caps_fail_completely_without_truncation() {
        let report = observed(vec![
            level(u64::MAX, Some(u64::MAX), Some(u64::MAX));
            MAX_LEVELS
        ]);
        let wire = report.to_json().unwrap();
        assert!(wire.len() < MAX_REPORT_BYTES);
        assert_eq!(report.encode_with_limit(wire.len() + 1).unwrap(), wire);
        assert_eq!(
            report.encode_with_limit(wire.len()),
            Err(MemoryDiagnosticError::ReportLimit)
        );
        for count in [0, MAX_LEVELS + 1] {
            assert_eq!(
                build_report(
                    Ok(PhysicalMemoryObservation {
                        total_bytes: 1,
                        available_bytes: 0
                    }),
                    Ok(ProcessRssObservation { resident_bytes: 0 }),
                    Ok(hierarchy(vec![level(0, None, None); count]))
                )
                .unwrap_err(),
                MemoryDiagnosticError::HierarchyBounds
            );
        }
    }
}
