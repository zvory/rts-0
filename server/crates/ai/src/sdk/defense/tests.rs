use super::*;
use crate::ai_core::map_analysis::AiStaticMapContextCache;
use rts_sim::game::map::Map;
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::{Snapshot, StartPayload};
use std::time::Instant;
fn fixture(
    name: &str,
    player: u32,
    n: usize,
) -> (
    StartPayload,
    Snapshot,
    Vec<super::super::AiBaseSite>,
    Vec<AiBaseSiteId>,
) {
    let players = [1, 2].map(|id| PlayerInit {
        id,
        team_id: id,
        faction_id: "kriegsia".into(),
        name: format!("P{id}"),
        color: "#ffffff".into(),
        is_ai: true,
    });
    let game =
        Game::new_with_random_ai_profiles_and_map(&players, 7, Map::load(name, 2, 7).unwrap());
    let start = game.start_payload();
    let mut snapshot = game.snapshot_for(player);
    let mut cache = AiStaticMapContextCache::default();
    let sites = cache.get_or_analyze(&start).base_sites().to_vec();
    let frame = AiFrame::from_host(&start, &snapshot, player, [], Some(&[1, 2]))
        .unwrap()
        .with_base_sites(&sites);
    let ids = WorldQueries::new(&frame)
        .ranked_bases(player)
        .iter()
        .take(n)
        .map(|b| b.site.id)
        .collect::<Vec<_>>();
    let specimen = snapshot
        .entities
        .iter()
        .find(|e| e.owner == player && e.kind == "resource_depot")
        .unwrap()
        .clone();
    snapshot.entities.retain(|e| e.owner != player);
    for (j, id) in ids.iter().enumerate() {
        let site = sites.iter().find(|s| s.id == *id).unwrap();
        let mut d = specimen.clone();
        d.id = 900000 + j as u32;
        d.x = site.position.0;
        d.y = site.position.1;
        snapshot.entities.push(d);
    }
    (start, snapshot, sites, ids)
}
fn frame(
    start: &StartPayload,
    snapshot: &Snapshot,
    sites: &[super::super::AiBaseSite],
    player: u32,
) -> AiFrame {
    AiFrame::from_host(start, snapshot, player, [], Some(&[1, 2]))
        .unwrap()
        .with_base_sites(sites)
}
fn matrix(name: &str) {
    for player in [1, 2] {
        let (start, mut snapshot, sites, all) = fixture(name, player, 4);
        let mut planner = DefensePlanner::with_firing_range_tiles(4.8);
        for n in 1..=4 {
            let f = frame(&start, &snapshot, &sites, player);
            let begin = Instant::now();
            let p = planner.plan(&f, &all[..n], 12);
            eprintln!(
                "{name} P{player} N{n} cold {:?} flows{} sectors{}",
                begin.elapsed(),
                planner.geometry_builds,
                p.sectors.len()
            );
            assert!(!p.sectors.is_empty());
            assert_eq!(p.slots.len(), 12, "{name} P{player} N{n}");
            for slot in &p.slots {
                let g = planner.grid.as_ref().unwrap();
                let i = g.index(slot.position).unwrap();
                assert!(g.walk[i] && g.entrench[i]);
                let geo = &planner.geometry[slot.sector_id];
                assert!(geo.points.iter().any(|p| dist2(g.point(*p), slot.position)
                    <= 4.8 * 4.8 * g.size * g.size
                    && g.ray(i, *p)));
            }
            let count = planner.geometry_builds;
            let begin = Instant::now();
            assert_eq!(p, planner.plan(&f, &all[..n], 12));
            eprintln!("warm {:?}", begin.elapsed());
            assert_eq!(count, planner.geometry_builds);
            for budget in [0, 1, 2, 5, 50, 100] {
                let p = planner.plan(&f, &all[..n], budget);
                assert_eq!(p.slots.len() + p.unallocated_budget, budget);
                assert!(p.slots.len() <= budget);
                let unique: BTreeSet<_> = p
                    .slots
                    .iter()
                    .map(|s| ((s.position.0 * 16.) as i32, (s.position.1 * 16.) as i32))
                    .collect();
                assert_eq!(unique.len(), p.slots.len());
            }
            assert_eq!(
                count, planner.geometry_builds,
                "budget must never recompute flow"
            );
            snapshot.tick += 90;
        }
    }
}
#[test]
fn classic_both_players_all_four_ranks() {
    matrix("Classic");
}
#[test]
fn crossroads_both_players_all_four_ranks() {
    matrix("Crossroads");
}
#[test]
fn schone_both_players_all_four_ranks() {
    matrix("Schone Tage");
}
#[test]
fn depleted_stock_lowers_value_unknown_stays_unknown_and_activity_is_actual() {
    let (start, mut snapshot, sites, ids) = fixture("Classic", 1, 1);
    let site = sites.iter().find(|s| s.id == ids[0]).unwrap();
    let resource = start
        .map
        .resources
        .iter()
        .find(|r| site.resource_ids.contains(&r.id) && r.kind == "steel")
        .unwrap();
    snapshot.resource_deltas.clear();
    snapshot.entities.retain(|e| e.id != resource.id);
    let f = frame(&start, &snapshot, &sites, 1);
    let g = Grid::new(f.map());
    let before = scoring::assets(&g, &f, &ids);
    assert!(before.iter().any(|a| a.unknown > 0.));
    assert!(f
        .resources()
        .iter()
        .any(|r| r.id == resource.id && r.remaining == AiResourceAmount::Unknown));
    let mut node = snapshot.entities[0].clone();
    node.id = resource.id;
    node.owner = rts_sim::game::entity::NEUTRAL;
    node.kind = "steel".into();
    node.x = resource.x;
    node.y = resource.y;
    node.remaining = Some(625);
    snapshot.entities.push(node.clone());
    let full = frame(&start, &snapshot, &sites, 1);
    let full_value: f32 = scoring::assets(&g, &full, &ids)
        .iter()
        .map(|a| a.value)
        .sum();
    snapshot.entities.last_mut().unwrap().remaining = Some(0);
    let empty = frame(&start, &snapshot, &sites, 1);
    let empty_value: f32 = scoring::assets(&g, &empty, &ids)
        .iter()
        .map(|a| a.value)
        .sum();
    assert!(
        (full_value - empty_value - 6.25).abs() < 0.001,
        "all 625 remaining Steel must count, without a cap or unmined discount"
    );
}
#[test]
fn unsupported_no_entrenchment_returns_unallocated_and_ignores_unowned_sites() {
    let (mut start, snapshot, sites, ids) = fixture("Classic", 1, 1);
    start.map.no_entrenchment_tiles = (0..start.map.height)
        .flat_map(|y| (0..start.map.width).map(move |x| rts_sim::protocol::MapTile { x, y }))
        .collect();
    let f = frame(&start, &snapshot, &sites, 1);
    let p = DefensePlanner::default().plan(&f, &ids, 12);
    assert!(p.slots.is_empty());
    assert_eq!(p.unallocated_budget, 12);
    assert!(p.sectors.iter().all(|s| !s.supported));
    let p = DefensePlanner::default().plan(&f, &[AiBaseSiteId::Start(2)], 12);
    assert!(p.sectors.is_empty());
}

