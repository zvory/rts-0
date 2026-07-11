use super::*;
use std::collections::BTreeSet;

use crate::game::entity::{
    BuildPhase, EntityKind, EntityStore, GatherPhase, Order, OrderIntent, RallyKind, WeaponSetup,
    MAX_QUEUED_ORDERS,
};
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::occupancy::{footprint_center, footprint_tiles, Occupancy};
use crate::game::services::pathing::PathingService;
use crate::game::services::spatial::SpatialIndex;
use crate::game::ScoreState;
use crate::protocol::terrain;

mod abilities;
mod artillery_point_fire_queue;
mod build;
mod command_budget;
mod orders;
mod production;
mod rally;
mod scout_plane_production;
mod support_weapons;
mod tank_traps;

/// Run `apply_commands` with throwaway derived state for command-validation tests.
fn apply(map: &Map, entities: &mut EntityStore, pending: Vec<(u32, SimCommand)>) {
    let mut players = vec![player_state(1), player_state(2)];
    let _ = apply_with_players(map, entities, &mut players, pending);
}

fn apply_with_players(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    pending: Vec<(u32, SimCommand)>,
) -> HashMap<u32, Vec<Event>> {
    let mut smokes = SmokeCloudStore::new();
    apply_with_players_and_smokes(map, entities, players, &mut smokes, normal_pending(pending))
}

fn apply_with_players_and_smokes(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    smokes: &mut SmokeCloudStore,
    pending: Vec<PendingCommand>,
) -> HashMap<u32, Vec<Event>> {
    let spatial = SpatialIndex::build(entities, map.size);
    let occ = Occupancy::build(map, entities);
    let mut pathing = PathingService::new(1024, 32);
    pathing.advance_tick(1);
    let mut coordinator = MoveCoordinator::new(&mut pathing, map, &occ, 1);
    let mut fog = Fog::new(map.size);
    fog.recompute(&[1, 2], entities, map);
    let mut events: HashMap<u32, Vec<Event>> = players
        .iter()
        .map(|player| (player.id, Vec::new()))
        .collect();
    let mut mortar_shells = MortarShellStore::default();
    let mut artillery_shells = ArtilleryShellStore::default();
    let mut firing_reveals = Vec::new();
    let mut ability_runtime = AbilityRuntime::new();
    apply_commands(
        map,
        entities,
        players,
        &spatial,
        &mut coordinator,
        &fog,
        smokes,
        &mut ability_runtime,
        &mut mortar_shells,
        &mut artillery_shells,
        &mut firing_reveals,
        pending,
        &mut events,
        1,
    );
    events
}

fn normal_pending(pending: Vec<(u32, SimCommand)>) -> Vec<PendingCommand> {
    pending
        .into_iter()
        .map(|(player, command)| PendingCommand {
            player,
            command,
            admission: CommandAdmission::Normal,
        })
        .collect()
}

fn flat_map(size: u32) -> Map {
    Map {
        size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![],
        base_sites: vec![],
    }
}

fn player_state(id: u32) -> PlayerState {
    PlayerState {
        id,
        team_id: id,
        faction_id: "kriegsia".to_string(),
        name: format!("Player {id}"),
        color: "#fff".to_string(),
        start_tile: (0, 0),
        steel: 1_000,
        oil: 1_000,
        supply_used: 0,
        supply_cap: 20,
        is_ai: false,
        score: ScoreState::default(),
        upgrades: Default::default(),
        ability_cooldowns: Default::default(),
    }
}

fn spawn_units(entities: &mut EntityStore, owner: u32, kind: EntityKind, count: usize) -> Vec<u32> {
    (0..count)
        .map(|index| {
            let x = 96.0 + (index % 8) as f32 * 32.0;
            let y = 96.0 + (index / 8) as f32 * 32.0;
            entities
                .spawn_unit(owner, kind, x, y)
                .expect("unit should spawn")
        })
        .collect()
}

fn mark_units_moving(entities: &mut EntityStore, units: &[u32]) {
    for id in units {
        entities
            .get_mut(*id)
            .expect("unit should exist")
            .set_order(Order::move_to(10.0, 10.0));
    }
}

fn assert_notice(events: &HashMap<u32, Vec<Event>>, player: u32, message: &str) {
    assert!(
        events
            .get(&player)
            .is_some_and(|player_events| player_events
                .iter()
                .any(|event| matches!(event, Event::Notice { msg, .. } if msg == message))),
        "expected notice {message:?} for player {player}: {events:?}"
    );
}

fn fill_queue(entities: &mut EntityStore, id: u32) {
    for _ in 0..MAX_QUEUED_ORDERS {
        entities
            .get_mut(id)
            .expect("unit should exist")
            .append_queued_order(OrderIntent::move_to(999.0, 999.0));
    }
}
