use super::super::armor::ArmorAssignments;
use super::*;
use crate::sdk::{DefenseMode, DefensePlan, DefenseSector};
use rts_rules::math::GameplayMath as _;
use std::collections::{BTreeMap, BTreeSet};

fn arena(tanks: usize) -> (StartPayload, Snapshot, Vec<AiBaseSite>, DefensePlan) {
    let (mut start, mut s, _) = fixture();
    start.map.terrain.fill(0);
    start.map.unwalkable_tiles.clear();
    start.map.no_vehicle_tiles.clear();
    start.map.doodads.clear();
    start.map.resources.clear();
    s.entities
        .retain(|e| e.owner == 1 && e.kind == "resource_depot");
    let mut sites = vec![];
    let mut plan = DefensePlan::default();
    for (index, center) in [
        (40.5 * 32., 35.5 * 32.),
        (70.5 * 32., 35.5 * 32.),
        (100.5 * 32., 35.5 * 32.),
    ]
    .into_iter()
    .enumerate()
    {
        let id = AiBaseSiteId::ResourceCluster(index as u32);
        sites.push(AiBaseSite {
            id,
            position: (center.0, center.1 - 6. * 32.),
            resource_ids: vec![],
            ground_distances_from_starts: BTreeMap::new(),
        });
        building(
            &mut s,
            3000 + index as u32,
            "resource_depot",
            (center.0, center.1 - 6. * 32.),
            None,
        );
        plan.sectors.push(DefenseSector {
            id: index,
            mode: if index == 1 {
                DefenseMode::Garrison
            } else {
                DefenseMode::Cut
            },
            base_ids: vec![id],
            cut_tiles: vec![],
            center,
            asset_value: 1.,
            priority: 1. / 3.,
            enemy_approach_seconds: Some(15.),
            known_remaining_steel: 0.,
            unknown_resource_count: 0.,
            active_mine_share: 0.,
            production_share: 0.,
            tech_share: 0.,
            supported: true,
            slots: vec![],
        });
    }
    for index in 0..tanks {
        building(
            &mut s,
            1000 + index as u32,
            "tank",
            ((30.5 + index as f32 * 2.) * 32., 22.5 * 32.),
            None,
        );
    }
    (start, s, sites, plan)
}
fn commanded(requests: &[AiActionRequest]) -> BTreeSet<u32> {
    requests
        .iter()
        .flat_map(|r| match r {
            AiActionRequest::AttackMove { units, .. }
            | AiActionRequest::Move { units, .. }
            | AiActionRequest::HoldPosition { units, .. } => units.clone(),
            _ => vec![],
        })
        .collect()
}
#[test]
fn idle_armor_forms_one_compact_reachable_reserve() {
    let (start, mut s, sites, plan) = arena(6);
    let f = frame(&start, &s, &sites);
    let mut armor = ArmorAssignments::default();
    let mut actions = AiActions::for_frame(&f);
    armor.step(&f, &mut actions, &plan, &BTreeSet::new(), None);
    assert_eq!(
        commanded(&actions.into_requests()).len(),
        6,
        "every idle Tank needs an order even without a Works rally"
    );
    let posts = armor.posts();
    assert_eq!(posts.len(), 6);
    let sectors: BTreeSet<_> = posts.values().map(|(key, _)| key.clone()).collect();
    assert_eq!(
        sectors.len(),
        1,
        "armor concentrates instead of splitting among defensive sectors"
    );
    for (id, (_, p)) in &posts {
        for (other, (_, q)) in &posts {
            if id != other {
                assert!(construction::dist2(*p, *q) >= (2_f32 * 32.).det_powi(2));
            }
        }
    }
    for tank in s.entities.iter_mut().filter(|e| e.kind == "tank") {
        tank.state = "move".into();
    }
    s.tick += 9;
    let f = frame(&start, &s, &sites);
    let mut actions = AiActions::for_frame(&f);
    armor.step(&f, &mut actions, &plan, &BTreeSet::new(), None);
    assert_eq!(
        armor.posts(),
        posts,
        "reinforcement must preserve valid posts"
    );
    assert!(
        actions.into_requests().is_empty(),
        "do not overwrite a moving Tank's route each decision"
    );
    s.tick += 100;
    let f = frame(&start, &s, &sites);
    let mut actions = AiActions::for_frame(&f);
    armor.step(&f, &mut actions, &plan, &BTreeSet::new(), None);
    assert_eq!(
        commanded(&actions.into_requests()).len(),
        6,
        "stalled moving Tanks need a bounded fresh route rather than waiting forever"
    );
}
#[test]
fn armor_respects_response_ownership_then_reassigns_released_tanks() {
    let (start, mut s, sites, plan) = arena(3);
    let mut armor = ArmorAssignments::default();
    let f = frame(&start, &s, &sites);
    let mut actions = AiActions::for_frame(&f);
    armor.step(&f, &mut actions, &plan, &BTreeSet::from([1000]), None);
    assert!(!commanded(&actions.into_requests()).contains(&1000));
    assert!(!armor.posts().contains_key(&1000));
    s.tick += 9;
    let f = frame(&start, &s, &sites);
    let mut actions = AiActions::for_frame(&f);
    armor.step(&f, &mut actions, &plan, &BTreeSet::new(), None);
    assert!(
        commanded(&actions.into_requests()).contains(&1000),
        "released response Tanks must not retain a stale contact destination forever"
    );
    assert_eq!(armor.posts().len(), 3);
}
#[test]
fn armor_does_not_assign_posts_across_an_unreachable_wall() {
    let (mut start, s, sites, plan) = arena(3);
    for y in 0..start.map.height {
        start.map.terrain[(y * start.map.width + 60) as usize] = 2;
    }
    // Use the terrain's actual wire tag rather than relying on a test-only grid obstacle.
    start.map.unwalkable_tiles = (0..start.map.height)
        .map(|y| rts_sim::protocol::MapTile { x: 60, y })
        .collect();
    let f = frame(&start, &s, &sites);
    let mut armor = ArmorAssignments::default();
    let mut actions = AiActions::for_frame(&f);
    armor.step(&f, &mut actions, &plan, &BTreeSet::new(), None);
    assert_eq!(armor.posts().len(), 3);
    assert!(armor
        .posts()
        .values()
        .all(|(key, point)| key.0.is_empty() && point.0 < 60. * 32.));
}
#[test]
fn known_construction_invalidates_an_armor_post_and_replans_the_route() {
    let (start, mut s, sites, plan) = arena(1);
    let mut armor = ArmorAssignments::default();
    let f = frame(&start, &s, &sites);
    let mut actions = AiActions::for_frame(&f);
    armor.step(&f, &mut actions, &plan, &BTreeSet::new(), None);
    let old = armor.posts()[&1000].1;
    building(&mut s, 2000, "barracks", old, Some(0.1));
    s.tick += 9;
    let f = frame(&start, &s, &sites);
    let mut actions = AiActions::for_frame(&f);
    armor.step(&f, &mut actions, &plan, &BTreeSet::new(), None);
    assert_ne!(armor.posts()[&1000].1, old);
    assert!(commanded(&actions.into_requests()).contains(&1000));
}