#[test]
fn economic_shares_conserve_actual_mines_stock_and_production_without_reflow() {
    let (start, mut snapshot, sites, ids) = fixture("Classic", 1, 2);
    let site = sites.iter().find(|s| s.id == ids[0]).unwrap();
    let resource = start
        .map
        .resources
        .iter()
        .find(|r| site.resource_ids.contains(&r.id) && r.kind == "steel")
        .unwrap();
    let mut mine = snapshot
        .entities
        .iter()
        .find(|e| e.owner == 1)
        .unwrap()
        .clone();
    mine.id = 910000;
    mine.kind = "steel_mine".into();
    mine.x = resource.x;
    mine.y = resource.y;
    mine.extractor_active = Some(true);
    mine.latched_node = Some(resource.id);
    snapshot.entities.retain(|e| e.id != resource.id);
    let mut node = mine.clone();
    node.owner = rts_sim::game::entity::NEUTRAL;
    node.id = resource.id;
    node.kind = "steel".into();
    node.remaining = Some(625);
    node.extractor_active = None;
    snapshot.entities.push(node);
    snapshot.entities.push(mine);
    let mut planner = DefensePlanner::default();
    let f = frame(&start, &snapshot, &sites, 1);
    let before = planner.plan(&f, &ids, 12);
    let flows = planner.geometry_builds;
    assert!(
        (before
            .sectors
            .iter()
            .map(|s| s.active_mine_share)
            .sum::<f32>()
            - 1.)
            .abs()
            < 0.001
    );
    assert!(
        before
            .sectors
            .iter()
            .map(|s| s.known_remaining_steel)
            .sum::<f32>()
            >= 625.
    );
    let before_value = before.sectors.iter().map(|s| s.asset_value).sum::<f32>();
    let mut barracks = snapshot
        .entities
        .iter()
        .find(|e| e.kind == "resource_depot" && e.owner == 1)
        .unwrap()
        .clone();
    barracks.id = 920000;
    barracks.kind = "barracks".into();
    barracks.x += 128.;
    snapshot.entities.push(barracks);
    let changed = planner.plan(&frame(&start, &snapshot, &sites, 1), &ids, 12);
    assert_eq!(planner.geometry_builds, flows);
    assert!(
        (changed
            .sectors
            .iter()
            .map(|s| s.production_share)
            .sum::<f32>()
            - 1.)
            .abs()
            < 0.001
    );
    assert!(
        (changed.sectors.iter().map(|s| s.asset_value).sum::<f32>() - before_value - 3.).abs()
            < 0.001
    );
    snapshot
        .entities
        .iter_mut()
        .find(|e| e.id == resource.id)
        .unwrap()
        .remaining = Some(0);
    snapshot.tick += 90;
    let depleted = planner.plan(&frame(&start, &snapshot, &sites, 1), &ids, 12);
    assert!(
        depleted.sectors.iter().map(|s| s.asset_value).sum::<f32>()
            < changed.sectors.iter().map(|s| s.asset_value).sum::<f32>()
    );
    assert_eq!(planner.geometry_builds, flows);
}

