use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseTiming {
    pub phase: String,
    pub branch: String,
    pub base_ref: String,
    pub phase_head: String,
    pub pr_number: Option<u64>,
    pub pr_url: Option<String>,
    pub merge_wait_state: String,
    pub timings_seconds: PhaseTimingSeconds,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseTimingSeconds {
    pub total: u64,
    pub executor: u64,
    pub pr: u64,
    pub wait: u64,
}
