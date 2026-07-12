use super::*;

#[test]
fn gather_command_ignores_nodes_without_nearby_completed_cc() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new_for_replay(&players, 0x1234_5678);
    let worker = game.state.entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == EntityKind::Worker)
        .map(|e| e.id)
        .expect("starting worker");
    let cc = game.state.entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == EntityKind::CityCentre)
        .expect("starting City Centre");
    let world = game.state.map.world_size_px();
    let far_x = if cc.pos_x < world * 0.5 {
        world - config::TILE_SIZE as f32 * 0.5
    } else {
        config::TILE_SIZE as f32 * 0.5
    };
    let far_y = if cc.pos_y < world * 0.5 {
        world - config::TILE_SIZE as f32 * 0.5
    } else {
        config::TILE_SIZE as f32 * 0.5
    };
    let far_node = game.state.entities
        .spawn_node(EntityKind::Steel, far_x, far_y)
        .expect("far resource node");

    game.enqueue(
        1,
        Command::Gather {
            units: vec![worker],
            node: far_node,
            queued: false,
        },
    );
    game.tick();

    let worker_order = game.state.entities.get(worker).expect("worker survives").order();
    assert!(
        !matches!(worker_order, Order::Gather(_)),
        "worker should ignore gather commands for patches outside City Centre mining range"
    );
}

#[test]
fn gather_command_to_occupied_patch_redirects_without_stealing_slot() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new_for_replay(&players, 0x1234_5678);
    let mut workers: Vec<u32> = game.state.entities
        .iter()
        .filter(|e| e.owner == 1 && e.kind == EntityKind::Worker)
        .map(|e| e.id)
        .collect();
    workers.sort_unstable();
    let holder = workers[0];
    let ordered = workers[1];
    let node = game.state.entities
        .iter()
        .find(|e| e.is_node())
        .map(|e| e.id)
        .expect("starting resource node");
    let (node_x, node_y) = game.state.entities
        .get(node)
        .map(|e| (e.pos_x, e.pos_y))
        .expect("node position");

    {
        let holder_entity = game.state.entities.get_mut(holder).expect("holder worker");
        holder_entity.pos_x = node_x;
        holder_entity.pos_y = node_y;
        holder_entity.set_order(Order::gather(node));
        holder_entity.mark_gather_phase(GatherPhase::Harvesting);
    }
    assert!(game.state.entities.claim_miner(node, holder));
    {
        let ordered_entity = game.state.entities.get_mut(ordered).expect("ordered worker");
        ordered_entity.pos_x = node_x + 4.0;
        ordered_entity.pos_y = node_y;
    }

    game.enqueue(
        1,
        Command::Gather {
            units: vec![ordered],
            node,
            queued: false,
        },
    );
    game.tick();

    let ordered_worker = game.state.entities.get(ordered).expect("worker survives");
    assert_ne!(
        ordered_worker.order().gather_node(),
        Some(node),
        "occupied patches should redirect extra workers away from the held node"
    );
    assert_eq!(
        game.state.entities.node_slot_holder(node),
        Some(holder),
        "the original worker should remain the single active miner"
    );
}

#[test]
fn worker_already_touching_resource_body_starts_harvesting() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new_for_replay(&players, 0x1234_5678);
    let worker = game.state.entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == EntityKind::Worker)
        .map(|e| e.id)
        .expect("starting worker");
    let node = game.state.entities
        .iter()
        .find(|e| e.is_node())
        .map(|e| e.id)
        .expect("starting resource node");
    let (node_x, node_y) = game.state.entities
        .get(node)
        .map(|e| (e.pos_x, e.pos_y))
        .expect("node position");
    let worker_radius = game.state.entities.get(worker).expect("worker").radius();
    let node_radius = game.state.entities.get(node).expect("node").radius();
    {
        let worker_entity = game.state.entities.get_mut(worker).expect("worker");
        worker_entity.pos_x = node_x + worker_radius + node_radius - 1.0;
        worker_entity.pos_y = node_y;
    }

    game.enqueue(
        1,
        Command::Gather {
            units: vec![worker],
            node,
            queued: false,
        },
    );
    game.tick();

    assert_eq!(
        game.state.entities.get(worker).and_then(|e| e.gather_phase()),
        Some(GatherPhase::Harvesting),
        "worker already touching the resource body should not need to reach the exact node center"
    );
}