#[test]
fn ground_approach_changes_priorities_and_known_footprint_immediately_evicts_slots() {
    let (mut start, mut snapshot, sites, ids) = fixture("Classic", 1, 1);
    let mut planner = DefensePlanner::with_firing_range_tiles(4.8);
    let f = frame(&start, &snapshot, &sites, 1);
    let before = planner.plan(&f, &ids, 12);
    assert!(before.sectors.len() > 1);
    let target = &before.sectors[0];
    let tile = (target.center.0 / 32.) as u32;
    let row = (target.center.1 / 32.) as u32;
    let enemy = start.players.iter_mut().find(|p| p.id == 2).unwrap();
    enemy.start_tile_x = tile;
    enemy.start_tile_y = row;
    let close = planner.plan(&frame(&start, &snapshot, &sites, 1), &ids, 12);
    assert!(close.sectors[0].enemy_approach_seconds < before.sectors[0].enemy_approach_seconds);
    assert!(close.sectors[0].priority > before.sectors[0].priority);
    let old_slot = close.slots[0];
    let mut obstacle = snapshot
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "resource_depot")
        .unwrap()
        .clone();
    obstacle.id = 930000;
    obstacle.x = old_slot.position.0;
    obstacle.y = old_slot.position.1;
    snapshot.entities.push(obstacle);
    let flows = planner.geometry_builds;
    let after = planner.plan(&frame(&start, &snapshot, &sites, 1), &ids, 12);
    assert!(!after.slots.iter().any(|s| s.position == old_slot.position));
    assert_eq!(planner.geometry_builds, flows);
}

