use super::*;

pub(super) fn human_vs_ai_players() -> [PlayerInit; 2] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Human".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Computer".into(),
            color: "#000".into(),
            is_ai: true,
        },
    ]
}

pub(super) fn advance_to_fog_refresh(game: &mut Game) {
    while !game.tick_count().is_multiple_of(FOG_UPDATE_INTERVAL_TICKS) {
        game.tick();
    }
}

pub(super) fn legacy_snapshot_entities(game: &Game, player: u32, fogged: bool) -> Vec<EntityView> {
    let mut entities = Vec::new();
    for id in game.final_spatial().all_ids() {
        let Some(e) = game.state.entities.get(id) else {
            continue;
        };
        let own = e.owner == player;
        if fogged
            && !own
            && !e.kind.is_node()
            && !game.state.fog.is_visible_world(player, e.pos_x, e.pos_y)
        {
            continue;
        }
        entities.push(legacy_view_of(game, e, player, fogged));
    }
    entities.sort_by_key(|v| v.id);
    entities
}

fn legacy_view_of(game: &Game, e: &Entity, viewer: u32, fogged: bool) -> EntityView {
    let mut v = EntityView::new(
        e.id,
        e.owner,
        crate::protocol::kind_to_wire(e.kind),
        e.pos_x,
        e.pos_y,
        e.hp,
        e.max_hp,
        e.state_str(),
    );

    if e.is_unit() {
        v.facing = Some(e.facing());
    }
    let active_combat_target = matches!(e.order(), Order::Attack(_) | Order::AttackMove(_))
        || (e.is_building() && e.can_attack());
    let target_visible = if let Some(t) = e.target_id() {
        game.state
            .entities
            .get(t)
            .map(|target| {
                e.owner == viewer
                    || !fogged
                    || game
                        .state
                        .fog
                        .is_visible_world(viewer, target.pos_x, target.pos_y)
            })
            .unwrap_or(false)
    } else {
        false
    };
    let weapon_facing_useful = e.kind == EntityKind::Tank || active_combat_target;
    if weapon_facing_useful {
        if let Some(weapon_facing) = e.weapon_facing() {
            let weapon_facing_is_safe = e.owner == viewer
                || !fogged
                || e.target_id().is_none()
                || !active_combat_target
                || target_visible;
            if weapon_facing_is_safe {
                v.weapon_facing = Some(weapon_facing);
            }
        }
    }
    if e.kind == EntityKind::MachineGunner {
        v.setup_state = Some(e.weapon_setup().to_protocol_str().to_string());
    }
    if e.kind == EntityKind::AntiTankGun {
        v.setup_state = Some(e.weapon_setup().to_protocol_str().to_string());
        if e.owner == viewer {
            v.setup_facing = e.emplacement_facing();
        }
    }
    if e.is_building() && !e.prod_queue().is_empty() {
        if let Some(front) = e.prod_queue().first() {
            v.prod_kind = Some(crate::protocol::kind_to_wire(front.unit).to_string());
            v.prod_progress = Some(if front.total == 0 {
                0.0
            } else {
                front.progress as f32 / front.total as f32
            });
        }
        if e.owner == viewer {
            v.prod_queue = Some(e.prod_queue().len() as u32);
            v.prod_scout_plane_queued = e
                .prod_queue()
                .iter()
                .any(|item| item.unit == EntityKind::ScoutPlane);
        }
    }
    if let Some(progress) = e.build_progress_fraction() {
        v.build_progress = Some(progress);
    }
    if e.is_node() {
        v.remaining = e.remaining();
    }
    if e.kind == EntityKind::Worker && e.gather_phase() == Some(GatherPhase::Harvesting) {
        if let Some(node) = e.order().gather_node() {
            v.latched_node = Some(node);
        }
    }
    if let Some(t) = e.target_id() {
        if active_combat_target {
            if game.state.entities.get(t).is_some() {
                if target_visible {
                    v.target_id = Some(t);
                }
            }
        }
    }
    if e.owner == viewer {
        for kind in [ability::AbilityKind::Charge, ability::AbilityKind::Smoke] {
            if ability::carried_by(kind, e.kind) {
                v.abilities.push(AbilityCooldownView {
                    ability: kind.to_protocol_str().to_string(),
                    cooldown_left: e.ability_cooldown_ticks(kind),
                    remaining_uses: e.ability_uses_remaining(kind),
                    autocast_enabled: e.autocast_enabled(kind),
                    active_object_id: None,
                    available_tick: None,
                    lockout_until_tick: None,
                    expires_in: None,
                });
            }
        }
        if let Order::Attack(order) = e.order() {
            if let Some(target) = game.state.entities.get(order.intent.target) {
                if target_visible {
                    v.order_plan.push(OrderPlanMarker {
                        kind: "attack".to_string(),
                        x: target.pos_x,
                        y: target.pos_y,
                    });
                }
            }
        }
    }
    v
}

pub(super) fn flat_tank_move_fixture() -> (Game, u32, (f32, f32)) {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new_for_replay(&players, 0x1234_5678);
    for tile in &mut game.state.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }

    let start = game.state.map.tile_center(4, 4);
    let goal = game.state.map.tile_center(28, 17);
    let tank = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Tank, start.0, start.1)
        .expect("tank should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);
    game.assert_invariants();

    (game, tank, goal)
}

pub(super) fn empty_flat_game(players: &[PlayerInit]) -> Game {
    let mut game = Game::new_for_replay(players, 0x1234_5678);
    for tile in &mut game.state.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }
    game.state.smokes = SmokeCloudStore::new();
    game.state.mortar_shells = MortarShellStore::default();
    game.state.artillery_shells = artillery::ArtilleryShellStore::default();
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);
    game
}

pub(super) fn smoke_command_fixture() -> (Game, u32, (f32, f32), (f32, f32)) {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new_for_replay_with_starting_resources(&players, 500, 500, 0x5150_0303);
    for tile in &mut game.state.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }

    let scout_pos = game.state.map.tile_center(8, 8);
    let target = game.state.map.tile_center(24, 8);
    let second_target = game.state.map.tile_center(25, 10);
    let scout = game
        .state
        .entities
        .spawn_unit(1, EntityKind::ScoutCar, scout_pos.0, scout_pos.1)
        .expect("scout car should spawn");
    let research_pos = game.state.map.tile_center(4, 4);
    game.state
        .entities
        .spawn_building(
            1,
            EntityKind::ResearchComplex,
            research_pos.0,
            research_pos.1,
            true,
        )
        .expect("completed R&D Complex should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state.fog.recompute_with_smoke(
        &ids,
        &game.state.entities,
        &game.state.map,
        &game.state.smokes,
    );
    game.assert_invariants();

    (game, scout, target, second_target)
}

pub(super) fn entity_distance_to(game: &Game, id: u32, point: (f32, f32)) -> f32 {
    let entity = game.state.entities.get(id).expect("entity should exist");
    let dx = entity.pos_x - point.0;
    let dy = entity.pos_y - point.1;
    (dx * dx + dy * dy).sqrt()
}
