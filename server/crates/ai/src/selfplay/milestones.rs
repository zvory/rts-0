use std::collections::BTreeMap;

use serde::Serialize;

use super::player_view::{is_complete, kind_of};
use crate::config;
use rts_sim::game::command::SimCommand as Command;
use rts_sim::game::entity::EntityKind;
use rts_sim::protocol::{states, Event, Snapshot};

pub(super) struct AttackerInfo {
    pub(super) owner: u32,
    pub(super) kind: EntityKind,
}

#[derive(Clone, Serialize)]
pub(super) struct SnapshotSample {
    pub(super) tick: u32,
    player_id: u32,
    steel: u32,
    oil: u32,
    supply_used: u32,
    supply_cap: u32,
    own_counts: BTreeMap<String, u32>,
    visible_entities: u32,
    damaged_own_entities: u32,
}

impl SnapshotSample {
    pub(super) fn from_snapshot(tick: u32, player_id: u32, snapshot: &Snapshot) -> Self {
        let mut own_counts = BTreeMap::new();
        let mut damaged_own_entities = 0;
        for e in snapshot.entities.iter().filter(|e| e.owner == player_id) {
            *own_counts.entry(e.kind.clone()).or_insert(0) += 1;
            if e.hp < e.max_hp {
                damaged_own_entities += 1;
            }
        }

        SnapshotSample {
            tick,
            player_id,
            steel: snapshot.steel,
            oil: snapshot.oil,
            supply_used: snapshot.supply_used,
            supply_cap: snapshot.supply_cap,
            own_counts,
            visible_entities: snapshot.entities.len() as u32,
            damaged_own_entities,
        }
    }
}

#[derive(Clone, Serialize)]
pub(super) struct Milestones {
    pub(super) players: BTreeMap<u32, PlayerMilestones>,
    pub(super) goals: BTreeMap<u32, PlayerMilestoneGoal>,
    pub(super) combat_goal: CombatGoal,
    pub(super) attack_events: u32,
    pub(super) death_events: u32,
    pub(super) attack_events_by_player: BTreeMap<u32, u32>,
    pub(super) worker_attack_events_by_player: BTreeMap<u32, u32>,
    pub(super) first_damage_tick: Option<u32>,
    pub(super) first_damage_tick_by_attacker: BTreeMap<u32, u32>,
}

impl Milestones {
    pub(super) fn tech_combat_for_players(ids: impl Iterator<Item = u32>) -> Self {
        Milestones::with_goals(
            ids.map(|id| (id, PlayerMilestoneGoal::tech_combat())),
            CombatGoal::any_combat(),
        )
    }

    pub(super) fn with_goals(
        goals: impl IntoIterator<Item = (u32, PlayerMilestoneGoal)>,
        combat_goal: CombatGoal,
    ) -> Self {
        let goals: BTreeMap<u32, PlayerMilestoneGoal> = goals.into_iter().collect();
        Milestones {
            players: goals
                .keys()
                .copied()
                .map(|id| (id, PlayerMilestones::default()))
                .collect(),
            goals,
            combat_goal,
            attack_events: 0,
            death_events: 0,
            attack_events_by_player: BTreeMap::new(),
            worker_attack_events_by_player: BTreeMap::new(),
            first_damage_tick: None,
            first_damage_tick_by_attacker: BTreeMap::new(),
        }
    }

    pub(super) fn observe_snapshots(
        &mut self,
        tick: u32,
        snapshots: &BTreeMap<u32, Snapshot>,
        resource_kinds: &BTreeMap<u32, EntityKind>,
    ) -> bool {
        let mut changed = false;
        for (player_id, snapshot) in snapshots {
            if let Some(player) = self.players.get_mut(player_id) {
                changed |= player.observe(tick, *player_id, snapshot, resource_kinds);
            }
        }
        changed
    }

    pub(super) fn observe_command(&mut self, tick: u32, player_id: u32, command: &Command) -> bool {
        let Some(player) = self.players.get_mut(&player_id) else {
            return false;
        };
        let Some(goal) = self.goals.get(&player_id) else {
            return false;
        };
        player.observe_command(tick, goal, command)
    }

