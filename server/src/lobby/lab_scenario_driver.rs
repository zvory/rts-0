use std::collections::{BTreeMap, BTreeSet};

use super::lab_replay_operations::lab_op_to_replay_operation;
use crate::protocol::{Command, LabReplayOperation, LabReplayOperationEntry};
use crate::tools::hellhole_spec::{
    composition_300_supply, hash_words, respawn_candidates, shuttle_endpoint,
    COMMAND_INTERVAL_TICKS, LEG_TICKS, SCENARIO_ID, SHUTTLE_SELECTION_COUNT, TILE,
};
use rts_sim::game::entity::EntityKind;
use rts_sim::game::lab::{LabCommandOptions, LabOp};
use rts_sim::game::Game;

const MAX_ACTIONS_PER_TICK: usize = 16;
const CORRIDOR_ALONG_RADIUS_TILES: i32 = 5;
const CORRIDOR_LATERAL_RADIUS_TILES: i32 = 2;

pub(crate) fn lab_scenario_driver_for(scenario_id: &str) -> Option<LabScenarioDriver> {
    (scenario_id == SCENARIO_ID)
        .then(LabScenarioDriver::supply_300_hellhole)
        .and_then(Result::ok)
}

pub(crate) struct LabScenarioDriver {
    shuttles: Vec<DiagonalShuttle>,
    target_composition: Vec<EntityKind>,
    respawn_candidates: Vec<(f32, f32)>,
    central_unit_provenance: BTreeMap<u32, (u32, EntityKind)>,
    active_central_unit_ids: BTreeSet<u32>,
    reset_roster_on_next_tick: bool,
    scheduled_actions: Vec<ScheduledAction>,
    last_processed_tick: Option<u32>,
    retained_entries_at_tick: Vec<LabReplayOperationEntry>,
}

impl LabScenarioDriver {
    fn supply_300_hellhole() -> Result<Self, String> {
        Ok(Self {
            shuttles: vec![
                DiagonalShuttle { player_id: 3 },
                DiagonalShuttle { player_id: 4 },
            ],
            target_composition: composition_300_supply()?,
            respawn_candidates: respawn_candidates(),
            central_unit_provenance: BTreeMap::new(),
            active_central_unit_ids: BTreeSet::new(),
            reset_roster_on_next_tick: true,
            scheduled_actions: Vec::new(),
            last_processed_tick: None,
            retained_entries_at_tick: Vec::new(),
        })
    }

    pub(crate) fn actions_for_tick(&mut self, game: &Game) -> Vec<LabScenarioAction> {
        let tick = game.tick_count();
        if self.last_processed_tick == Some(tick) {
            return Vec::new();
        }
        self.last_processed_tick = Some(tick);

        let mut actions: Vec<_> = self
            .scheduled_actions
            .iter()
            .filter(|scheduled| scheduled.tick == tick)
            .map(|scheduled| scheduled.action.clone())
            .collect();
        if let Some(action) = self.respawn_action(game, tick) {
            actions.push(action);
        }
        if tick.is_multiple_of(COMMAND_INTERVAL_TICKS) {
            let epoch = tick / COMMAND_INTERVAL_TICKS;
            actions.extend(
                self.shuttles
                    .iter()
                    .filter_map(|shuttle| shuttle.command_for_epoch(game, tick, epoch))
                    .map(LabScenarioAction::Command),
            );
        }
        actions.retain(|action| {
            !self
                .retained_entries_at_tick
                .iter()
                .any(|entry| action.matches_replay_entry(entry))
        });
        self.retained_entries_at_tick.clear();
        actions.truncate(MAX_ACTIONS_PER_TICK);
        actions
    }