#[test]
fn arbitrary_base_counts_and_fresh_four_base_cache_cost() {
    let (start, snapshot, sites, ids) = fixture("Schone Tage", 1, 6);
    assert_eq!(ids.len(), 6);
    let f = frame(&start, &snapshot, &sites, 1);
    let mut planner = DefensePlanner::default();
    let now = Instant::now();
    let p = planner.plan(&f, &ids[..4], 12);
    let cold = now.elapsed();
    assert_eq!(p.slots.len(), 12);
    let flow = planner.geometry_builds;
    let now = Instant::now();
    for budget in 1..=100 {
        let p = planner.plan(&f, &ids[..4], budget);
        assert_eq!(p.slots.len() + p.unallocated_budget, budget);
    }
    eprintln!(
        "fresh Schone4 {cold:?}; budgets1..100 {:?}; flows{flow}",
        now.elapsed()
    );
    assert_eq!(planner.geometry_builds, flow);
    let p = planner.plan(&f, &ids, 24);
    assert!(
        p.sectors
            .iter()
            .flat_map(|s| s.base_ids.iter())
            .collect::<BTreeSet<_>>()
            .len()
            == 6
    );
    assert_eq!(p.slots.len() + p.unallocated_budget, 24);
}

#[test]
fn firing_rays_check_opaque_corners_known_footprints_and_forest_exit() {
    let (start, snapshot, sites, _) = fixture("Classic", 1, 1);
    let mut map = frame(&start, &snapshot, &sites, 1).map().clone();
    map.width = 8;
    map.height = 3;
    map.terrain = vec![AiTerrain::Grass; 24];
    map.no_building_tiles.clear();
    map.unwalkable_tiles.clear();
    map.no_entrenchment_tiles.clear();
    map.slow_movement_tiles.clear();
    map.concealment_tiles.clear();
    map.terrain[1] = AiTerrain::Rock;
    let g = Grid::new(&map);
    assert!(!g.ray(0, 18));
    map.terrain[1] = AiTerrain::Grass;
    map.concealment_tiles.insert((2, 1));
    let g = Grid::new(&map);
    assert!(!g.ray(8, 12));
    assert!(g.ray(10, 12));
    map.concealment_tiles.clear();
    let g = Grid::new(&map);
    assert!(g.ray(8, 12));
    let mut occupied = vec![false; 24];
    occupied[10] = true;
    assert!(!g.ray_blocked(8, 12, &occupied));
}
#[test]
fn long_flow_paths_use_heap_not_recursive_simulation_stack() {
    let n = 40_000;
    let mut f = flow::Flow::new(n);
    for i in 0..n - 1 {
        f.add(i, i + 1, 1);
    }
    let seen = f.cut(0, n - 1, 100).unwrap();
    assert!(seen[0]);
    assert!(!seen[n - 1]);
}

#[test]
fn public_enemy_elimination_updates_threat_without_rebuilding_static_cuts() {
    let (start, snapshot, sites, ids) = fixture("Classic", 1, 2);
    let mut planner = DefensePlanner::default();
    let alive = frame(&start, &snapshot, &sites, 1);
    let before = planner.plan(&alive, &ids, 12);
    assert_eq!(before.slots.len(), 12);
    let flows = planner.geometry_builds;
    let eliminated = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1]))
        .unwrap()
        .with_base_sites(&sites);
    let after = planner.plan(&eliminated, &ids, 12);
    assert_eq!(flows, planner.geometry_builds);
    assert!(after
        .sectors
        .iter()
        .all(|s| s.enemy_approach_seconds.is_none() && s.priority == 0.));
    assert_eq!(after.unallocated_budget, 12);
    let restored = planner.plan(&alive, &ids, 12);
    assert_eq!(flows, planner.geometry_builds);
    assert_eq!(restored, before);
}