    pub(super) fn observe_combat_event(
        &mut self,
        tick: u32,
        player_id: u32,
        attacker: Option<AttackerInfo>,
        event: &Event,
    ) -> bool {
        let before_damage_tick = self.first_damage_tick;
        let changed = match event {
            Event::Attack { .. } => {
                self.attack_events += 1;
                self.first_damage_tick.get_or_insert(tick);
                if let Some(attacker) = attacker {
                    *self
                        .attack_events_by_player
                        .entry(attacker.owner)
                        .or_default() += 1;
                    self.first_damage_tick_by_attacker
                        .entry(attacker.owner)
                        .or_insert(tick);
                    if attacker.kind == EntityKind::Worker {
                        *self
                            .worker_attack_events_by_player
                            .entry(attacker.owner)
                            .or_default() += 1;
                    }
                } else {
                    *self.attack_events_by_player.entry(player_id).or_default() += 1;
                }
                true
            }
            Event::Death { .. } => {
                self.death_events += 1;
                true
            }
            _ => false,
        };
        changed || before_damage_tick != self.first_damage_tick
    }

    pub(super) fn complete(&self) -> bool {
        let players_complete = self
            .goals
            .iter()
            .all(|(player_id, goal)| self.players[player_id].complete_for(goal));
        players_complete && self.combat_goal.complete(self)
    }

    pub(super) fn missing_summary(&self) -> String {
        let mut missing = Vec::new();
        for (player_id, goal) in &self.goals {
            if let Some(player) = self.players.get(player_id) {
                for item in player.missing_for(goal) {
                    missing.push(format!("p{player_id}:{item}"));
                }
            }
        }
        for item in self.combat_goal.missing(self) {
            missing.push(item);
        }
        missing.join(", ")
    }
}

#[derive(Clone, Default, Serialize)]
pub(super) struct CombatGoal {
    require_any_combat: bool,
    require_damage: bool,
    min_attacks_by_player: BTreeMap<u32, u32>,
    min_worker_attacks_by_player: BTreeMap<u32, u32>,
}

impl CombatGoal {
    pub(super) fn any_combat() -> Self {
        CombatGoal {
            require_any_combat: true,
            ..CombatGoal::default()
        }
    }

    pub(super) fn damage() -> Self {
        CombatGoal {
            require_damage: true,
            ..CombatGoal::default()
        }
    }

    pub(super) fn worker_attack_by(player_id: u32) -> Self {
        CombatGoal {
            min_worker_attacks_by_player: BTreeMap::from([(player_id, 1)]),
            ..CombatGoal::default()
        }
    }

    fn complete(&self, milestones: &Milestones) -> bool {
        if self.require_any_combat && milestones.attack_events == 0 && milestones.death_events == 0
        {
            return false;
        }
        if self.require_damage && milestones.first_damage_tick.is_none() {
            return false;
        }
        for (player_id, required) in &self.min_attacks_by_player {
            if milestones
                .attack_events_by_player
                .get(player_id)
                .copied()
                .unwrap_or(0)
                < *required
            {
                return false;
            }
        }
        for (player_id, required) in &self.min_worker_attacks_by_player {
            if milestones
                .worker_attack_events_by_player
                .get(player_id)
                .copied()
                .unwrap_or(0)
                < *required
            {
                return false;
            }
        }
        true
    }

    fn missing(&self, milestones: &Milestones) -> Vec<String> {
        let mut out = Vec::new();
        if self.require_any_combat && milestones.attack_events == 0 && milestones.death_events == 0
        {
            out.push("combat-event".to_string());
        }
        if self.require_damage && milestones.first_damage_tick.is_none() {
            out.push("damage-event".to_string());
        }
        for (player_id, required) in &self.min_attacks_by_player {
            let seen = milestones
                .attack_events_by_player
                .get(player_id)
                .copied()
                .unwrap_or(0);
            if seen < *required {
                out.push(format!("p{player_id}:attack-events>={required}"));
            }
        }
        for (player_id, required) in &self.min_worker_attacks_by_player {
            let seen = milestones
                .worker_attack_events_by_player
                .get(player_id)
                .copied()
                .unwrap_or(0);
            if seen < *required {
                out.push(format!("p{player_id}:worker-attacks>={required}"));
            }
        }
        out
    }
}

#[derive(Clone, Default, Serialize)]
pub(super) struct PlayerMilestoneGoal {
    pub(super) require_gathering: bool,
    pub(super) require_oil: bool,
    pub(super) require_oil_extractor_assignment: bool,
    pub(super) require_barracks_complete: bool,
    pub(super) require_rifleman: bool,
    pub(super) require_tank: bool,
    pub(super) require_damage_taken: bool,
    pub(super) allow_elimination_before_milestones: bool,
    pub(super) min_workers: u32,
    pub(super) min_supply_cap: u32,
    pub(super) min_attack_command_units: u32,
    pub(super) min_units_by_kind: BTreeMap<&'static str, u32>,
    pub(super) min_buildings_by_kind: BTreeMap<&'static str, u32>,
}

