use super::expansion::ExpansionPlan;
use super::production::wants_depot;
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
    pub(super) defer_supply_for_tech: bool,
    pub(super) emergency_supply: bool,
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
    BuildSupplyDepot,
    BuildExpansionCityCentre,
    TrainWorker,
    AssignOilWorkers,
    AssignSteelWorkers,
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
    plan.target_workers = plan
        .target_steel_workers
        .saturating_add(plan.desired_oil_workers);

    let mut proposals = Vec::new();
    if wants_depot(input.facts, input.profile)
        && (!input.signals.defer_supply_for_tech || input.signals.emergency_supply)
    {
        proposals.push(EconomyProposal::BuildSupplyDepot);
    }
    if input.expansion_plan.should_save {
        proposals.push(EconomyProposal::BuildExpansionCityCentre);
    }
    if !input.signals.defer_worker_training_for_tech
        && input.facts.worker_count < plan.target_workers
    {
        proposals.push(EconomyProposal::TrainWorker);
    }
    if plan.desired_oil_workers > plan.current_oil_workers {
        proposals.push(EconomyProposal::AssignOilWorkers);
    }
    if plan.target_steel_workers > plan.current_steel_workers {
        proposals.push(EconomyProposal::AssignSteelWorkers);
    }

    EconomyManagerOutput { plan, proposals }
}