#[test]
fn prospective_line_requires_a_reachable_public_site_without_hostile_depot() {
    let (start, mut snapshot, sites, owned) = fixture("Classic", 1, 1);
    let f = frame(&start, &snapshot, &sites, 1);
    let query = WorldQueries::new(&f);
    let candidate = query
        .ranked_bases(1)
        .into_iter()
        .find(|b| {
            !owned.contains(&b.site.id)
                && b.distance_from_main.is_some()
                && !matches!(b.site.id, AiBaseSiteId::Start(_))
        })
        .unwrap();
    let mut planner = DefensePlanner::with_firing_range_tiles(4.8);
    assert!(planner.plan(&f, &[candidate.site.id], 8).slots.is_empty());
    assert!(!planner
        .plan_expansion(&f, candidate.site.id, 8)
        .slots
        .is_empty());
    assert!(planner
        .plan_expansion(&f, AiBaseSiteId::Start(2), 8)
        .slots
        .is_empty());
    assert!(planner
        .plan_expansion(&f, AiBaseSiteId::ResourceCluster(u32::MAX), 8)
        .slots
        .is_empty());
    let mut enemy = snapshot
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "resource_depot")
        .unwrap()
        .clone();
    enemy.id = 999999;
    enemy.owner = 2;
    enemy.x = candidate.site.position.0;
    enemy.y = candidate.site.position.1;
    snapshot.entities.push(enemy);
    let f = frame(&start, &snapshot, &sites, 1);
    assert!(planner
        .plan_expansion(&f, candidate.site.id, 8)
        .slots
        .is_empty());
}

#[test]
fn prospective_construction_is_reserved_before_slot_allocation_and_cache_reuse() {
    for player in [1, 2] {
        let (start, mut snapshot, sites, _) = fixture("Classic", player, 1);
        let f = frame(&start, &snapshot, &sites, player);
        let q = WorldQueries::new(&f);
        let site = q
            .ranked_bases(player)
            .into_iter()
            .find(|b| {
                q.known_base_depots(b.site.id).is_empty()
                    && !matches!(b.site.id,AiBaseSiteId::Start(p) if p!=player)
            })
            .unwrap()
            .site
            .id;
        let tile = q.base_depot_site(site, &Default::default()).unwrap();
        let foot = super::super::AiRulebook::for_frame(&f)
            .unwrap()
            .entity(EntityKind::ResourceDepot)
            .unwrap()
            .footprint
            .unwrap();
        let mut planner = DefensePlanner::with_firing_range_tiles(4.8);
        let plain = planner.plan_expansion(&f, site, 5);
        eprintln!(
            "Classic P{player} prospective modes: {:?}",
            plain.sectors.iter().map(|s| s.mode).collect::<Vec<_>>()
        );
        for shifted in [tile, q.tile(tile.x() + 1, tile.y()).unwrap(), tile] {
            let reservation = DefenseReservation {
                building: EntityKind::ResourceDepot,
                tile: shifted,
            };
            let plan = planner.plan_expansion_reserved(&f, site, 5, &[reservation]);
            assert_eq!(
                plan.slots.len(),
                5,
                "allocate replacement posts rather than filter an already allocated plan"
            );
            let ts = f.map().tile_size as f32;
            let rect = (
                shifted.x() as f32 * ts,
                shifted.y() as f32 * ts,
                (shifted.x() + foot.width_tiles) as f32 * ts,
                (shifted.y() + foot.height_tiles) as f32 * ts,
            );
            for (i, slot) in plan.slots.iter().enumerate() {
                assert!(!super::super::unit_circle_touches_rect(
                    slot.position,
                    rts_rules::balance::unit_placement_radius(EntityKind::MachineGunner) + ts,
                    rect
                ));
                assert!(plan.slots[..i]
                    .iter()
                    .all(|s| dist2(s.position, slot.position) > ts * ts));
                assert!(plan.sectors[slot.sector_id].slots.contains(slot));
            }
            assert_eq!(
                plan,
                planner.plan_expansion_reserved(&f, site, 5, &[reservation])
            );
        }
        // Adding an actual scaffold must keep both the footprint and access ring clear.
        let mut scaffold = snapshot
            .entities
            .iter()
            .find(|e| e.owner == player)
            .unwrap()
            .clone();
        scaffold.id = 9999;
        scaffold.x = (tile.x() as f32 + foot.width_tiles as f32 / 2.) * start.map.tile_size as f32;
        scaffold.y = (tile.y() as f32 + foot.height_tiles as f32 / 2.) * start.map.tile_size as f32;
        scaffold.build_progress = Some(0.1);
        snapshot.entities.push(scaffold);
        let f = frame(&start, &snapshot, &sites, player);
        assert_eq!(
            planner
                .plan_expansion_reserved(
                    &f,
                    site,
                    5,
                    &[DefenseReservation {
                        building: EntityKind::ResourceDepot,
                        tile
                    }]
                )
                .slots
                .len(),
            5
        );
    }
}