#[test]
fn disconnected_quotas_redistribute_to_reachable_fronts_instead_of_idling_tanks() {
    let (mut start, mut s, sites, plan) = arena(3);
    start.map.unwalkable_tiles = (0..start.map.height)
        .map(|y| rts_sim::protocol::MapTile { x: 60, y })
        .collect();
    s.entities.iter_mut().find(|e| e.id == 1002).unwrap().x = 80.5 * 32.;
    let f = frame(&start, &s, &sites);
    let mut armor = ArmorAssignments::default();
    let mut actions = AiActions::for_frame(&f);
    armor.step(&f, &mut actions, &plan, &BTreeSet::new(), None);
    assert_eq!(
        armor.posts().len(),
        3,
        "an impossible right-side quota must not leave the second left-side Tank idle"
    );
    assert_eq!(
        armor
            .posts()
            .values()
            .filter(|(_, point)| point.0 < 60. * 32.)
            .count(),
        2
    );
    assert_eq!(commanded(&actions.into_requests()).len(), 3);
}
#[test]
fn armor_can_cover_fronts_without_entrenchable_infantry_posts() {
    let (start, s, sites, mut plan) = arena(3);
    for sector in &mut plan.sectors {
        sector.supported = false;
    }
    let f = frame(&start, &s, &sites);
    let mut armor = ArmorAssignments::default();
    let mut actions = AiActions::for_frame(&f);
    armor.step(&f, &mut actions, &plan, &BTreeSet::new(), None);
    assert_eq!(
        armor.posts().len(),
        3,
        "infantry entrenchment support is not Tank movement support"
    );
    assert_eq!(commanded(&actions.into_requests()).len(), 3);
}