    fn respawn_action(&mut self, game: &Game, tick: u32) -> Option<LabScenarioAction> {
        if self.target_composition.is_empty() {
            return None;
        }
        let mut current = BTreeMap::new();
        for player_id in [1, 2] {
            let units = match game.lab_owned_units(player_id) {
                Ok(units) => units,
                Err(err) => {
                    eprintln!(
                        "Hellhole could not inspect player {player_id} roster at tick {tick}: {err:?}"
                    );
                    return None;
                }
            };
            current.extend(
                units
                    .into_iter()
                    .map(|(id, kind)| (id, (player_id, kind))),
            );
        }

        if self.reset_roster_on_next_tick {
            self.active_central_unit_ids = current.keys().copied().collect();
            self.reset_roster_on_next_tick = false;
        }
        if self.central_unit_provenance.is_empty() {
            for (&entity_id, &(owner, current_kind)) in &current {
                self.active_central_unit_ids.insert(entity_id);
                self.central_unit_provenance
                    .insert(entity_id, (owner, current_kind));
            }
            return None;
        }

        let missing_before_conversions: Vec<_> = self
            .active_central_unit_ids
            .iter()
            .filter(|entity_id| !current.contains_key(entity_id))
            .filter_map(|entity_id| {
                self.central_unit_provenance
                    .get(entity_id)
                    .map(|&(owner, kind)| (*entity_id, owner, kind))
            })
            .collect();
        let mut converted_old_ids = BTreeSet::new();
        for (&new_id, &(owner, current_kind)) in &current {
            if self.active_central_unit_ids.contains(&new_id) {
                continue;
            }
            let converted_from = (current_kind == EntityKind::Rifleman).then(|| {
                missing_before_conversions
                    .iter()
                    .find(|(old_id, old_owner, old_kind)| {
                        !converted_old_ids.contains(old_id)
                            && *old_owner == owner
                            && *old_kind == EntityKind::Panzerfaust
                    })
            });
            if let Some(Some(&(old_id, old_owner, original_kind))) = converted_from {
                converted_old_ids.insert(old_id);
                self.active_central_unit_ids.remove(&old_id);
                self.active_central_unit_ids.insert(new_id);
                self.central_unit_provenance
                    .insert(new_id, (old_owner, original_kind));
            } else {
                self.active_central_unit_ids.insert(new_id);
                self.central_unit_provenance
                    .entry(new_id)
                    .or_insert((owner, current_kind));
            }
        }
        let missing: Vec<_> = missing_before_conversions
            .into_iter()
            .filter(|(entity_id, _, _)| !converted_old_ids.contains(entity_id))
            .collect();
        let mut selected_missing = Vec::new();
        let mut requests = Vec::new();
        for player_id in [1, 2] {
            let live_kinds: Vec<_> = current
                .values()
                .filter(|(owner, _)| *owner == player_id)
                .map(|(_, kind)| *kind)
                .collect();
            let shortage = self
                .target_composition
                .len()
                .saturating_sub(live_kinds.len());
            selected_missing.extend(
                missing
                    .iter()
                    .filter(|(_, owner, _)| *owner == player_id)
                    .take(shortage)
                    .copied(),
            );
            let selected_for_player: Vec<_> = selected_missing
                .iter()
                .filter(|(_, owner, _)| *owner == player_id)
                .copied()
                .collect();
            requests.extend(
                selected_for_player
                    .iter()
                    .map(|&(_, owner, kind)| (owner, kind)),
            );

            let mut modeled_kinds = live_kinds;
            modeled_kinds.extend(selected_for_player.iter().map(|(_, _, kind)| *kind));
            let mut remaining = shortage.saturating_sub(selected_for_player.len());
            for &target_kind in &self.target_composition {
                if let Some(index) = modeled_kinds.iter().position(|kind| *kind == target_kind) {
                    modeled_kinds.swap_remove(index);
                } else if remaining > 0 {
                    requests.push((player_id, target_kind));
                    remaining -= 1;
                }
            }
            requests.extend((0..remaining).map(|_| (player_id, EntityKind::Rifleman)));
        }
        if requests.is_empty() {
            if tick == 0 {
                for player_id in [1, 2] {
                    let count = current
                        .values()
                        .filter(|(owner, _)| *owner == player_id)
                        .count();
                    if count != self.target_composition.len() {
                        eprintln!(
                            "Hellhole central roster for player {player_id} started at {count}, expected {}",
                            self.target_composition.len()
                        );
                    }
                }
            }
            return None;
        }
        let spawns = match game.lab_plan_unit_spawns(&requests, &self.respawn_candidates) {
            Ok(spawns) => spawns,
            Err(err) => {
                eprintln!("Hellhole respawn planning failed at tick {tick}: {err:?}");
                return None;
            }
        };
        if spawns.len() != requests.len() {
            eprintln!(
                "Hellhole respawn planning left {} of {} deficits unresolved at tick {tick}",
                requests.len() - spawns.len(),
                requests.len()
            );
        }
        let mut claimed_missing = BTreeSet::new();
        for spawn in &spawns {
            if let Some((entity_id, _, _)) =
                selected_missing.iter().find(|(entity_id, owner, kind)| {
                    !claimed_missing.contains(entity_id)
                        && *owner == spawn.owner
                        && *kind == spawn.kind
                })
            {
                claimed_missing.insert(*entity_id);
                self.active_central_unit_ids.remove(entity_id);
            }
        }
        (!spawns.is_empty()).then(|| LabScenarioAction::LabOperation {
            request_id: deterministic_request_id(0x52, tick, 0),
            op: LabOp::SpawnEntities(spawns),
        })
    }