#[test]
fn adding_fifth_base_keeps_useful_shared_cuts_from_four_base_plan() {
    for player in [1, 2] {
        let (start, snapshot, sites, ids) = fixture("Classic", player, 5);
        assert_eq!(ids.len(), 5);
        let f = frame(&start, &snapshot, &sites, player);
        let mut planner = DefensePlanner::with_firing_range_tiles(4.8);
        planner.plan(&f, &ids[..4], 60);
        let previous = planner
            .geometry
            .iter()
            .filter(|sector| sector.mode == DefenseMode::Cut && sector.bases.len() > 1)
            .map(|sector| sector.bases.clone())
            .collect::<BTreeSet<_>>();
        assert!(
            !previous.is_empty(),
            "Classic P{player} must exercise a shared cut"
        );
        let before_cost = planner
            .geometry
            .iter()
            .map(|sector| sector.bases.clone())
            .collect::<BTreeSet<_>>()
            .iter()
            .map(|ids| {
                planner
                    .cuts
                    .get(ids)
                    .and_then(Option::as_ref)
                    .map_or(43, |c| c.cost)
            })
            .sum::<usize>();
        let flows = planner.geometry_builds;
        let after = planner.plan(&f, &ids, 60);
        let selected = planner
            .geometry
            .iter()
            .map(|sector| sector.bases.clone())
            .collect::<BTreeSet<_>>();
        eprintln!("Classic P{player} shared={previous:?} five={selected:?}");
        assert!(
            previous.iter().all(|group| selected
                .iter()
                .any(|new| group.iter().all(|id| new.contains(id)))),
            "adding a base must retain or profitably extend the existing shared cuts"
        );
        let after_cost = selected
            .iter()
            .map(|ids| {
                planner
                    .cuts
                    .get(ids)
                    .and_then(Option::as_ref)
                    .map_or(43, |c| c.cost)
            })
            .sum::<usize>();
        assert!(after_cost <= before_cost + 43);
        assert!(
            planner.geometry_builds - flows <= 69,
            "two bounded 32-merge seeds plus singleton cuts"
        );
        let flows = planner.geometry_builds;
        assert_eq!(after, planner.plan(&f, &ids, 60));
        assert_eq!(flows, planner.geometry_builds);
        assert_eq!(after.slots.len() + after.unallocated_budget, 60);
    }
}

