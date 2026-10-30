use super::*;
use crate::ai_core::observation::AiEconomy;
use rts_sim::game::map::Map;
use rts_sim::game::{Game, MapMetadata, PlayerInit};

const TS: f32 = config::TILE_SIZE as f32;

/// Schone Tage as player 1, whose main is at (8,47) and whose raids arrive from the north-east.
fn schone() -> (AiMapAnalysis, AiObservation) {
    let players = (1..=2)
        .map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: "kriegsia".to_string(),
            name: format!("P{id}"),
            color: format!("#{id}{id}{id}"),
            is_ai: true,
        })
        .collect::<Vec<_>>();
    let slots = players
        .iter()
        .map(|p| (p.id, p.team_id))
        .collect::<Vec<_>>();
    let map = Map::load_for_players("Schone Tage", &slots, 0x1234_5678).expect("map loads");
    let metadata = Map::metadata_for_name("Schone Tage").unwrap_or_else(|_| MapMetadata {
        name: "Schone Tage".to_string(),
        schema_version: rts_sim::game::map::CURRENT_MAP_VERSION,
        content_hash: "test".to_string(),
    });
    let game =
        Game::new_with_random_ai_profiles_and_map_metadata(&players, 0x1234_5678, map, metadata);
    let start = game.start_payload();
    let me = start.players.iter().find(|p| p.id == 1).expect("player 1");
    let observation = AiObservation {
        player_id: 1,
        tick: 3_000,
        map: AiMapSummary {
            width: start.map.width,
            height: start.map.height,
            tile_size: start.map.tile_size,
        },
        economy: AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 100,
        },
        own_start_tile: (me.start_tile_x, me.start_tile_y),
        players: Vec::new(),
        owned: Vec::new(),
        resources: Vec::new(),
        visible_allies: Vec::new(),
        visible_enemies: Vec::new(),
        ability_states: Vec::new(),
        smokes: Vec::new(),
        visible_tank_traps: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    };
    (AiMapAnalysis::analyze(&start), observation)
}

fn unit(id: u32, owner: u32, kind: EntityKind, tile: (f32, f32)) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner,
        kind,
        x: tile.0 * TS,
        y: tile.1 * TS,
        hp: config::unit_stats(kind).map_or(100, |stats| stats.hp),
        state: AiEntityState::Idle,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: true,
    }
}

/// A main Depot and `count` Riflemen standing in the base, ids 101.. in age order.
fn home(observation: &mut AiObservation, count: u32) {
    let main = observation.own_start_tile;
    observation.owned.push(AiEntitySummary {
        is_complete: true,
        ..unit(
            1,
            1,
            EntityKind::ResourceDepot,
            (main.0 as f32 + 0.5, main.1 as f32 + 0.5),
        )
    });
    for index in 0..count {
        observation.owned.push(unit(
            101 + index,
            1,
            EntityKind::Rifleman,
            (
                main.0 as f32 + 3.0,
                main.1 as f32 - 2.0 + index as f32 * 0.8,
            ),
        ));
    }
}

