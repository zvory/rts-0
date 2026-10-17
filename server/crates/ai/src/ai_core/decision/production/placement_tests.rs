use super::super::super::geometry::{building_center, dist2, squared};
use super::super::super::tank_micro::{observe as observe_tanks, TankMicro};
use super::super::try_build_production;
use super::*;
use crate::ai_core::actions::SpendBudget;
use crate::ai_core::observation::{AiEntityState, AiEntitySummary};
use crate::ai_core::profiles::{JEFFS_AI, JEFFS_AI_PRE_SAFE_BASES};
use rts_sim::game::map::Map;
use rts_sim::game::{Game, PlayerInit};

/// Each start of a two-player map at tick 0, with its map analysis.
fn starts(map_name: &str) -> (AiMapAnalysis, Vec<AiObservation>) {
    let players: Vec<_> = (1..=2)
        .map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: "kriegsia".into(),
            name: format!("P{id}"),
            color: "#ffffff".into(),
            is_ai: true,
        })
        .collect();
    let map = Map::load_for_players(map_name, &[(1, 1), (2, 2)], 0x1234_5678).unwrap();
    let game = Game::new_with_random_ai_profiles_and_map_metadata(
        &players,
        0x1234_5678,
        map,
        Map::metadata_for_name(map_name).unwrap(),
    );
    let start = game.start_payload();
    let analysis = AiMapAnalysis::analyze(&start);
    let observations = [1, 2]
        .into_iter()
        .map(|player| {
            AiObservation::from_snapshot_with_alive(
                &start,
                &game.snapshot_for(player),
                player,
                [],
                None,
            )
            .unwrap()
        })
        .collect();
    (analysis, observations)
}

fn building(id: u32, owner: u32, kind: EntityKind, (x, y): (f32, f32)) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner,
        kind,
        x,
        y,
        hp: config::building_stats(kind).map_or(100, |stats| stats.hp),
        state: AiEntityState::Idle,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: false,
    }
}

/// The tech a Factory needs, standing in a corner out of the way.
fn with_factory_tech(mut observation: AiObservation) -> AiObservation {
    let player = observation.player_id;
    for (id, kind) in [
        (900, EntityKind::Barracks),
        (901, EntityKind::TrainingCentre),
        (902, EntityKind::EngineeringComplex),
    ] {
        observation
            .owned
            .push(building(id, player, kind, (16.0, 16.0)));
    }
    observation
}

/// Open ground, off the resources and clear of Jeff's HQ and other buildings.
fn placeable<'a>(
    observation: &'a AiObservation,
    analysis: &'a AiMapAnalysis,
) -> impl FnMut(EntityKind, u32, u32) -> bool + 'a {
    move |kind, tile_x, tile_y| {
        let Some(stats) = config::building_stats(kind) else {
            return false;
        };
        let open = (0..stats.foot_h).all(|dy| {
            (0..stats.foot_w).all(|dx| analysis.tile_is_passable(tile_x + dx, tile_y + dy))
        });
        let ts = observation.map.tile_size as f32;
        let Some(center) = building_center((tile_x, tile_y), kind, observation.map.tile_size)
        else {
            return false;
        };
        open && observation
            .resources
            .iter()
            .all(|resource| dist2(resource.x, resource.y, center.0, center.1) > squared(4.0 * ts))
            && observation
                .owned
                .iter()
                .filter(|entity| entity.kind.is_building())
                .all(|entity| dist2(entity.x, entity.y, center.0, center.1) > squared(5.0 * ts))
    }
}

/// Where `profile` would place `kind` now.
fn site(
    profile: &AiProfile,
    observation: &AiObservation,
    analysis: &AiMapAnalysis,
    micro: &TankMicro,
    kind: EntityKind,
) -> Option<(u32, u32)> {
    let facts = AiFacts::from_observation(observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(2_000, 2_000, 0, 200));
    let workers: Vec<u32> = observation
        .owned
        .iter()
        .filter(|entity| entity.kind == EntityKind::Worker)
        .map(|entity| entity.id)
        .collect();
    let pools = [workers.as_slice()];
    let search = ai_shared::BuildSearch {
        min_radius: 2,
        max_radius: ai_shared::DEFAULT_BUILD_SEARCH_MAX_RADIUS,
        prefer_away_from_center: false,
        prefer_toward_center: false,
    };
    try_build_production(
        observation,
        &facts,
        &mut actions,
        &pools,
        profile,
        kind,
        search,
        (Some(analysis), micro),
        &mut placeable(observation, analysis),
    )
    .map(|built| (built.tile_x, built.tile_y))
}

fn depth(
    observation: &AiObservation,
    analysis: &AiMapAnalysis,
    kind: EntityKind,
    at: (u32, u32),
) -> f32 {
    site_depth(observation, analysis, kind, at.0, at.1).unwrap()
}