#[test]
fn active_mining_stops_when_nearby_cc_is_removed() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new_for_replay(&players, 0x1234_5678);
    let worker = game.state.entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == EntityKind::Worker)
        .map(|e| e.id)
        .expect("starting worker");
    let (worker_x, worker_y) = game.state.entities
        .get(worker)
        .map(|e| (e.pos_x, e.pos_y))
        .expect("worker position");
    let node = game.state.entities
        .iter()
        .filter(|e| e.is_node())
        .min_by(|a, b| {
            let da = (a.pos_x - worker_x).powi(2) + (a.pos_y - worker_y).powi(2);
            let db = (b.pos_x - worker_x).powi(2) + (b.pos_y - worker_y).powi(2);
            da.total_cmp(&db).then_with(|| a.id.cmp(&b.id))
        })
        .map(|e| e.id)
        .expect("starting resource node");

    game.enqueue(
        1,
        Command::Gather {
            units: vec![worker],
            node,
            queued: false,
        },
    );
    for _ in 0..600 {
        game.tick();
        if matches!(
            game.state.entities.get(worker).and_then(|e| e.gather_phase()),
            Some(GatherPhase::Harvesting)
        ) {
            break;
        }
    }
    assert_eq!(
        game.state.entities.get(worker).and_then(|e| e.gather_phase()),
        Some(GatherPhase::Harvesting),
        "worker should reach and latch the starting patch before the City Centre is removed"
    );

    let cc = game.state.entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == EntityKind::CityCentre)
        .map(|e| e.id)
        .expect("starting City Centre");
    game.state.entities.remove(cc);
    let steel_before = game.state.players.iter().find(|p| p.id == 1).unwrap().steel;

    game.tick();
    assert!(
        matches!(
            game.state.entities.get(worker).map(|e| e.order()),
            Some(Order::Move(_))
        ),
        "worker should scatter away when its mining City Centre disappears"
    );

    for _ in 0..(config::HARVEST_TICKS + 5) {
        game.tick();
    }

    let steel_after = game.state.players.iter().find(|p| p.id == 1).unwrap().steel;
    assert_eq!(
        steel_after, steel_before,
        "mining should not continue without a City Centre"
    );
    assert!(
        !matches!(
            game.state.entities.get(worker).map(|e| e.order()),
            Some(Order::Gather(_))
        ),
        "worker should not resume gathering without City Centre coverage"
    );
}

#[test]
fn resource_snapshots_include_remaining_even_through_fog() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "A".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "B".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let game = Game::new_for_replay(&players, 0x1234_5678);
    let snapshot = game.snapshot_for(1);
    let resources: Vec<_> = snapshot
        .entities
        .iter()
        .filter(|e| e.owner == 0 && (e.kind == kinds::STEEL || e.kind == kinds::OIL))
        .collect();

    assert!(
        resources.iter().all(|e| e.remaining.is_some()),
        "current resource snapshots expose remaining for all static resource nodes"
    );
}