    pub(super) fn sync_to_tick(&mut self, tick: u32, entries: &[LabReplayOperationEntry]) {
        self.last_processed_tick = None;
        self.reset_roster_on_next_tick = true;
        self.retained_entries_at_tick = entries
            .iter()
            .filter(|entry| entry.tick == tick)
            .cloned()
            .collect();
    }

    #[cfg(test)]
    pub(crate) fn scripted_for_test(tick: u32, action: LabScenarioAction) -> Self {
        Self::scripted_actions_for_test(tick, vec![action])
    }

    #[cfg(test)]
    pub(crate) fn scripted_actions_for_test(tick: u32, actions: Vec<LabScenarioAction>) -> Self {
        Self {
            shuttles: Vec::new(),
            target_composition: Vec::new(),
            respawn_candidates: Vec::new(),
            central_unit_provenance: BTreeMap::new(),
            active_central_unit_ids: BTreeSet::new(),
            reset_roster_on_next_tick: true,
            scheduled_actions: actions
                .into_iter()
                .map(|action| ScheduledAction { tick, action })
                .collect(),
            last_processed_tick: None,
            retained_entries_at_tick: Vec::new(),
        }
    }
}

struct ScheduledAction {
    tick: u32,
    action: LabScenarioAction,
}

struct DiagonalShuttle {
    player_id: u32,
}

impl DiagonalShuttle {
    fn command_for_epoch(&self, game: &Game, tick: u32, epoch: u32) -> Option<LabScenarioCommand> {
        let mut units: Vec<_> = game
            .lab_owned_units(self.player_id)
            .ok()?
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        if units.len() < SHUTTLE_SELECTION_COUNT {
            return None;
        }
        units.sort_unstable_by_key(|entity_id| {
            (
                hash_words(&[0x53, self.player_id, epoch, *entity_id]),
                *entity_id,
            )
        });
        units.truncate(SHUTTLE_SELECTION_COUNT);
        let (x, y) = self.destination_for_epoch(game, epoch)?;
        Some(LabScenarioCommand {
            request_id: deterministic_request_id(0x43, tick, self.player_id),
            player_id: self.player_id,
            command: Command::Move {
                units,
                x,
                y,
                queued: false,
            },
            options: LabCommandOptions {
                ignore_command_limits: true,
            },
        })
    }

    fn destination_for_epoch(&self, game: &Game, epoch: u32) -> Option<(f32, f32)> {
        let phase = epoch.saturating_mul(COMMAND_INTERVAL_TICKS) / LEG_TICKS;
        let base = shuttle_endpoint(self.player_id, phase);
        let mut offsets = Vec::new();
        for along in -CORRIDOR_ALONG_RADIUS_TILES..=CORRIDOR_ALONG_RADIUS_TILES {
            for lateral in -CORRIDOR_LATERAL_RADIUS_TILES..=CORRIDOR_LATERAL_RADIUS_TILES {
                let (axis_y, normal_y) = if self.player_id == 3 {
                    (-1, 1)
                } else {
                    (1, -1)
                };
                offsets.push((along + lateral, along * axis_y + lateral * normal_y));
            }
        }
        let start = hash_words(&[0x47, self.player_id, epoch]) as usize % offsets.len();
        let map = &game.start_payload().map;
        (0..offsets.len()).find_map(|step| {
            let (dx, dy) = offsets[(start + step) % offsets.len()];
            let tile_x = base.0 + dx;
            let tile_y = base.1 + dy;
            if tile_x < 0 || tile_y < 0 || tile_x >= map.width as i32 || tile_y >= map.height as i32
            {
                return None;
            }
            let index = tile_y as usize * map.width as usize + tile_x as usize;
            (map.terrain.get(index).copied() != Some(crate::protocol::terrain::ROCK))
                .then_some(((tile_x as f32 + 0.5) * TILE, (tile_y as f32 + 0.5) * TILE))
        })
    }

    #[cfg(test)]
    fn destination_tile_for_epoch(&self, game: &Game, epoch: u32) -> Option<(i32, i32)> {
        self.destination_for_epoch(game, epoch)
            .map(|(x, y)| ((x / TILE).floor() as i32, (y / TILE).floor() as i32))
    }
}

fn deterministic_request_id(tag: u32, tick: u32, player_id: u32) -> u32 {
    0xa000_0000 | (hash_words(&[tag, tick, player_id]) & 0x0fff_ffff)
}

