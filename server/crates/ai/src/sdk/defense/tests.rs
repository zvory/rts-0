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
    assert!(full_value > empty_value);
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
