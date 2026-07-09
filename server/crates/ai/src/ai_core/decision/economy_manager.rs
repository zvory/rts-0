use super::expansion::ExpansionPlan;
use super::production::wants_depot;
use super::resources::{plan_economy, EconomyPlan};
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OilDemandSignal {
    ProfileDefault,
    ExactWorkers(usize),
    AtLeastWorkers(usize),
    HoldCurrent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct EconomyManagerSignals {
    pub(super) recovery_active: bool,
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
    let mut plan = plan_economy(
        input.observation,
        input.facts,
        input.profile,
        input.signals.recovery_active,
        oil_override,
    );
    match input.signals.oil_demand {
        OilDemandSignal::ProfileDefault | OilDemandSignal::ExactWorkers(_) => {}
        OilDemandSignal::AtLeastWorkers(workers) => {
            if !plan.mineable_oil_nodes.is_empty() {
                plan.desired_oil_workers = plan.desired_oil_workers.max(workers);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_core::observation::{
        AiEconomy, AiEntityState, AiEntitySummary, AiMapSummary, AiObservation, AiPlayerSummary,
        AiResourceSummary,
    };
    use crate::ai_core::profiles::AI_2_1_ECONOMY_MANAGER;

    fn entity(id: u32, kind: EntityKind) -> AiEntitySummary {
        AiEntitySummary {
            id,
            owner: 1,
            kind,
            x: 8.0 * config::TILE_SIZE as f32,
            y: 8.0 * config::TILE_SIZE as f32,
            state: AiEntityState::Idle,
            is_complete: true,
            production_queue_len: if kind == EntityKind::CityCentre {
                Some(0)
            } else {
                None
            },
            production_kind: None,
            latched_node: None,
            target_id: None,
            free_for_combat: false,
        }
    }

    fn resource(id: u32, kind: EntityKind, x: f32, y: f32) -> AiResourceSummary {
        AiResourceSummary {
            id,
            kind,
            x,
            y,
            remaining: 1_000,
        }
    }

    fn observation() -> AiObservation {
        let tile = config::TILE_SIZE as f32;
        AiObservation {
            player_id: 1,
            tick: 90,
            map: AiMapSummary {
                width: 64,
                height: 64,
                tile_size: config::TILE_SIZE,
            },
            economy: AiEconomy {
                steel: 500,
                oil: 0,
                supply_used: 4,
                supply_cap: 12,
            },
            own_start_tile: (8, 8),
            players: vec![
                AiPlayerSummary {
                    id: 1,
                    team_id: 1,
                    start_tile: (8, 8),
                    is_ai: true,
                    is_alive: true,
                },
                AiPlayerSummary {
                    id: 2,
                    team_id: 2,
                    start_tile: (48, 48),
                    is_ai: false,
                    is_alive: true,
                },
            ],
            owned: vec![entity(1, EntityKind::CityCentre), entity(2, EntityKind::Worker)],
            resources: vec![
                resource(100, EntityKind::Steel, 8.0 * tile, 9.0 * tile),
                resource(101, EntityKind::Oil, 9.0 * tile, 9.0 * tile),
            ],
            visible_allies: Vec::new(),
            visible_enemies: Vec::new(),
            pending_builds: Vec::new(),
            upgrades: Vec::new(),
        }
    }

    #[test]
    fn economy_manager_outputs_action_proposals() {
        let observation = observation();
        let facts = AiFacts::from_observation(&observation);
        let expansion_plan = ExpansionPlan {
            policy: AI_2_1_ECONOMY_MANAGER.expansion,
            should_save: true,
            blocks_tech_path: false,
            blockers: Vec::new(),
        };

        let output = propose_economy(EconomyManagerInput {
            observation: &observation,
            facts: &facts,
            profile: &AI_2_1_ECONOMY_MANAGER,
            expansion_plan: &expansion_plan,
            signals: EconomyManagerSignals {
                recovery_active: false,
                oil_demand: OilDemandSignal::AtLeastWorkers(1),
                defer_supply_for_tech: false,
                emergency_supply: false,
                defer_worker_training_for_tech: false,
            },
        });

        assert!(output.proposes(EconomyProposal::BuildExpansionCityCentre));
        assert!(output.proposes(EconomyProposal::TrainWorker));
        assert!(output.proposes(EconomyProposal::AssignOilWorkers));
        assert!(output.proposes(EconomyProposal::AssignSteelWorkers));
    }

    #[test]
    fn economy_manager_signals_can_hold_oil_at_current_assignment() {
        let observation = observation();
        let facts = AiFacts::from_observation(&observation);
        let expansion_plan = ExpansionPlan {
            policy: None,
            should_save: false,
            blocks_tech_path: false,
            blockers: Vec::new(),
        };

        let output = propose_economy(EconomyManagerInput {
            observation: &observation,
            facts: &facts,
            profile: &AI_2_1_ECONOMY_MANAGER,
            expansion_plan: &expansion_plan,
            signals: EconomyManagerSignals {
                recovery_active: false,
                oil_demand: OilDemandSignal::HoldCurrent,
                defer_supply_for_tech: false,
                emergency_supply: false,
                defer_worker_training_for_tech: false,
            },
        });

        assert_eq!(output.plan.desired_oil_workers, output.plan.current_oil_workers);
        assert!(!output.proposes(EconomyProposal::AssignOilWorkers));
    }
}