impl PlayerMilestoneGoal {
    pub(super) fn tech_combat() -> Self {
        PlayerMilestoneGoal {
            require_gathering: true,
            require_oil: true,
            require_barracks_complete: true,
            require_rifleman: true,
            require_tank: true,
            ..PlayerMilestoneGoal::default()
        }
    }

    pub(super) fn damaged_economy() -> Self {
        PlayerMilestoneGoal {
            require_gathering: true,
            require_damage_taken: true,
            min_workers: config::STARTING_WORKERS + 2,
            ..PlayerMilestoneGoal::default()
        }
    }

    pub(super) fn with_min_workers(mut self, min_workers: u32) -> Self {
        self.min_workers = min_workers;
        self
    }

    pub(super) fn with_min_supply_cap(mut self, min_supply_cap: u32) -> Self {
        self.min_supply_cap = min_supply_cap;
        self
    }

    pub(super) fn with_min_attack_command_units(mut self, min_units: u32) -> Self {
        self.min_attack_command_units = min_units;
        self
    }

    pub(super) fn with_min_units(mut self, kind: &'static str, count: u32) -> Self {
        self.min_units_by_kind.insert(kind, count);
        self
    }

    pub(super) fn with_min_buildings(mut self, kind: &'static str, count: u32) -> Self {
        self.min_buildings_by_kind.insert(kind, count);
        self
    }

    pub(super) fn allowing_elimination_before_milestones(mut self) -> Self {
        self.allow_elimination_before_milestones = true;
        self
    }
}

#[derive(Clone, Default, PartialEq, Serialize)]
pub(super) struct PlayerMilestones {
    saw_owned_entities: bool,
    pub(super) eliminated: bool,
    saw_gathering: bool,
    oil_gathered: bool,
    pub(super) oil_extractor_started: bool,
    barracks_started: bool,
    barracks_complete: bool,
    rifleman_trained: bool,
    tank_trained: bool,
    damage_taken: bool,
    first_attack_command_tick: Option<u32>,
    pub(super) first_goal_attack_command_tick: Option<u32>,
    pub(super) first_tank_tick: Option<u32>,
    first_damage_tick: Option<u32>,
    pub(super) max_workers: u32,
    max_steel: u32,
    max_oil: u32,
    max_supply_cap: u32,
    max_riflemen: u32,
    max_tanks: u32,
    pub(super) max_units_by_kind: BTreeMap<String, u32>,
    max_buildings_by_kind: BTreeMap<String, u32>,
}

impl PlayerMilestones {
    fn observe(
        &mut self,
        tick: u32,
        player_id: u32,
        snapshot: &Snapshot,
        _resource_kinds: &BTreeMap<u32, EntityKind>,
    ) -> bool {
        let before = self.clone();
        let mut workers = 0;
        let mut riflemen = 0;
        let mut tanks = 0;
        let mut owned_entities = 0;
        let mut owned_buildings = 0;
        let mut units_by_kind = BTreeMap::<String, u32>::new();
        let mut buildings_by_kind = BTreeMap::<String, u32>::new();
        for e in snapshot.entities.iter().filter(|e| e.owner == player_id) {
            owned_entities += 1;
            let Some(k) = kind_of(e) else { continue };
            if k.is_unit() {
                *units_by_kind.entry(e.kind.clone()).or_default() += 1;
            }
            if k.is_building() {
                owned_buildings += 1;
                *buildings_by_kind.entry(e.kind.clone()).or_default() += 1;
            }
            match k {
                EntityKind::Worker => {
                    workers += 1;
                    if e.state == states::GATHER || e.latched_node.is_some() {
                        self.saw_gathering = true;
                    }
                }
                EntityKind::Rifleman => riflemen += 1,
                EntityKind::Tank => tanks += 1,
                EntityKind::PumpJack => self.oil_extractor_started = true,
                EntityKind::Barracks => {
                    self.barracks_started = true;
                    if is_complete(e) {
                        self.barracks_complete = true;
                    }
                }
                _ => {}
            }
            if e.hp < e.max_hp {
                self.damage_taken = true;
                self.first_damage_tick.get_or_insert(tick);
            }
        }
        if owned_entities > 0 {
            self.saw_owned_entities = true;
        }
        if self.saw_owned_entities && owned_buildings == 0 {
            self.eliminated = true;
        }
        self.oil_gathered |= snapshot.oil > 0;
        self.max_workers = self.max_workers.max(workers);
        self.max_steel = self.max_steel.max(snapshot.steel);
        self.max_oil = self.max_oil.max(snapshot.oil);
        self.max_supply_cap = self.max_supply_cap.max(snapshot.supply_cap);
        self.max_riflemen = self.max_riflemen.max(riflemen);
        self.max_tanks = self.max_tanks.max(tanks);
        for (kind, count) in units_by_kind {
            self.max_units_by_kind
                .entry(kind)
                .and_modify(|max| *max = (*max).max(count))
                .or_insert(count);
        }
        for (kind, count) in buildings_by_kind {
            self.max_buildings_by_kind
                .entry(kind)
                .and_modify(|max| *max = (*max).max(count))
                .or_insert(count);
        }
        self.rifleman_trained |= riflemen > 0;
        if tanks > 0 {
            self.tank_trained = true;
            self.first_tank_tick.get_or_insert(tick);
        }
        before != *self
    }