/// Every player must receive the same relative resource layout, and all starting resources
/// must fall within the configured min/max distance from the City Centre.
#[test]
fn spawn_resource_distances_are_fair_and_symmetric() {
    let counts = [1, 2, 3, 4];
    for &pc in &counts {
        let players: Vec<PlayerInit> = (1..=pc)
            .map(|id| PlayerInit {
                id,
                team_id: id,
                faction_id: "kriegsia".to_string(),
                name: format!("P{id}"),
                color: "#fff".into(),
                is_ai: false,
            })
            .collect();
        let game = Game::new_for_replay(&players, 0x1234_5678);

        let mut all_player_dists: Vec<Vec<(EntityKind, f32)>> = Vec::new();
        for p in &game.state.players {
            let cc = game.state.entities
                .iter()
                .find(|e| e.owner == p.id && e.kind == EntityKind::CityCentre)
                .expect("City Centre exists for every player");

            let mut dists = Vec::new();
            for e in game.state.entities.iter() {
                if e.owner != 0 || (!e.is_node()) {
                    continue;
                }
                let d_x = e.pos_x - cc.pos_x;
                let d_y = e.pos_y - cc.pos_y;
                let dist_tiles = (d_x * d_x + d_y * d_y).sqrt() / config::TILE_SIZE as f32;

                // Only consider nodes that belong to this player's start cluster.
                if dist_tiles <= config::CC_RESOURCE_MAX_DIST_TILES + 1.0 {
                    dists.push((e.kind, dist_tiles));
                    assert!(
                        dist_tiles >= config::CC_RESOURCE_MIN_DIST_TILES,
                        "player {} has a {:?} node too close ({:.2} tiles) to their City Centre",
                        p.id,
                        e.kind,
                        dist_tiles
                    );
                    assert!(
                        dist_tiles <= config::CC_RESOURCE_MAX_DIST_TILES,
                        "player {} has a {:?} node too far ({:.2} tiles) from their City Centre",
                        p.id,
                        e.kind,
                        dist_tiles
                    );
                }
            }
            // Sort for deterministic comparison.
            dists.sort_by(|a, b| {
                let kind_ord =
                    crate::protocol::kind_to_wire(a.0).cmp(crate::protocol::kind_to_wire(b.0));
                if kind_ord != std::cmp::Ordering::Equal {
                    return kind_ord;
                }
                a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
            });
            all_player_dists.push(dists);
        }

        // Every player in the same match must have identical distance sets.
        if let Some(first) = all_player_dists.first() {
            for (i, other) in all_player_dists.iter().enumerate().skip(1) {
                assert_eq!(
                    first.len(),
                    other.len(),
                    "player count {}: player {} has a different number of nearby resources",
                    pc,
                    i + 1
                );
                for (j, ((ek_a, da), (ek_b, db))) in first.iter().zip(other.iter()).enumerate() {
                    assert_eq!(*ek_a, *ek_b, "mismatched resource kind at index {j}");
                    assert!(
                            (da - db).abs() < 0.01,
                            "player count {pc}: resource {j} distance mismatch — player 1 has {:.3} tiles, player {} has {:.3} tiles",
                            da,
                            i + 1,
                            db
                        );
                }
            }
        }
    }
}

#[test]
fn generated_oil_nodes_spawn_at_tile_centers() {
    for player_count in 1..=4 {
        let players: Vec<PlayerInit> = (1..=player_count)
            .map(|id| PlayerInit {
                id,
                team_id: id,
                faction_id: "kriegsia".to_string(),
                name: format!("P{id}"),
                color: "#fff".into(),
                is_ai: false,
            })
            .collect();

        for seed in [0, 1, 0x1234_5678] {
            let game = Game::new_for_replay(&players, seed);
            let mut oil_count = 0;
            let mut oil_tiles = Vec::new();
            for oil in game.state.entities.iter().filter(|entity| entity.kind == EntityKind::Oil) {
                oil_count += 1;
                let (tile_x, tile_y) = game.state.map.tile_of(oil.pos_x, oil.pos_y);
                let (center_x, center_y) = game.state.map.tile_center(tile_x, tile_y);
                assert!(
                    (oil.pos_x - center_x).abs() < 0.001
                        && (oil.pos_y - center_y).abs() < 0.001,
                    "oil node {} should be centered on tile ({tile_x}, {tile_y}), got ({:.3}, {:.3}) vs center ({:.3}, {:.3})",
                    oil.id,
                    oil.pos_x,
                    oil.pos_y,
                    center_x,
                    center_y
                );
                oil_tiles.push((oil.id, tile_x, tile_y));
            }
            for (index, &(a_id, a_x, a_y)) in oil_tiles.iter().enumerate() {
                for &(b_id, b_x, b_y) in oil_tiles.iter().skip(index + 1) {
                    assert!(
                        a_x.abs_diff(b_x) > 1 || a_y.abs_diff(b_y) > 1,
                        "oil nodes {a_id} and {b_id} should have one free tile between them, got tiles ({a_x}, {a_y}) and ({b_x}, {b_y})"
                    );
                }
            }
            assert!(
                oil_count > 0,
                "generated games should include oil nodes for player count {player_count}"
            );
        }
    }
}
