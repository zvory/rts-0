use super::expansion::ExpansionPlan;
use super::resources::{plan_economy, EconomyPlan};
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OilDemandSignal {
    ProfileDefault,
    ExactWorkers(usize),
    HoldCurrent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct EconomyManagerSignals {
    pub(super) oil_demand: OilDemandSignal,
    pub(super) defer_worker_training_for_tech: bool,
}

pub(super) struct EconomyManagerInput<'a> {
    pub(super) observation: &'a AiObservation,
    pub(super) facts: &'a AiFacts,
    pub(super) profile: &'a AiProfile,
    pub(super) expansion_plan: &'a ExpansionPlan,
    pub(super) signals: EconomyManagerSignals,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EconomyProposal {
    BuildExpansionResourceDepot,
    TrainWorker,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct EconomyManagerOutput {
    pub(super) plan: EconomyPlan,
    proposals: Vec<EconomyProposal>,
}

impl EconomyManagerOutput {
    pub(super) fn proposes(&self, proposal: EconomyProposal) -> bool {
        self.proposals.contains(&proposal)
    }
}

pub(super) fn propose_economy(input: EconomyManagerInput<'_>) -> EconomyManagerOutput {
    let oil_override = match input.signals.oil_demand {
        OilDemandSignal::ExactWorkers(workers) => Some(workers),
        _ => None,
    };
    let mut plan = plan_economy(input.observation, input.facts, input.profile, oil_override);
    match input.signals.oil_demand {
        OilDemandSignal::ProfileDefault | OilDemandSignal::ExactWorkers(_) => {}
        OilDemandSignal::HoldCurrent => {
            plan.desired_oil_workers = plan.current_oil_workers;
        }
    }
    // Engineers are construction units now; resource growth belongs to Depot-produced extractors.
    // Keep the starting Engineer and only train the profile's explicitly requested extras.
    plan.target_workers = 1usize.saturating_add(input.profile.workers.extra_builder_workers);

    let mut proposals = Vec::new();
    if input.expansion_plan.should_save {
        proposals.push(EconomyProposal::BuildExpansionResourceDepot);
    }
    if !input.signals.defer_worker_training_for_tech
        && input.facts.worker_count < plan.target_workers
    {
        proposals.push(EconomyProposal::TrainWorker);
    }
    EconomyManagerOutput { plan, proposals }
}