    fn observe_command(
        &mut self,
        tick: u32,
        goal: &PlayerMilestoneGoal,
        command: &Command,
    ) -> bool {
        let before = self.clone();
        let attack_units = match command {
            Command::AttackMove { units, .. }
            | Command::Attack { units, .. }
            | Command::AttackTankTrapCluster { units, .. }
            | Command::FormationMove {
                units,
                attack_move: true,
                ..
            } => Some(units.len() as u32),
            Command::Move { units, .. }
            | Command::FormationMove {
                units,
                attack_move: false,
                ..
            } if self.rifleman_trained => Some(units.len() as u32),
            Command::Move { .. }
            | Command::FormationMove {
                attack_move: false, ..
            }
            | Command::SetupAntiTankGuns { .. }
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
            | Command::Rejected { .. } => None,
        };
        if let Some(attack_units) = attack_units {
            self.first_attack_command_tick.get_or_insert(tick);
            if goal.min_attack_command_units > 0 && attack_units >= goal.min_attack_command_units {
                self.first_goal_attack_command_tick.get_or_insert(tick);
            }
        }
        before != *self
    }

    fn complete_for(&self, goal: &PlayerMilestoneGoal) -> bool {
        self.missing_for(goal).is_empty()
    }

    pub(super) fn missing_for(&self, goal: &PlayerMilestoneGoal) -> Vec<String> {
        if goal.allow_elimination_before_milestones && self.eliminated {
            return Vec::new();
        }

        let mut out = Vec::new();
        if goal.require_gathering && !self.saw_gathering {
            out.push("economy-gather".to_string());
        }
        if goal.require_oil && !self.oil_gathered {
            out.push("oil-gather".to_string());
        }
        if goal.require_oil_extractor_assignment && !self.oil_extractor_started {
            out.push("oil-extractor".to_string());
        }
        if goal.require_barracks_complete && !self.barracks_complete {
            out.push("barracks".to_string());
        }
        if goal.require_rifleman && !self.rifleman_trained {
            out.push("rifleman".to_string());
        }
        if goal.require_tank && !self.tank_trained {
            out.push("tank".to_string());
        }
        if goal.require_damage_taken && !self.damage_taken {
            out.push("damage-taken".to_string());
        }
        if self.max_workers < goal.min_workers {
            out.push(format!("workers>={}", goal.min_workers));
        }
        if self.max_supply_cap < goal.min_supply_cap {
            out.push(format!("supply-cap>={}", goal.min_supply_cap));
        }
        if goal.min_attack_command_units > 0 && self.first_goal_attack_command_tick.is_none() {
            out.push(format!(
                "attack-command-units>={}",
                goal.min_attack_command_units
            ));
        }
        for (kind, required) in &goal.min_units_by_kind {
            let seen = self
                .max_units_by_kind
                .get(*kind)
                .copied()
                .unwrap_or_default();
            if seen < *required {
                out.push(format!("{kind}>={required}"));
            }
        }
        for (kind, required) in &goal.min_buildings_by_kind {
            let seen = self
                .max_buildings_by_kind
                .get(*kind)
                .copied()
                .unwrap_or_default();
            if seen < *required {
                out.push(format!("{kind}>={required}"));
            }
        }
        out
    }
}