#[test]
fn shared_cut_without_profitable_intermediate_pairs_survives_fifth_site() {
    let (start, snapshot, sites, ids) = fixture("Classic", 1, 6);
    let f = frame(&start, &snapshot, &sites, 1);
    let mut planner = DefensePlanner::default();
    planner.grid = Some(Grid::new(f.map()));
    planner.enemy = vec![FAR; planner.grid.as_ref().unwrap().walk.len()];
    let mut ordered = ids.clone();
    ordered.sort_unstable();
    // Isolate a legitimate min-cut selection shape: no pair has a useful cut, but protecting
    // three nearby sites together exposes a cheap shared choke. Singleton agglomeration
    // cannot reach that partition, even with an unlimited staffing budget.
    for mask in 1..1usize << ordered.len() {
        let group = ordered
            .iter()
            .enumerate()
            .filter_map(|(i, id)| (mask & (1 << i) != 0).then_some(*id))
            .collect::<Vec<_>>();
        planner.cuts.insert(group, None);
    }
    let shared = ordered[..3].to_vec();
    let point = planner
        .grid
        .as_ref()
        .unwrap()
        .near(sites[0].position)
        .unwrap();
    planner.cuts.insert(
        shared.clone(),
        Some(geometry::Cut {
            components: vec![vec![point]],
            demand: 2,
            cost: 10,
        }),
    );
    planner.geometry_capacity = 60;
    planner.owned = ordered[..4].to_vec();
    planner.geometry = geometry::build(&mut planner, &f);
    assert!(planner.geometry.iter().any(|g| g.bases == shared));
    planner.owned = ordered[..5].to_vec();
    planner.geometry = geometry::build(&mut planner, &f);
    assert!(
        planner.geometry.iter().any(|g| g.bases == shared),
        "the fifth site must not discard the selected three-site choke"
    );
    assert_eq!(planner.geometry_builds, 0, "reuse cached cuts");
    // A destroyed base invalidates its whole old group; no hidden protection of lost sites.
    planner.owned.retain(|id| *id != shared[0]);
    planner.owned.push(ordered[5]);
    planner.owned.sort_unstable();
    planner.geometry = geometry::build(&mut planner, &f);
    assert!(planner
        .geometry
        .iter()
        .all(|g| !g.bases.contains(&shared[0])));
}

#[test]
fn resource_defense_value_tracks_stock_instead_of_mine_activity() {
    let (start, mut snapshot, sites, ids) = fixture("Classic", 1, 1);
    let site = sites.iter().find(|s| s.id == ids[0]).unwrap();
    let resource = start
        .map
        .resources
        .iter()
        .find(|r| site.resource_ids.contains(&r.id) && r.kind == "steel")
        .unwrap();
    snapshot.resource_deltas.clear();
    snapshot
        .entities
        .retain(|e| e.id != resource.id && e.kind != "steel_mine");
    let mut node = snapshot.entities[0].clone();
    node.id = resource.id;
    node.owner = rts_sim::game::entity::NEUTRAL;
    node.kind = "steel".into();
    node.x = resource.x;
    node.y = resource.y;
    node.remaining = Some(625);
    snapshot.entities.push(node);
    let g = Grid::new(frame(&start, &snapshot, &sites, 1).map());
    let value = |snapshot: &Snapshot| {
        scoring::assets(&g, &frame(&start, snapshot, &sites, 1), &ids)
            .iter()
            .map(|a| a.value)
            .sum::<f32>()
    };
    let fresh = value(&snapshot);
    let mut mine = snapshot
        .entities
        .iter()
        .find(|e| e.owner == 1)
        .unwrap()
        .clone();
    mine.id = 910000;
    mine.kind = "steel_mine".into();
    mine.x = resource.x;
    mine.y = resource.y;
    mine.latched_node = Some(resource.id);
    mine.extractor_active = Some(false);
    snapshot.entities.push(mine);
    for (progress, active) in [(Some(0.5), false), (None, false), (None, true)] {
        let mine = snapshot
            .entities
            .iter_mut()
            .find(|e| e.id == 910000)
            .unwrap();
        mine.build_progress = progress;
        mine.extractor_active = Some(active);
        assert!(
            (value(&snapshot) - fresh).abs() < 0.001,
            "starting, finishing, or activating a mine must not change defensive stock value"
        );
    }
    for remaining in [500, 250, 100, 0] {
        snapshot
            .entities
            .iter_mut()
            .find(|e| e.id == resource.id)
            .unwrap()
            .remaining = Some(remaining);
        assert!(
            (fresh - value(&snapshot) - (625 - remaining) as f32 / 100.).abs() < 0.001,
            "defensive value must fall proportionally to extracted Steel"
        );
    }
    assert!(
        fresh > value(&snapshot),
        "a fresh unmined base must retain more resource value than a depleted active mine"
    );
    snapshot
        .entities
        .retain(|e| e.id != resource.id && e.id != 910000);
    let unknown = value(&snapshot);
    assert!(
        (fresh - unknown - 3.125).abs() < 0.001,
        "unseen stock uses a half-patch estimate, without inventing exact known stock"
    );
    assert!(frame(&start, &snapshot, &sites, 1)
        .resources()
        .iter()
        .any(|r| r.id == resource.id && r.remaining == AiResourceAmount::Unknown));
}

