use std::collections::{BTreeMap, BTreeSet};

use super::pending_build::PendingBuildTracker;
use super::player_view::{
    footprint_placeable_from_snapshot, is_kind, occupied_tiles_from_snapshot, player_start_world,
    PlayerView,
};
use super::{ATTACK_REISSUE_TICKS, SELFPLAY_ATTACK_STAGE_SUPPRESSION_TICKS, THINK_INTERVAL};
use crate::ai_core::actions::{self, AiActionContext, ResourceAssignmentPolicy, SpendBudget};
use crate::ai_core::decision::{decide_profile_with_analysis, AiDecisionMemory, AiIntent};
use crate::ai_core::facts::AiFacts;
use crate::ai_core::map_analysis::AiStaticMapContextCache;
use crate::ai_core::observation::AiObservation;
use crate::ai_core::profiles::{profile_by_id, AiProfile, AI_2_1, AI_2_1_ID};
use crate::ai_core::resource_availability::ResourceAvailability;
use crate::ai_shared;
use rts_sim::game::command::SimCommand as Command;
use rts_sim::game::entity::EntityKind;

pub(super) trait ScriptedPlayer: Send {
    fn player_id(&self) -> u32;
    fn name(&self) -> &'static str;
    fn commands(&mut self, view: PlayerView<'_>) -> Vec<Command>;
    fn last_trace_lines(&self) -> Option<&[String]> {
        None
    }
}

pub(super) struct ProfileBackedScript {
    player_id: u32,
    profile: &'static AiProfile,
    memory: AiDecisionMemory,
    static_map_context: AiStaticMapContextCache,
    pending_builds: PendingBuildTracker,
    staged_units: BTreeSet<u32>,
    held_stage_units: BTreeSet<u32>,
    active_attack_units: BTreeMap<u32, u32>,
    allow_combat_commands: bool,
    script_name: &'static str,
    last_trace_lines: Option<Vec<String>>,
}

impl ProfileBackedScript {
    pub(super) fn new(player_id: u32, profile_id: &'static str) -> Self {
        Self::with_combat(player_id, profile_id, true, profile_id)
    }

    pub(super) fn economy_only(player_id: u32) -> Self {
        Self::with_combat(player_id, AI_2_1_ID, false, "profile-economy")
    }

    fn with_combat(
        player_id: u32,
        profile_id: &'static str,
        allow_combat_commands: bool,
        script_name: &'static str,
    ) -> Self {
        let profile = profile_by_id(profile_id).unwrap_or(&AI_2_1);
        Self {
            player_id,
            profile,
            memory: AiDecisionMemory::for_profile(profile),
            static_map_context: AiStaticMapContextCache::default(),
            pending_builds: PendingBuildTracker::default(),
            staged_units: BTreeSet::new(),
            held_stage_units: BTreeSet::new(),
            active_attack_units: BTreeMap::new(),
            allow_combat_commands,
            script_name,
            last_trace_lines: None,
        }
    }

    fn should_think(&self, tick: u32) -> bool {
        tick == 0
            || tick
                .wrapping_add(self.player_id)
                .is_multiple_of(THINK_INTERVAL)
    }
}

impl ScriptedPlayer for ProfileBackedScript {
    fn player_id(&self) -> u32 {
        self.player_id
    }

    fn name(&self) -> &'static str {
        self.script_name
    }