fn plan(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    analysis: &AiMapAnalysis,
) -> RouteLineOrders {
    let facts = AiFacts::from_observation(observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    plan_route_line(&mut actions, observation, memory, Some(analysis))
}

#[test]
fn the_picket_waits_for_entrenchment_and_a_full_home_pocket() {
    let (analysis, mut observation) = schone();
    home(&mut observation, 5);
    let mut memory = AiDecisionMemory::default();
    plan(&observation, &mut memory, &analysis);
    assert_eq!(memory.route_line.picket(), None, "no Entrenchment yet");

    let (analysis, mut observation) = schone();
    home(&mut observation, 4);
    observation.upgrades.push(UpgradeKind::Entrenchment);
    plan(&observation, &mut memory, &analysis);
    assert_eq!(
        memory.route_line.picket(),
        None,
        "the pocket needs all four"
    );

    let (analysis, mut observation) = schone();
    home(&mut observation, 5);
    observation.upgrades.push(UpgradeKind::Entrenchment);
    let orders = plan(&observation, &mut memory, &analysis);
    assert_eq!(
        memory.route_line.picket(),
        Some(105),
        "the newest Rifleman, never a pocket owner"
    );
    assert_eq!(orders.ordered, vec![105]);
}

#[test]
fn the_picket_heads_up_the_route_toward_the_corridor() {
    let (analysis, mut observation) = schone();
    home(&mut observation, 5);
    observation.upgrades.push(UpgradeKind::Entrenchment);
    let mut memory = AiDecisionMemory::default();
    let point = picket_point(&observation, Some(&analysis)).expect("picket point");
    let main = tile_center(observation.own_start_tile, observation.map.tile_size);
    let distance = dist2(point.0, point.1, main.0, main.1).sqrt() / TS;
    assert!(
        (20.0..=28.0).contains(&distance),
        "picket {distance} tiles out"
    );
    assert!(point.1 < main.1, "Schone raids come from the north");
    plan(&observation, &mut memory, &analysis);
    assert!(memory.route_line.picket().is_some());
}

#[test]
fn a_lost_picket_is_not_replaced_until_the_retry_delay() {
    let (analysis, mut observation) = schone();
    home(&mut observation, 6);
    observation.upgrades.push(UpgradeKind::Entrenchment);
    let mut memory = AiDecisionMemory::default();
    plan(&observation, &mut memory, &analysis);
    let picket = memory.route_line.picket().expect("picket");

    observation.owned.retain(|unit| unit.id != picket);
    observation.tick += 9;
    plan(&observation, &mut memory, &analysis);
    assert_eq!(memory.route_line.picket(), None);

    observation.tick += PICKET_RETRY_TICKS - 18;
    plan(&observation, &mut memory, &analysis);
    assert_eq!(
        memory.route_line.picket(),
        None,
        "still inside the retry delay"
    );

    observation.tick += 18;
    plan(&observation, &mut memory, &analysis);
    assert!(memory.route_line.picket().is_some());
}

/// Enemy Riflemen `distance` tiles north-east of the main, outside the base.
fn raiders(observation: &mut AiObservation, count: u32, distance: f32, kind: EntityKind) {
    let main = tile_center(observation.own_start_tile, observation.map.tile_size);
    observation.visible_enemies = (0..count)
        .map(|index| AiEntitySummary {
            x: main.0 + (distance * 0.7 + index as f32 * 0.6) * TS,
            y: main.1 - distance * 0.7 * TS,
            ..unit(500 + index, 2, kind, (0.0, 0.0))
        })
        .collect();
}

fn alert_after_two_sightings(count: u32, kind: EntityKind, second_distance: f32) -> bool {
    let (analysis, mut observation) = schone();
    home(&mut observation, 4);
    let mut memory = AiDecisionMemory::default();
    raiders(&mut observation, count, 22.0, kind);
    plan(&observation, &mut memory, &analysis);
    observation.tick += 9;
    raiders(&mut observation, count, second_distance, kind);
    plan(&observation, &mut memory, &analysis);
    memory.route_line.alert.is_some()
}

#[test]
fn an_alert_needs_three_unarmored_units_closing_in() {
    assert!(alert_after_two_sightings(3, EntityKind::Rifleman, 20.0));
    assert!(
        !alert_after_two_sightings(2, EntityKind::Rifleman, 20.0),
        "two is a probe"
    );
    assert!(
        !alert_after_two_sightings(3, EntityKind::Rifleman, 22.0),
        "not closing in"
    );
    assert!(
        !alert_after_two_sightings(3, EntityKind::Rifleman, 23.0),
        "moving away"
    );
    assert!(
        !alert_after_two_sightings(3, EntityKind::Tank, 20.0),
        "armor is not a raid"
    );
}

#[test]
fn a_single_sighting_never_raises_an_alert() {
    let (analysis, mut observation) = schone();
    home(&mut observation, 4);
    let mut memory = AiDecisionMemory::default();
    raiders(&mut observation, 8, 15.0, EntityKind::Rifleman);
    plan(&observation, &mut memory, &analysis);
    assert!(memory.route_line.alert.is_none());
}

#[test]
fn sealers_are_spare_riflemen_near_home_and_are_released_after_the_alert() {
    let (analysis, mut observation) = schone();
    home(&mut observation, 7);
    // A spare Rifleman far from home is never pulled back to seal.
    let main = observation.own_start_tile;
    observation.owned.push(unit(
        120,
        1,
        EntityKind::Rifleman,
        (main.0 as f32 + 40.0, main.1 as f32),
    ));
    let mut memory = AiDecisionMemory::default();
    raiders(&mut observation, 4, 22.0, EntityKind::Rifleman);
    plan(&observation, &mut memory, &analysis);
    observation.tick += 9;
    raiders(&mut observation, 4, 20.0, EntityKind::Rifleman);
    let orders = plan(&observation, &mut memory, &analysis);
    let sealers: BTreeSet<u32> = memory.route_line.sealers.keys().copied().collect();
    assert_eq!(
        sealers,
        BTreeSet::from([105, 106, 107]),
        "never the four pocket owners"
    );
    assert_eq!(orders.ordered, vec![105, 106, 107]);

    // Out of sight long enough: the alert lapses and the sealers go back.
    observation.visible_enemies.clear();
    observation.tick += ALERT_EXPIRY_TICKS + 9;
    let orders = plan(&observation, &mut memory, &analysis);
    assert!(memory.route_line.alert.is_none());
    assert_eq!(orders.released, vec![105, 106, 107]);
    assert!(memory.route_line.sealers.is_empty());
}

#[test]
fn nothing_moves_while_an_enemy_is_inside_the_base() {
    let (analysis, mut observation) = schone();
    home(&mut observation, 7);
    let mut memory = AiDecisionMemory::default();
    raiders(&mut observation, 4, 22.0, EntityKind::Rifleman);
    plan(&observation, &mut memory, &analysis);
    observation.tick += 9;
    raiders(&mut observation, 4, 20.0, EntityKind::Rifleman);
    // One raider is already next to the Depot: local defense owns the fight.
    let main = tile_center(observation.own_start_tile, observation.map.tile_size);
    observation.visible_enemies.push(AiEntitySummary {
        x: main.0 + 2.0 * TS,
        y: main.1,
        ..unit(599, 2, EntityKind::Rifleman, (0.0, 0.0))
    });
    let orders = plan(&observation, &mut memory, &analysis);
    assert!(orders.ordered.is_empty(), "{orders:?}");
}