#[test]
fn garrison_posts_surround_base_on_bounded_ring_and_keep_construction_access_clear() {
    for player in [1, 2] {
        let (start, snapshot, sites, ids) = fixture("Classic", player, 1);
        let f = frame(&start, &snapshot, &sites, player);
        let base = sites.iter().find(|site| site.id == ids[0]).unwrap();
        let ts = f.map().tile_size as f32;
        let mut planner = DefensePlanner::with_firing_range_tiles(4.8);
        // Exercise the fallback policy independently of the map's selected min-cut.
        planner.plan(&f, &ids, 8);
        planner.cuts.insert(vec![ids[0]], None);
        planner.last_plan = None;
        let ring = planner.plan(&f, &ids, 8);
        assert_eq!(ring.slots.len(), 8);
        assert_eq!(ring.sectors.len(), 1);
        assert_eq!(ring.sectors[0].mode, DefenseMode::Garrison);
        assert!(dist2(ring.sectors[0].center, base.position) <= ts * ts);
        let mut quadrants = BTreeSet::new();
        for slot in &ring.slots {
            let radius = dist2(slot.position, base.position) / (ts * ts);
            assert!(
                (9. ..=25.).contains(&radius),
                "P{player} radius squared={radius}"
            );
            quadrants.insert((
                slot.position.0 >= base.position.0,
                slot.position.1 >= base.position.1,
            ));
        }
        assert_eq!(
            quadrants.len(),
            4,
            "garrison must surround the base, not cluster forward"
        );
        let p = ring.slots[0].position;
        let tile = WorldQueries::new(&f)
            .tile((p.0 / ts) as u32, (p.1 / ts) as u32)
            .unwrap();
        let reservation = DefenseReservation {
            building: EntityKind::Barracks,
            tile,
        };
        let foot = super::super::AiRulebook::for_frame(&f)
            .unwrap()
            .entity(reservation.building)
            .unwrap()
            .footprint
            .unwrap();
        let rect = (
            tile.x() as f32 * ts,
            tile.y() as f32 * ts,
            (tile.x() + foot.width_tiles) as f32 * ts,
            (tile.y() + foot.height_tiles) as f32 * ts,
        );
        let reserved = planner.plan_sites(&f, &ids, None, 256, &[reservation], false);
        assert!(!reserved.slots.is_empty());
        assert!(
            reserved.unallocated_budget > 0,
            "exhausted ring must not invent fallback posts"
        );
        for (i, slot) in reserved.slots.iter().enumerate() {
            assert!((9. ..=25.).contains(&(dist2(slot.position, base.position) / (ts * ts))));
            assert!(!super::super::unit_circle_touches_rect(
                slot.position,
                rts_rules::balance::unit_placement_radius(EntityKind::MachineGunner) + ts,
                rect
            ));
            assert!(
                reserved.slots[..i].iter().all(|previous| {
                    dist2(previous.position, slot.position) > 1.4 * 1.4 * ts * ts
                }),
                "fallback must never pile multiple posts onto one point"
            );
        }
    }
}
