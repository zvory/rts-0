use crate::ai_core::actions::{ReservationCounts, SpendBudget};
use crate::ai_core::facts::AiFacts;
use crate::ai_core::observation::AiObservation;
use crate::ai_core::profiles::AiProfile;
use rts_sim::game::entity::EntityKind;

use super::expansion::ExpansionBlocker;
use super::frontal::FrontalWaveBlocker;
use super::AiIntent;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum StrategicGoal {
    Economy,
    Expansion,
    Tech,
    Production,
    LocalDefense,
    FrontalAttack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum GoalStatus {
    Selected,
    Skipped,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum GoalBlocker {
    BudgetSteel,
    BudgetOil,
    SupplyCap,
    MissingPrerequisite(&'static str),
    NoBuilder,
    NoProductionBuilding,
    NoReadyUnits,
    AttackCadence,
    WaitingForUnits,
    WaitingForTank,
    WaitingForMethamphetamines,
    Staging,
    DeferredForExpansion,
    DeferredForTech,
    DefensivePanic,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BudgetTrace {
    pub(crate) start_steel: u32,
    pub(crate) start_oil: u32,
    pub(crate) start_free_supply: u32,
    pub(crate) end_steel: u32,
    pub(crate) end_oil: u32,
    pub(crate) end_free_supply: u32,
}

impl BudgetTrace {
    pub(crate) fn new(start: SpendBudget, end: SpendBudget) -> Self {
        Self {
            start_steel: start.steel(),
            start_oil: start.oil(),
            start_free_supply: start.free_supply(),
            end_steel: end.steel(),
            end_oil: end.oil(),
            end_free_supply: end.free_supply(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReservationTrace {
    pub(crate) workers: usize,
    pub(crate) resource_nodes: usize,
    pub(crate) production_buildings: usize,
}

impl From<ReservationCounts> for ReservationTrace {
    fn from(counts: ReservationCounts) -> Self {
        Self {
            workers: counts.workers,
            resource_nodes: counts.resource_nodes,
            production_buildings: counts.production_buildings,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GoalTrace {
    pub(crate) goal: StrategicGoal,
    pub(crate) status: GoalStatus,
    pub(crate) blockers: Vec<GoalBlocker>,
    pub(crate) intents: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ManagerOutputTrace {
    pub(crate) profile_id: &'static str,
    pub(crate) tick: u32,
    pub(crate) goals: Vec<GoalTrace>,
    pub(crate) commands: Vec<String>,
    pub(crate) budget: BudgetTrace,
    pub(crate) reservations: ReservationTrace,
}

impl ManagerOutputTrace {
    pub(crate) fn format_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(format!(
            "profile={} tick={} budget=steel:{}->{} oil:{}->{} supply:{}->{} reservations=workers:{} nodes:{} production:{}",
            self.profile_id,
            self.tick,
            self.budget.start_steel,
            self.budget.end_steel,
            self.budget.start_oil,
            self.budget.end_oil,
            self.budget.start_free_supply,
            self.budget.end_free_supply,
            self.reservations.workers,
            self.reservations.resource_nodes,
            self.reservations.production_buildings
        ));
        for goal in &self.goals {
            let blockers = format_blockers(&goal.blockers);
            let intents = if goal.intents.is_empty() {
                "-".to_string()
            } else {
                goal.intents.join(",")
            };
            lines.push(format!(
                "goal={:?} status={:?} blockers={} intents={}",
                goal.goal, goal.status, blockers, intents
            ));
        }
        for command in &self.commands {
            lines.push(format!("command={command}"));
        }
        lines
    }

    pub(crate) fn format_compact(&self) -> String {
        self.format_lines().join("\n")
    }
}

#[derive(Clone, Copy)]
pub(super) struct TraceInput<'a> {
    pub(super) observation: &'a AiObservation,
    pub(super) profile: &'static AiProfile,
    pub(super) facts: &'a AiFacts,
    pub(super) intents: &'a [AiIntent],
    pub(super) command_trace: &'a [String],
    pub(super) start_budget: SpendBudget,
    pub(super) end_budget: SpendBudget,
    pub(super) reservations: ReservationCounts,
    pub(super) save_for_expansion: bool,
    pub(super) expansion_blockers: &'a [ExpansionBlocker],
    pub(super) expansion_blocks_tech_path: bool,
    pub(super) save_for_unplanned_expansion: bool,
    pub(super) save_for_required_tech_building: bool,
    pub(super) save_worker_training_for_tech: bool,
    pub(super) defensive_panic_active: bool,
    pub(super) local_threat_active: bool,
    pub(super) ready_units: usize,
    pub(super) attack_size: usize,
    pub(super) attack_due: bool,
    pub(super) frontal_wave_blockers: &'a [FrontalWaveBlocker],
    pub(super) required_tech_path: &'a [EntityKind],
}

pub(super) fn build_manager_trace(input: TraceInput<'_>) -> ManagerOutputTrace {
    let mut goals = vec![
        goal_trace(StrategicGoal::Economy, input),
        goal_trace(StrategicGoal::Expansion, input),
        goal_trace(StrategicGoal::Tech, input),
        goal_trace(StrategicGoal::Production, input),
        goal_trace(StrategicGoal::LocalDefense, input),
        goal_trace(StrategicGoal::FrontalAttack, input),
    ];
    goals.sort_by_key(|goal| goal.goal);
    ManagerOutputTrace {
        profile_id: input.profile.id,
        tick: input.observation.tick,
        goals,
        commands: input.command_trace.to_vec(),
        budget: BudgetTrace::new(input.start_budget, input.end_budget),
        reservations: input.reservations.into(),
    }
}

fn goal_trace(goal: StrategicGoal, input: TraceInput<'_>) -> GoalTrace {
    let intents = match goal {
        StrategicGoal::LocalDefense if !input.local_threat_active => Vec::new(),
        _ => intents_for_goal(goal, input.intents),
    };
    let selected = match goal {
        StrategicGoal::Expansion => has_city_centre_intent(input.intents),
        StrategicGoal::Tech => input.intents.iter().any(|intent| {
            matches!(intent, AiIntent::Build { kind } if input.required_tech_path.contains(kind))
                || matches!(intent, AiIntent::Research { .. })
        }),
        StrategicGoal::Production => input.intents.iter().any(|intent| {
            matches!(
                intent,
                AiIntent::Train { kind } if *kind != EntityKind::Worker
            ) || matches!(intent, AiIntent::Research { .. })
        }),
        StrategicGoal::Economy => input.intents.iter().any(|intent| {
            matches!(
                intent,
                AiIntent::Train {
                    kind: EntityKind::Worker
                }
            ) || matches!(intent, AiIntent::Gather { .. })
        }),
        StrategicGoal::LocalDefense => {
            input.local_threat_active
                && input
                    .intents
                    .iter()
                    .any(|intent| matches!(intent, AiIntent::Attack { .. }))
        }
        StrategicGoal::FrontalAttack => input
            .intents
            .iter()
            .any(|intent| matches!(intent, AiIntent::Attack { .. } | AiIntent::Stage { .. })),
    };
    let mut blockers = blockers_for_goal(goal, input, selected);
    blockers.sort();
    blockers.dedup();
    GoalTrace {
        goal,
        status: if selected {
            GoalStatus::Selected
        } else {
            GoalStatus::Skipped
        },
        blockers,
        intents,
    }
}

fn blockers_for_goal(
    goal: StrategicGoal,
    input: TraceInput<'_>,
    selected: bool,
) -> Vec<GoalBlocker> {
    if selected {
        if goal == StrategicGoal::FrontalAttack
            && input
                .intents
                .iter()
                .any(|intent| matches!(intent, AiIntent::Stage { .. }))
        {
            let mut blockers: Vec<GoalBlocker> = input
                .frontal_wave_blockers
                .iter()
                .map(frontal_wave_blocker_trace)
                .collect();
            blockers.push(GoalBlocker::Staging);
            return blockers;
        }
        return Vec::new();
    }
    let mut blockers = Vec::new();
    match goal {
        StrategicGoal::Expansion => {
            if !input.expansion_blockers.is_empty() {
                blockers.extend(input.expansion_blockers.iter().map(expansion_blocker_trace));
            } else if input.save_for_expansion {
                push_budget_blockers(&mut blockers, input.end_budget, EntityKind::CityCentre);
                if input.facts.available_builder_count() == 0 {
                    blockers.push(GoalBlocker::NoBuilder);
                }
            }
        }
        StrategicGoal::Tech => {
            if input.defensive_panic_active {
                blockers.push(GoalBlocker::DefensivePanic);
            }
            if input.expansion_blocks_tech_path || input.save_for_unplanned_expansion {
                blockers.push(GoalBlocker::DeferredForExpansion);
            }
            if input.required_tech_path.is_empty() {
                blockers.push(GoalBlocker::MissingPrerequisite("no_required_tech_path"));
            }
        }
        StrategicGoal::Production => {
            if input.save_worker_training_for_tech {
                blockers.push(GoalBlocker::DeferredForTech);
            }
            if input.facts.production_building_count() == 0 {
                blockers.push(GoalBlocker::NoProductionBuilding);
            }
            if input.end_budget.free_supply() == 0 {
                blockers.push(GoalBlocker::SupplyCap);
            }
        }
        StrategicGoal::Economy => {
            if input.save_worker_training_for_tech {
                blockers.push(GoalBlocker::DeferredForTech);
            }
            if input.facts.production_building_count() == 0 {
                blockers.push(GoalBlocker::NoProductionBuilding);
            }
        }
        StrategicGoal::LocalDefense => {
            if !input.local_threat_active {
                blockers.push(GoalBlocker::MissingPrerequisite("no_local_threat"));
            } else if input.ready_units == 0 {
                blockers.push(GoalBlocker::NoReadyUnits);
            }
        }
        StrategicGoal::FrontalAttack => blockers.extend(
            input
                .frontal_wave_blockers
                .iter()
                .map(frontal_wave_blocker_trace),
        ),
    }
    if blockers.is_empty() {
        blockers.push(GoalBlocker::MissingPrerequisite("not_due"));
    }
    blockers
}

fn expansion_blocker_trace(blocker: &ExpansionBlocker) -> GoalBlocker {
    match blocker {
        ExpansionBlocker::NotDue => GoalBlocker::MissingPrerequisite("expansion_not_due"),
        ExpansionBlocker::DefensivePanic => GoalBlocker::DefensivePanic,
        ExpansionBlocker::MissingRequiredBuilding => {
            GoalBlocker::MissingPrerequisite("expansion_required_building")
        }
        ExpansionBlocker::MissingDefensiveUnits => {
            GoalBlocker::MissingPrerequisite("expansion_defenders")
        }
        ExpansionBlocker::RequirementNotMet => {
            GoalBlocker::MissingPrerequisite("city_centre_requirement")
        }
        ExpansionBlocker::AlreadyAtTarget => GoalBlocker::MissingPrerequisite("expansion_done"),
        ExpansionBlocker::MaxPending => GoalBlocker::MissingPrerequisite("expansion_pending"),
        ExpansionBlocker::NoCandidateResources => {
            GoalBlocker::MissingPrerequisite("no_expansion_resources")
        }
        ExpansionBlocker::NoValidSite => GoalBlocker::MissingPrerequisite("no_expansion_site"),
    }
}

fn frontal_wave_blocker_trace(blocker: &FrontalWaveBlocker) -> GoalBlocker {
    match blocker {
        FrontalWaveBlocker::WaitingForUnits => GoalBlocker::WaitingForUnits,
        FrontalWaveBlocker::WaitingForTank => GoalBlocker::WaitingForTank,
        FrontalWaveBlocker::WaitingForMethamphetamines => GoalBlocker::WaitingForMethamphetamines,
        FrontalWaveBlocker::Staging => GoalBlocker::Staging,
        FrontalWaveBlocker::AttackCadence => GoalBlocker::AttackCadence,
    }
}

fn push_budget_blockers(blockers: &mut Vec<GoalBlocker>, budget: SpendBudget, kind: EntityKind) {
    let (steel, oil) = rts_rules::economy::cost(kind);
    if budget.steel() < steel {
        blockers.push(GoalBlocker::BudgetSteel);
    }
    if budget.oil() < oil {
        blockers.push(GoalBlocker::BudgetOil);
    }
}

fn intents_for_goal(goal: StrategicGoal, intents: &[AiIntent]) -> Vec<String> {
    intents
        .iter()
        .filter(|intent| intent_matches_goal(goal, intent))
        .map(format_intent)
        .collect()
}

fn intent_matches_goal(goal: StrategicGoal, intent: &AiIntent) -> bool {
    match goal {
        StrategicGoal::Economy => matches!(
            intent,
            AiIntent::Train {
                kind: EntityKind::Worker
            } | AiIntent::Gather { .. }
        ),
        StrategicGoal::Expansion => {
            matches!(
                intent,
                AiIntent::Build {
                    kind: EntityKind::CityCentre
                } | AiIntent::ResumeConstruction {
                    kind: EntityKind::CityCentre
                }
            )
        }
        StrategicGoal::Tech => matches!(intent, AiIntent::Build { .. } | AiIntent::Research { .. }),
        StrategicGoal::Production => {
            matches!(intent, AiIntent::Train { .. } | AiIntent::Research { .. })
        }
        StrategicGoal::LocalDefense | StrategicGoal::FrontalAttack => {
            matches!(intent, AiIntent::Attack { .. } | AiIntent::Stage { .. })
        }
    }
}

fn format_intent(intent: &AiIntent) -> String {
    match intent {
        AiIntent::Move { units } => format!("move:{}", units.len()),
        AiIntent::Build { kind } => format!("build:{kind:?}"),
        AiIntent::ResumeConstruction { kind } => format!("resume:{kind:?}"),
        AiIntent::Train { kind } => format!("train:{kind:?}"),
        AiIntent::Research { upgrade } => format!("research:{upgrade:?}"),
        AiIntent::Gather {
            resource,
            assignments,
        } => format!("gather:{resource:?}:{assignments}"),
        AiIntent::Stage { units } => format!("stage:{}", units.len()),
        AiIntent::Attack { units } => format!("attack:{}", units.len()),
    }
}

fn has_build_intent(intents: &[AiIntent], kind: EntityKind) -> bool {
    intents
        .iter()
        .any(|intent| matches!(intent, AiIntent::Build { kind: built } if *built == kind))
}

fn has_city_centre_intent(intents: &[AiIntent]) -> bool {
    intents.iter().any(|intent| {
        matches!(
            intent,
            AiIntent::Build {
                kind: EntityKind::CityCentre
            } | AiIntent::ResumeConstruction {
                kind: EntityKind::CityCentre
            }
        )
    })
}

fn format_blockers(blockers: &[GoalBlocker]) -> String {
    if blockers.is_empty() {
        return "-".to_string();
    }
    blockers
        .iter()
        .map(|blocker| format!("{blocker:?}"))
        .collect::<Vec<_>>()
        .join(",")
}