    fn commands(&mut self, view: PlayerView<'_>) -> Vec<Command> {
        self.last_trace_lines = None;
        if !self.should_think(view.tick) {
            return Vec::new();
        }

        self.pending_builds.observe(view);
        let Some(observation) = view.observation(self.pending_builds.intents()) else {
            return Vec::new();
        };
        self.prune_combat_memory(&observation, view.tick);

        let occupied = occupied_tiles_from_snapshot(&view.start.map, view.snapshot);
        let failed_builds = &self.pending_builds;
        let map_analysis = self
            .static_map_context
            .get_or_analyze(view.start)
            .analysis();
        let decision = decide_profile_with_analysis(
            &observation,
            self.profile,
            &mut self.memory,
            map_analysis,
            ai_shared::BuildSearch::default(),
            |building, tile_x, tile_y| {
                !failed_builds.failed(building, tile_x, tile_y)
                    && footprint_placeable_from_snapshot(
                        &view.start.map,
                        view.snapshot,
                        building,
                        tile_x,
                        tile_y,
                        &occupied,
                    )
            },
        );
        debug_assert_eq!(decision.profile_id, self.profile.id);

        let combat_intent_units = combat_intent_units(&decision.intents);
        self.last_trace_lines = Some(decision.trace.format_lines());
        let mut commands =
            self.filter_repeated_stage_commands(view.tick, &decision.intents, decision.commands);
        if !self.allow_combat_commands {
            commands.retain(|command| !is_combat_command(command, &combat_intent_units));
        }
        self.pending_builds.record_commands(view.tick, &commands);
        commands
    }

    fn last_trace_lines(&self) -> Option<&[String]> {
        self.last_trace_lines.as_deref()
    }
}

fn combat_intent_units(intents: &[AiIntent]) -> BTreeSet<u32> {
    let mut units = BTreeSet::new();
    for intent in intents {
        if let AiIntent::Attack { units: attacking } = intent {
            units.extend(attacking.iter().copied());
        }
    }
    units
}

pub(super) fn is_combat_command(command: &Command, combat_intent_units: &BTreeSet<u32>) -> bool {
    match command {
        Command::Attack { .. } | Command::AttackMove { .. } => true,
        Command::FormationMove {
            attack_move: true, ..
        } => true,
        Command::Move { units, .. }
        | Command::FormationMove {
            units,
            attack_move: false,
            ..
        } => units.iter().any(|id| combat_intent_units.contains(id)),
        Command::SetupAntiTankGuns { .. }
        | Command::TearDownAntiTankGuns { .. }
        | Command::UseAbility { .. }
        | Command::ArtilleryFire { .. }
        | Command::RecastAbility { .. }
        | Command::SetAutocast { .. }
        | Command::Gather { .. }
        | Command::Build { .. }
        | Command::Deconstruct { .. }
        | Command::Train { .. }
        | Command::AdjustProductionRepeat { .. }
        | Command::Research { .. }
        | Command::Cancel { .. }
        | Command::Stop { .. }
        | Command::HoldPosition { .. }
        | Command::SetRally { .. }
        | Command::Rejected { .. } => false,
    }
}

impl ProfileBackedScript {
    fn prune_combat_memory(&mut self, observation: &AiObservation, tick: u32) {
        let owned: BTreeSet<u32> = observation.owned.iter().map(|entity| entity.id).collect();
        self.staged_units.retain(|id| owned.contains(id));
        self.held_stage_units.retain(|id| owned.contains(id));
        let suppress_ticks = self
            .profile
            .attack
            .reissue_cadence_ticks
            .max(SELFPLAY_ATTACK_STAGE_SUPPRESSION_TICKS);
        self.active_attack_units.retain(|id, issued| {
            owned.contains(id) && tick.saturating_sub(*issued) < suppress_ticks
        });
    }