#[test]
fn reserve_remains_compact_when_infantry_uses_different_fronts() {
    let (start, s, sites, plan) = arena(12);
    let f = frame(&start, &s, &sites);
    let mut armor = ArmorAssignments::default();
    armor.step(
        &f,
        &mut AiActions::for_frame(&f),
        &plan,
        &BTreeSet::new(),
        None,
    );
    let posts = armor.posts();
    assert_eq!(posts.len(), 12);
    let positions: Vec<_> = posts.values().map(|(_, p)| *p).collect();
    assert!(positions.iter().all(|p| positions
        .iter()
        .all(|q| construction::dist2(*p, *q) < (10_f32 * 32.).det_powi(2))));
}
#[test]
fn depleted_base_stops_pulling_the_reserve_centroid() {
    let (mut start, mut s, mut sites, plan) = arena(3);
    let template = fixture()
        .0
        .map
        .resources
        .into_iter()
        .find(|r| r.kind == "steel")
        .unwrap();
    for (i, site) in sites.iter_mut().enumerate() {
        let mut resource = template.clone();
        resource.id = 9000 + i as u32;
        resource.x = site.position.0;
        resource.y = site.position.1;
        site.resource_ids = vec![resource.id];
        start.map.resources.push(resource.clone());
        let mut node = s.entities[0].clone();
        node.id = resource.id;
        node.owner = rts_sim::game::entity::NEUTRAL;
        node.kind = "steel".into();
        node.x = resource.x;
        node.y = resource.y;
        node.remaining = Some(if i == 2 { 625 } else { 0 });
        s.entities.push(node);
    }
    s.resource_deltas.clear();
    let f = frame(&start, &s, &sites);
    let mut armor = ArmorAssignments::default();
    armor.step(
        &f,
        &mut AiActions::for_frame(&f),
        &plan,
        &BTreeSet::new(),
        None,
    );
    assert_eq!(armor.posts().len(), 3);
    assert!(armor
        .posts()
        .values()
        .all(|(_, p)| construction::dist2(*p, sites[2].position) < (6_f32 * 32.).det_powi(2)));
}

#[test]
fn returning_armor_keeps_every_route_bend_in_the_queued_batch() {
    let (mut start, s, sites, plan) = arena(1);
    start.map.unwalkable_tiles = (0..start.map.height)
        .filter(|y| !(10..=14).contains(y))
        .map(|y| rts_sim::protocol::MapTile { x: 60, y })
        .collect();
    let f = frame(&start, &s, &sites);
    let mut armor = ArmorAssignments::default();
    let mut actions = AiActions::for_frame(&f);
    armor.step(&f, &mut actions, &plan, &BTreeSet::new(), None);
    let goal = armor.posts()[&1000].1;
    let moves: Vec<_> = actions
        .into_requests()
        .into_iter()
        .filter_map(|r| match r {
            AiActionRequest::Move {
                units,
                x,
                y,
                queued,
            } if units.contains(&1000) => Some(((x, y), queued)),
            _ => None,
        })
        .collect();
    assert!(
        moves.len() > 1,
        "a wall detour needs queued corners, not only the first bend"
    );
    assert!(!moves[0].1);
    assert!(moves.iter().skip(1).all(|(_, queued)| *queued));
    assert_eq!(
        moves.last().unwrap().0,
        goal,
        "the return order must reach the staging slot"
    );
}