#[cfg(test)]
fn corridor_contains(player_id: u32, phase: u32, tile: (i32, i32)) -> bool {
    let base = shuttle_endpoint(player_id, phase);
    let dx = tile.0 - base.0;
    let dy = tile.1 - base.1;
    if player_id == 3 {
        let along = (dx - dy) / 2;
        let lateral = (dx + dy) / 2;
        along.abs() <= CORRIDOR_ALONG_RADIUS_TILES && lateral.abs() <= CORRIDOR_LATERAL_RADIUS_TILES
    } else {
        let along = (dx + dy) / 2;
        let lateral = (dx - dy) / 2;
        along.abs() <= CORRIDOR_ALONG_RADIUS_TILES && lateral.abs() <= CORRIDOR_LATERAL_RADIUS_TILES
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum LabScenarioAction {
    Command(LabScenarioCommand),
    LabOperation { request_id: u32, op: LabOp },
}

impl LabScenarioAction {
    fn matches_replay_entry(&self, entry: &LabReplayOperationEntry) -> bool {
        match self {
            Self::Command(command) => {
                entry.request_id == command.request_id
                    && entry.op
                        == (LabReplayOperation::IssueCommandAs {
                            player_id: command.player_id,
                            cmd: command.command.clone(),
                            ignore_command_limits: command.options.ignore_command_limits,
                        })
            }
            Self::LabOperation { request_id, op } => {
                entry.request_id == *request_id
                    && lab_op_to_replay_operation(op).is_some_and(|replay_op| entry.op == replay_op)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LabScenarioCommand {
    pub(crate) request_id: u32,
    pub(crate) player_id: u32,
    pub(crate) command: Command,
    pub(crate) options: LabCommandOptions,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use rts_sim::game::entity::EntityKind;
    use rts_sim::game::lab::LabSpawnEntity;

    fn move_entry(player_id: u32, tick: u32, destination: (f32, f32)) -> LabReplayOperationEntry {
        LabReplayOperationEntry {
            sequence: 0,
            tick,
            request_id: tick + 1,
            operator_id: 99,
            op: LabReplayOperation::IssueCommandAs {
                player_id,
                cmd: Command::Move {
                    units: vec![1],
                    x: destination.0,
                    y: destination.1,
                    queued: false,
                },
                ignore_command_limits: true,
            },
        }
    }

    #[test]
    fn replay_matching_only_recognizes_the_exact_scripted_shuttle_command() {
        let driver = LabScenarioDriver::supply_300_hellhole().unwrap();
        let scenario = crate::lab_scenarios::load_lab_scenario_by_id(SCENARIO_ID).unwrap();
        let game = scenario.build_game().unwrap();
        let scripted_destination = driver.shuttles[0].destination_for_epoch(&game, 30).unwrap();
        let mut user_entry = move_entry(3, LEG_TICKS, (0.0, 0.0));
        let action = LabScenarioAction::Command(LabScenarioCommand {
            request_id: LEG_TICKS + 1,
            player_id: 3,
            command: Command::Move {
                units: vec![1],
                x: scripted_destination.0,
                y: scripted_destination.1,
                queued: false,
            },
            options: LabCommandOptions {
                ignore_command_limits: true,
            },
        });

        assert!(!action.matches_replay_entry(&user_entry));

        user_entry = move_entry(3, LEG_TICKS, scripted_destination);
        assert!(action.matches_replay_entry(&user_entry));

        user_entry.request_id += 1;
        assert!(!action.matches_replay_entry(&user_entry));
    }

    #[test]
    fn retained_spawn_action_is_filtered_independently() {
        let action = LabScenarioAction::LabOperation {
            request_id: 7,
            op: LabOp::SpawnEntities(vec![LabSpawnEntity {
                owner: 1,
                kind: EntityKind::Rifleman,
                x: 320.0,
                y: 320.0,
                completed: true,
            }]),
        };
        let replay_op = lab_op_to_replay_operation(match &action {
            LabScenarioAction::LabOperation { op, .. } => op,
            _ => unreachable!(),
        })
        .unwrap();
        let entry = LabReplayOperationEntry {
            sequence: 0,
            tick: 0,
            request_id: 7,
            operator_id: 99,
            op: replay_op,
        };
        let mut driver = LabScenarioDriver::scripted_for_test(0, action);
        driver.sync_to_tick(0, &[entry]);
        let scenario =
            crate::lab_scenarios::load_lab_scenario_by_id(SCENARIO_ID).expect("hellhole scenario");
        let game = scenario.build_game().expect("hellhole game");
        assert!(driver.actions_for_tick(&game).is_empty());
    }

    #[test]
    fn shuttle_commands_select_exactly_43_each_second_and_vary_goal_tiles() {
        let scenario = crate::lab_scenarios::load_lab_scenario_by_id(SCENARIO_ID).unwrap();
        let game = scenario.build_game().unwrap();
        let first_driver = LabScenarioDriver::supply_300_hellhole().unwrap();
        let second_driver = LabScenarioDriver::supply_300_hellhole().unwrap();

        let first: Vec<_> = first_driver
            .shuttles
            .iter()
            .map(|shuttle| shuttle.command_for_epoch(&game, 0, 0).unwrap())
            .collect();
        let second: Vec<_> = second_driver
            .shuttles
            .iter()
            .map(|shuttle| shuttle.command_for_epoch(&game, 0, 0).unwrap())
            .collect();
        assert_eq!(
            first, second,
            "fresh runs must choose the same units and goals"
        );
        for command in &first {
            assert!(command.options.ignore_command_limits);
            assert!(matches!(
                &command.command,
                Command::Move {
                    units,
                    queued: false,
                    ..
                } if units.len() == SHUTTLE_SELECTION_COUNT
            ));
        }

        for shuttle in &first_driver.shuttles {
            let destinations: BTreeSet<_> = (0..30)
                .map(|epoch| shuttle.destination_tile_for_epoch(&game, epoch).unwrap())
                .collect();
            assert!(
                destinations.len() >= 8,
                "player {} should vary integer goal tiles throughout a leg",
                shuttle.player_id
            );
            assert!(destinations
                .iter()
                .all(|tile| corridor_contains(shuttle.player_id, 0, *tile)));
            let next_leg = shuttle.destination_tile_for_epoch(&game, 30).unwrap();
            assert!(corridor_contains(shuttle.player_id, 1, next_leg));

            let selections: BTreeSet<_> = (0..4)
                .map(|epoch| {
                    let command = shuttle
                        .command_for_epoch(&game, epoch * COMMAND_INTERVAL_TICKS, epoch)
                        .unwrap();
                    match command.command {
                        Command::Move { units, .. } => units,
                        _ => unreachable!(),
                    }
                })
                .collect();
            assert!(
                selections.len() > 1,
                "the selected half should change by epoch"
            );
        }
    }

    #[test]
    fn central_deficit_has_one_low_snapshot_then_restores_owner_kind_and_supply() {
        let scenario = crate::lab_scenarios::load_lab_scenario_by_id(SCENARIO_ID).unwrap();
        let mut game = scenario.build_game().unwrap();
        let mut driver = LabScenarioDriver::supply_300_hellhole().unwrap();
        assert_eq!(driver.actions_for_tick(&game).len(), 2);
        let victim = game
            .snapshot_full_for(1)
            .entities
            .iter()
            .find(|entity| entity.owner == 1 && entity.kind == "tank")
            .map(|entity| entity.id)
            .unwrap();
        game.apply_lab_op(LabOp::DeleteEntity { entity_id: victim })
            .unwrap();
        game.tick();

        let low = game.snapshot_full_for(1);
        assert!(low.entities.len() < 380);
        assert!(low
            .player_resources
            .iter()
            .find(|player| player.id == 1)
            .is_some_and(|player| player.supply_used < 300));

        let actions = driver.actions_for_tick(&game);
        let respawns: Vec<_> = actions
            .iter()
            .filter_map(|action| match action {
                LabScenarioAction::LabOperation {
                    op: LabOp::SpawnEntities(spawns),
                    ..
                } => Some(spawns),
                _ => None,
            })
            .collect();
        assert_eq!(respawns.len(), 1);
        assert_eq!(respawns[0].len(), 380 - low.entities.len());
        assert!(respawns[0]
            .iter()
            .any(|spawn| (spawn.owner, spawn.kind) == (1, EntityKind::Tank)));

        for action in actions {
            match action {
                LabScenarioAction::Command(command) => game
                    .issue_lab_command_as(command.player_id, command.command, command.options)
                    .unwrap(),
                LabScenarioAction::LabOperation { op, .. } => {
                    game.apply_lab_op(op).unwrap();
                }
            }
        }
        let restored = game.snapshot_full_for(1);
        assert_eq!(restored.entities.len(), 380);
        assert!(restored
            .player_resources
            .iter()
            .find(|player| player.id == 1)
            .is_some_and(|player| player.supply_used == 300));
        assert_eq!(
            restored
                .entities
                .iter()
                .filter(|entity| entity.owner == 1 && entity.kind == "tank")
                .count(),
            17
        );
    }
}