    fn filter_repeated_stage_commands(
        &mut self,
        tick: u32,
        intents: &[AiIntent],
        commands: Vec<Command>,
    ) -> Vec<Command> {
        let mut attacking = BTreeSet::new();
        let mut staging = BTreeSet::new();
        for intent in intents {
            match intent {
                AiIntent::Attack { units } => attacking.extend(units.iter().copied()),
                AiIntent::Stage { units } => staging.extend(units.iter().copied()),
                AiIntent::Move { .. }
                | AiIntent::Build { .. }
                | AiIntent::ResumeConstruction { .. }
                | AiIntent::Train { .. }
                | AiIntent::Research { .. }
                | AiIntent::Gather { .. } => {}
            }
        }
        for id in &attacking {
            self.staged_units.remove(id);
            self.held_stage_units.remove(id);
            self.active_attack_units.insert(*id, tick);
        }
        if staging.is_empty() {
            return commands;
        }

        let mut filtered = Vec::new();
        let mut freshly_staged = BTreeSet::new();
        let command_stages_units = |units: &[u32]| units.iter().any(|id| staging.contains(id));
        for command in commands {
            match command {
                Command::AttackMove {
                    units,
                    x,
                    y,
                    queued,
                } if command_stages_units(&units) => {
                    let fresh: Vec<u32> = units
                        .into_iter()
                        .filter(|id| !self.staged_units.contains(id))
                        .filter(|id| !self.active_attack_units.contains_key(id))
                        .collect();
                    self.staged_units.extend(fresh.iter().copied());
                    for id in &fresh {
                        self.held_stage_units.remove(id);
                    }
                    freshly_staged.extend(fresh.iter().copied());
                    if !fresh.is_empty() {
                        filtered.push(Command::AttackMove {
                            units: fresh,
                            x,
                            y,
                            queued,
                        });
                    }
                }
                Command::Move {
                    units,
                    x,
                    y,
                    queued,
                } if command_stages_units(&units) => {
                    let fresh: Vec<u32> = units
                        .into_iter()
                        .filter(|id| !self.staged_units.contains(id))
                        .filter(|id| !self.active_attack_units.contains_key(id))
                        .collect();
                    self.staged_units.extend(fresh.iter().copied());
                    for id in &fresh {
                        self.held_stage_units.remove(id);
                    }
                    freshly_staged.extend(fresh.iter().copied());
                    if !fresh.is_empty() {
                        filtered.push(Command::Move {
                            units: fresh,
                            x,
                            y,
                            queued,
                        });
                    }
                }
                Command::HoldPosition { units, queued } if command_stages_units(&units) => {
                    let fresh: Vec<u32> = units
                        .into_iter()
                        .filter(|id| !self.active_attack_units.contains_key(id))
                        .filter(|id| !self.held_stage_units.contains(id))
                        .collect();
                    self.staged_units.extend(fresh.iter().copied());
                    self.held_stage_units.extend(fresh.iter().copied());
                    if !fresh.is_empty() {
                        filtered.push(Command::HoldPosition {
                            units: fresh,
                            queued,
                        });
                    }
                }
                Command::SetupAntiTankGuns {
                    units,
                    x,
                    y,
                    queued,
                } if command_stages_units(&units) => {
                    let fresh: Vec<u32> = units
                        .into_iter()
                        .filter(|id| !self.active_attack_units.contains_key(id))
                        .filter(|id| !self.staged_units.contains(id) || freshly_staged.contains(id))
                        .collect();
                    self.staged_units.extend(fresh.iter().copied());
                    if !fresh.is_empty() {
                        filtered.push(Command::SetupAntiTankGuns {
                            units: fresh,
                            x,
                            y,
                            queued,
                        });
                    }
                }
                other => filtered.push(other),
            }
        }
        filtered
    }
}

pub(super) struct WorkerRushScript {
    player_id: u32,
    target_player_id: u32,
    last_attack_tick: u32,
}

// Intentionally retained as special harness coverage: this is an all-in worker pull, not a normal
// strategy profile.
impl WorkerRushScript {
    pub(super) fn new(player_id: u32, target_player_id: u32) -> Self {
        WorkerRushScript {
            player_id,
            target_player_id,
            last_attack_tick: 0,
        }
    }

    fn should_think(&self, tick: u32) -> bool {
        tick == 0
            || tick
                .wrapping_add(self.player_id)
                .is_multiple_of(THINK_INTERVAL)
    }
}

impl ScriptedPlayer for WorkerRushScript {
    fn player_id(&self) -> u32 {
        self.player_id
    }

