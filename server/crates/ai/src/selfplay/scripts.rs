use std::collections::BTreeSet;

use super::player_view::PlayerView;
use super::SYNTHETIC_SCRIPT_THINK_INTERVAL;
use crate::ai_core::actions::{self, AiActionContext, ResourceAssignmentPolicy, SpendBudget};
use crate::ai_core::facts::AiFacts;
use crate::ai_core::observation::AiObservation;
use crate::ai_core::profiles::AI_2_1_ID;
use crate::ai_core::resource_availability::ResourceAvailability;
use crate::live::{AiController, AiThinkContext, CanonicalAiTickDriver};
use rts_sim::game::command::SimCommand as Command;
use rts_sim::game::entity::EntityKind;

pub(super) trait ScriptedPlayer: Send {
    fn player_id(&self) -> u32;
    fn name(&self) -> &'static str;
    fn commands(&mut self, view: PlayerView<'_>, retreat_commands: Vec<Command>) -> Vec<Command>;
    fn last_trace_lines(&self) -> Option<&[String]> {
        None
    }
}

pub(super) struct ProfileBackedScript {
    controller: AiController,
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
        Self {
            controller: AiController::with_profile_id(player_id, profile_id),
            allow_combat_commands,
            script_name,
            last_trace_lines: None,
        }
    }
}

impl ScriptedPlayer for ProfileBackedScript {
    fn player_id(&self) -> u32 {
        self.controller.player_id()
    }

    fn name(&self) -> &'static str {
        self.script_name
    }

    fn commands(&mut self, view: PlayerView<'_>, retreat_commands: Vec<Command>) -> Vec<Command> {
        self.last_trace_lines = None;
        let retreat_count = retreat_commands.len();
        let (mut commands, trace) = CanonicalAiTickDriver::run_prepared_controller(
            &mut self.controller,
            AiThinkContext {
                start: view.start,
                snapshot: view.snapshot,
                alive_player_ids: view.alive_player_ids,
                retreat_commands,
            },
        );
        self.last_trace_lines = trace.map(|trace| trace.lines);
        if !self.allow_combat_commands {
            let strategic_commands = commands.split_off(retreat_count.min(commands.len()));
            commands.extend(
                strategic_commands
                    .into_iter()
                    .filter(|command| !is_combat_command(command)),
            );
        }
        commands
    }

    fn last_trace_lines(&self) -> Option<&[String]> {
        self.last_trace_lines.as_deref()
    }
}

fn is_combat_command(command: &Command) -> bool {
    match command {
        Command::Attack { .. }
        | Command::AttackTankTrapCluster { .. }
        | Command::AttackMove { .. }
        | Command::Move { .. }
        | Command::SetupAntiTankGuns { .. }
        | Command::TearDownAntiTankGuns { .. }
        | Command::UseAbility { .. }
        | Command::ArtilleryFire { .. }
        | Command::RecastAbility { .. }
        | Command::SetAutocast { .. } => true,
        Command::FormationMove { .. } | Command::HoldPosition { .. } => true,
        Command::Gather { .. }
        | Command::Build { .. }
        | Command::Deconstruct { .. }
        | Command::Train { .. }
        | Command::AdjustProductionRepeat { .. }
        | Command::SetAutoBuildSettings { .. }
        | Command::Research { .. }
        | Command::Cancel { .. }
        | Command::Stop { .. }
        | Command::SetRally { .. }
        | Command::Rejected { .. } => false,
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
                .is_multiple_of(SYNTHETIC_SCRIPT_THINK_INTERVAL)
    }
}

impl ScriptedPlayer for MineOnlyScript {
    fn player_id(&self) -> u32 {
        self.player_id
    }

    fn name(&self) -> &'static str {
        "mine-only"
    }

    fn commands(&mut self, view: PlayerView<'_>, _retreat_commands: Vec<Command>) -> Vec<Command> {
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