#[test]
fn production_goes_behind_the_hq_on_doppelganger() {
    let (analysis, observations) = starts("Doppelganger");
    let micro = TankMicro::default();
    for observation in observations {
        let observation = with_factory_tech(observation);
        for kind in [
            EntityKind::Factory,
            EntityKind::Barracks,
            EntityKind::EngineeringComplex,
            EntityKind::Steelworks,
        ] {
            let live = site(&JEFFS_AI, &observation, &analysis, &micro, kind).unwrap();
            let before = site(
                &JEFFS_AI_PRE_SAFE_BASES,
                &observation,
                &analysis,
                &micro,
                kind,
            )
            .unwrap();
            let (live_depth, before_depth) = (
                depth(&observation, &analysis, kind, live),
                depth(&observation, &analysis, kind, before),
            );
            assert!(
                live_depth >= SHELTERED_DEPTH_TILES,
                "{kind:?} from {:?} at {live:?}: {live_depth}",
                observation.own_start_tile
            );
            assert!(
                before_depth < live_depth,
                "{kind:?} from {:?}: before {before:?} {before_depth}, live {live:?} {live_depth}",
                observation.own_start_tile
            );
        }
    }
}

#[test]
fn the_first_barracks_keeps_its_usual_site() {
    let (analysis, observations) = starts("Doppelganger");
    let micro = TankMicro::default();
    for mut observation in observations {
        observation
            .owned
            .retain(|entity| entity.kind != EntityKind::Barracks);
        let live = site(
            &JEFFS_AI,
            &observation,
            &analysis,
            &micro,
            EntityKind::Barracks,
        );
        let before = site(
            &JEFFS_AI_PRE_SAFE_BASES,
            &observation,
            &analysis,
            &micro,
            EntityKind::Barracks,
        );
        assert_eq!(live, before);
    }
}

#[test]
fn production_keeps_out_of_reach_of_remembered_enemy_tanks() {
    let (analysis, observations) = starts("Doppelganger");
    let tile_size = observations[0].map.tile_size;
    let ts = tile_size as f32;
    for observation in observations {
        let mut observation = with_factory_tech(observation);
        let quiet = site(
            &JEFFS_AI,
            &observation,
            &analysis,
            &TankMicro::default(),
            EntityKind::Factory,
        )
        .unwrap();
        let center = building_center(quiet, EntityKind::Factory, tile_size).unwrap();
        // An enemy Tank seen standing 6 tiles from that site.
        let enemy = if observation.player_id == 1 { 2 } else { 1 };
        let mut tank = building(
            800,
            enemy,
            EntityKind::Tank,
            (center.0 + 6.0 * ts, center.1),
        );
        tank.hp = 300;
        let tank_at = (tank.x, tank.y);
        observation.visible_enemies.push(tank);
        let mut micro = TankMicro::default();
        observe_tanks(&observation, &mut micro);
        let moved = site(
            &JEFFS_AI,
            &observation,
            &analysis,
            &micro,
            EntityKind::Factory,
        )
        .unwrap();
        let moved_center = building_center(moved, EntityKind::Factory, tile_size).unwrap();
        let reach = footprint_distance2(EntityKind::Factory, moved_center, tank_at, ts);
        assert!(reach > squared(TANK_CLEARANCE_TILES * ts), "{moved:?}");
        // The freeze still builds where it did.
        assert_eq!(
            site(
                &JEFFS_AI_PRE_SAFE_BASES,
                &observation,
                &analysis,
                &micro,
                EntityKind::Factory
            ),
            site(
                &JEFFS_AI_PRE_SAFE_BASES,
                &observation,
                &analysis,
                &TankMicro::default(),
                EntityKind::Factory
            )
        );
    }
}

#[test]
fn a_factory_shelled_by_tanks_keeps_new_production_off_its_site_while_tanks_stay() {
    let (_, mut observations) = starts("Doppelganger");
    let tile_size = observations[0].map.tile_size;
    let ts = tile_size as f32;
    let mut observation = with_factory_tech(observations.remove(0));
    let factory_at = (
        observation.own_start_tile.0 as f32 * ts,
        observation.own_start_tile.1 as f32 * ts + 9.0 * ts,
    );
    observation
        .owned
        .push(building(700, 1, EntityKind::Factory, factory_at));
    let mut micro = TankMicro::default();
    observe_tanks(&observation, &mut micro);
    assert!(micro.shelled.sites.is_empty());
    // The Factory is gone with an enemy Tank 12 tiles away: its site is shelled.
    observation.tick += 30;
    observation.owned.retain(|entity| entity.id != 700);
    let mut tank = building(
        800,
        2,
        EntityKind::Tank,
        (factory_at.0, factory_at.1 + 12.0 * ts),
    );
    tank.hp = 300;
    observation.visible_enemies.push(tank);
    observe_tanks(&observation, &mut micro);
    assert_eq!(micro.shelled.sites.len(), 1);
    // Tanks seen near it keep it; a minute without them clears it.
    observation.visible_enemies.clear();
    observation.tick += SHELLED_MEMORY_TICKS;
    observe_tanks(&observation, &mut micro);
    assert_eq!(micro.shelled.sites.len(), 1);
    observation.tick += 1;
    observe_tanks(&observation, &mut micro);
    assert!(micro.shelled.sites.is_empty());
    // A building lost with no enemy Tank around was not shelled.
    observation
        .owned
        .push(building(701, 1, EntityKind::Barracks, factory_at));
    observe_tanks(&observation, &mut micro);
    observation.tick += 30;
    observation.owned.retain(|entity| entity.id != 701);
    observe_tanks(&observation, &mut micro);
    assert!(micro.shelled.sites.is_empty());
}