    fn name(&self) -> &'static str {
        "worker-rush"
    }

    fn commands(&mut self, view: PlayerView<'_>) -> Vec<Command> {
        if !self.should_think(view.tick) {
            return Vec::new();
        }
        let workers: Vec<u32> = view
            .snapshot
            .entities
            .iter()
            .filter(|e| e.owner == view.player_id && is_kind(e, EntityKind::Worker))
            .map(|e| e.id)
            .collect();
        if workers.is_empty() {
            return Vec::new();
        }
        let attack_due = view.tick == 0
            || view.tick.saturating_sub(self.last_attack_tick) >= ATTACK_REISSUE_TICKS;
        if !attack_due {
            return Vec::new();
        }
        let Some((x, y)) = player_start_world(view.start, self.target_player_id) else {
            return Vec::new();
        };
        let Some(observation) = view.observation([]) else {
            return Vec::new();
        };
        let facts = AiFacts::from_observation(&observation);
        let mut actions = AiActionContext::new(
            &facts,
            SpendBudget::new(
                view.snapshot.steel,
                view.snapshot.oil,
                view.snapshot.supply_used,
                view.snapshot.supply_cap,
            ),
        );
        self.last_attack_tick = view.tick;
        actions::attack_move_units(&mut actions, workers, x, y);
        actions.into_commands()
    }
}

fn assign_steel_workers(
    observation: &AiObservation,
    actions: &mut AiActionContext<'_>,
    initial_gather_sent: bool,
) {
    let has_steel = observation
        .resources
        .iter()
        .any(|node| node.kind == EntityKind::Steel && node.remaining > 0);
    if !has_steel {
        return;
    }
    let latched_nodes: BTreeSet<u32> = observation
        .owned
        .iter()
        .filter_map(|worker| worker.latched_node)
        .collect();
    let skipped_workers = BTreeSet::new();
    let availability = ResourceAvailability::from_observation(observation, &latched_nodes);
    let mineable_steel_nodes = availability.free_mineable_node_ids(EntityKind::Steel);
    actions::assign_workers_to_resource(
        actions,
        ResourceAssignmentPolicy {
            workers: &observation.owned,
            resources: &observation.resources,
            resource_kind: EntityKind::Steel,
            assignable_node_ids: &mineable_steel_nodes,
            candidate_worker_ids: None,
            skip_workers: &skipped_workers,
            pre_reserved_nodes: &latched_nodes,
            idle_only: initial_gather_sent,
            allow_latched_reassignment: false,
            max_assignments: None,
            max_worker_resource_distance_px: None,
            remote_worker_assignment_fallback: false,
        },
    );
}

pub(super) struct MineOnlyScript {
    player_id: u32,
    initial_gather_sent: bool,
}

impl MineOnlyScript {
    pub(super) fn new(player_id: u32) -> Self {
        MineOnlyScript {
            player_id,
            initial_gather_sent: false,
        }
    }

    fn should_think(&self, tick: u32) -> bool {
        tick == 0
            || tick
                .wrapping_add(self.player_id)
                .is_multiple_of(THINK_INTERVAL)
    }
}

impl ScriptedPlayer for MineOnlyScript {
    fn player_id(&self) -> u32 {
        self.player_id
    }

    fn name(&self) -> &'static str {
        "mine-only"
    }

    fn commands(&mut self, view: PlayerView<'_>) -> Vec<Command> {
        if !self.should_think(view.tick) {
            return Vec::new();
        }

        let Some(observation) = view.observation([]) else {
            return Vec::new();
        };
        let facts = AiFacts::from_observation(&observation);
        let mut actions = AiActionContext::new(
            &facts,
            SpendBudget::new(
                view.snapshot.steel,
                view.snapshot.oil,
                view.snapshot.supply_used,
                view.snapshot.supply_cap,
            ),
        );
        assign_steel_workers(&observation, &mut actions, self.initial_gather_sent);
        self.initial_gather_sent = true;
        actions.into_commands()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_stage_filter_sends_hold_position_once_per_staged_unit() {
        let mut script = ProfileBackedScript::new(1, AI_2_1_ID);
        let intents = [AiIntent::Stage { units: vec![42] }];
        let hold = Command::HoldPosition {
            units: vec![42],
            queued: false,
        };

        let first = script.filter_repeated_stage_commands(10, &intents, vec![hold.clone()]);
        let second = script.filter_repeated_stage_commands(16, &intents, vec![hold]);

        assert_eq!(
            first,
            vec![Command::HoldPosition {
                units: vec![42],
                queued: false,
            }]
        );
        assert!(second.is_empty());
    }
}
