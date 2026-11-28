use super::*;
use rts_rules::math::GameplayMath as _;
use std::collections::BTreeSet;

fn two_base_scene(count: usize) -> (StartPayload, Snapshot, Vec<AiBaseSite>) {
    let (start, mut s, sites) = fixture();
    let template = s
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "rifleman")
        .unwrap()
        .clone();
    s.entities.retain(|e| e.kind != "rifleman");
    let home = sites
        .iter()
        .find(|b| b.id == AiBaseSiteId::Start(1))
        .unwrap();
    let second = sites
        .iter()
        .filter(|b| b.id != home.id && !matches!(b.id, AiBaseSiteId::Start(_)))
        .min_by(|a, b| {
            construction::dist2(home.position, a.position)
                .total_cmp(&construction::dist2(home.position, b.position))
        })
        .unwrap();
    building(&mut s, 800, "resource_depot", second.position, None);
    let f = frame(&start, &s, &sites);
    let owned = owned_sites(&f);
    let plan = DefensePlanner::with_garrison_posts_per_base(4).plan(&f, &owned, 8);
    let rings = [home.id, second.id].map(|id| {
        plan.sectors
            .iter()
            .find(|sector| sector.base_ids == [id])
            .unwrap()
            .slots
            .clone()
    });
    for i in 0..count {
        let mut e = template.clone();
        e.id = 1000 + i as u32;
        let post = rings[i % 2][(i / 2) % 4].position;
        e.x = post.0;
        e.y = post.1;
        e.hp = 100;
        e.max_hp = 100;
        s.entities.push(e);
    }
    (start, s, sites)
}
#[test]
fn responses_and_outbound_pushes_leave_four_infantry_at_each_base() {
    let (start, mut s, sites) = two_base_scene(24);
    let f = frame(&start, &s, &sites);
    let owned = owned_sites(&f);
    let mut defense = tactics::DefenseAssignments::default();
    let protected = defense.prepare_sentries(&f, &owned);
    assert_eq!(protected.len(), 8);
    let initial = defense.assigned_sentry_posts().clone();
    let mut enemy = s.entities.iter().find(|e| e.id == 1000).unwrap().clone();
    enemy.id = 9000;
    enemy.owner = 2;
    enemy.kind = "tank".into();
    enemy.x = initial.values().next().unwrap().0;
    enemy.y = initial.values().next().unwrap().1;
    enemy.target_id = protected.iter().next().copied();
    s.entities.push(enemy);
    s.tick += 9;
    let f = frame(&start, &s, &sites);
    let protected_now = defense.prepare_sentries(&f, &owned);
    assert_eq!(protected_now, protected);
    let outbound = f
        .owned()
        .iter()
        .filter(|e| tactics::infantry(e.kind) && !protected.contains(&e.id))
        .map(|e| e.id)
        .collect();
    let mut actions = AiActions::for_frame(&f);
    let mut planner = DefensePlanner::default();
    let ownership = defense.step_with_outbound(&f, &mut actions, &mut planner, &owned, &outbound);
    assert!(ownership.active.is_superset(&protected));
    assert_eq!(defense.assigned_sentry_posts(), &initial);
    let requests = actions.into_requests();
    let first_guards = requests
        .iter()
        .take(protected.len())
        .flat_map(|r| match r {
            AiActionRequest::Move { units, .. } | AiActionRequest::HoldPosition { units, .. } => {
                units.clone()
            }
            _ => Vec::new(),
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        first_guards, protected,
        "observation guard commands precede combat batches"
    );
    assert!(!requests.iter().any(|r| matches!(r,AiActionRequest::AttackMove{units,x,y,..} if units.iter().any(|id|protected.contains(id)) && *x==enemy_position(&s).0 && *y==enemy_position(&s).1)));
    for site in sites.iter().filter(|site| owned.contains(&site.id)) {
        assert_eq!(
            initial
                .values()
                .filter(|p| construction::dist2(**p, site.position) <= (9. * 32f32).det_powi(2))
                .count(),
            4
        );
    }
}
fn enemy_position(s: &Snapshot) -> (f32, f32) {
    let e = s.entities.iter().find(|e| e.id == 9000).unwrap();
    (e.x, e.y)
}
#[test]
fn dead_sentry_is_replaced_without_stealing_other_bases_posts() {
    let (start, mut s, sites) = two_base_scene(12);
    let mut defense = tactics::DefenseAssignments::default();
    let f = frame(&start, &s, &sites);
    let owned = owned_sites(&f);
    let ids = defense.prepare_sentries(&f, &owned);
    let dead = *ids.iter().next().unwrap();
    s.entities.retain(|e| e.id != dead);
    s.tick += 9;
    let f = frame(&start, &s, &sites);
    let replacement = defense.prepare_sentries(&f, &owned);
    assert_eq!(replacement.len(), 8);
    assert!(!replacement.contains(&dead));
    assert_eq!(ids.intersection(&replacement).count(), 7);
}
#[test]
fn scarce_infantry_spread_across_bases_instead_of_emptying_one() {
    let (start, s, sites) = two_base_scene(3);
    let f = frame(&start, &s, &sites);
    let owned = owned_sites(&f);
    let mut defense = tactics::DefenseAssignments::default();
    assert_eq!(defense.prepare_sentries(&f, &owned).len(), 3);
    let staffed = sites
        .iter()
        .filter(|site| owned.contains(&site.id))
        .map(|site| {
            defense
                .assigned_sentry_posts()
                .values()
                .filter(|p| construction::dist2(**p, site.position) <= (9. * 32f32).det_powi(2))
                .count()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(staffed, BTreeSet::from([1, 2]));
}

#[test]
fn newly_assigned_attacking_guards_hold_once_then_keep_shooting_without_reorders() {
    let (start, mut s, sites) = two_base_scene(8);
    let mut defense = tactics::DefenseAssignments::default();
    let f = frame(&start, &s, &sites);
    let owned = owned_sites(&f);
    let ids = defense.prepare_sentries(&f, &owned);
    let posts = defense.assigned_sentry_posts().clone();
    for e in s.entities.iter_mut().filter(|e| ids.contains(&e.id)) {
        let p = posts[&e.id];
        e.x = p.0;
        e.y = p.1;
        e.state = "attack".into();
    }
    let f = frame(&start, &s, &sites);
    let mut actions = AiActions::for_frame(&f);
    defense.step(&f, &mut actions, &mut DefensePlanner::default(), &owned);
    let held = actions
        .into_requests()
        .into_iter()
        .filter_map(|r| match r {
            AiActionRequest::HoldPosition { units, .. } => Some(units),
            _ => None,
        })
        .flatten()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        held, ids,
        "new guards must cancel the previous chase order even at the post"
    );
    s.tick += 9;
    let f = frame(&start, &s, &sites);
    let mut actions = AiActions::for_frame(&f);
    defense.step(&f, &mut actions, &mut DefensePlanner::default(), &owned);
    assert!(
        actions.into_requests().is_empty(),
        "existing guards shooting in place must not restart weapon setup"
    );
}

#[test]
fn unreachable_infantry_do_not_count_as_observation_guards() {
    let (mut start, mut s, sites) = fixture();
    start.map.terrain.fill(0);
    start.map.unwalkable_tiles.clear();
    start.map.no_vehicle_tiles.clear();
    start.map.no_entrenchment_tiles.clear();
    start.map.doodads.clear();
    let wall = start.map.width / 2;
    for y in 0..start.map.height {
        start
            .map
            .unwalkable_tiles
            .push(rts_sim::protocol::MapTile { x: wall, y });
    }
    let home = sites
        .iter()
        .find(|b| b.id == AiBaseSiteId::Start(1))
        .unwrap()
        .position;
    let far = if home.0 < wall as f32 * 32. {
        (wall + 10) as f32 * 32. + 16.
    } else {
        (wall - 10) as f32 * 32. + 16.
    };
    for e in s
        .entities
        .iter_mut()
        .filter(|e| e.owner == 1 && e.kind == "rifleman")
    {
        e.x = far;
        e.y = home.1;
        e.hp = 100;
    }
    let f = frame(&start, &s, &sites);
    let mut defense = tactics::DefenseAssignments::default();
    assert!(defense.prepare_sentries(&f, &owned_sites(&f)).is_empty());
    assert!(defense.assigned_sentry_posts().is_empty());
}

#[test]
fn formerly_assigned_guards_are_replaced_if_they_can_no_longer_reach_their_posts() {
    let (mut start, mut s, sites) = fixture();
    start.map.terrain.fill(0);
    start.map.unwalkable_tiles.clear();
    start.map.no_vehicle_tiles.clear();
    start.map.no_entrenchment_tiles.clear();
    start.map.doodads.clear();
    let wall = start.map.width / 2;
    for y in 0..start.map.height {
        start
            .map
            .unwalkable_tiles
            .push(rts_sim::protocol::MapTile { x: wall, y });
    }
    add_observation_garrisons(&start, &mut s, &sites);
    let f = frame(&start, &s, &sites);
    let owned = owned_sites(&f);
    let mut defense = tactics::DefenseAssignments::default();
    let previous = defense.prepare_sentries(&f, &owned);
    assert_eq!(previous.len(), 4);
    let home = sites
        .iter()
        .find(|b| b.id == AiBaseSiteId::Start(1))
        .unwrap()
        .position;
    let far = if home.0 < wall as f32 * 32. {
        (wall + 10) as f32 * 32. + 16.
    } else {
        (wall - 10) as f32 * 32. + 16.
    };
    for e in s.entities.iter_mut().filter(|e| previous.contains(&e.id)) {
        e.x = far;
        e.y = home.1;
    }
    s.tick += 9;
    let f = frame(&start, &s, &sites);
    let replacements = defense.prepare_sentries(&f, &owned);
    assert_eq!(replacements.len(), 4);
    assert!(previous.is_disjoint(&replacements));
}

#[test]
fn completing_a_base_shares_scarce_surviving_guards_instead_of_leaving_it_empty() {
    let (start, mut s, sites) = two_base_scene(4);
    let second = s.entities.iter().find(|e| e.id == 800).unwrap().clone();
    s.entities.retain(|e| e.id != 800);
    let mut defense = tactics::DefenseAssignments::default();
    let f = frame(&start, &s, &sites);
    assert_eq!(defense.prepare_sentries(&f, &owned_sites(&f)).len(), 4);
    for e in s.entities.iter_mut() {
        if let Some(post) = defense.assigned_sentry_posts().get(&e.id) {
            e.x = post.0;
            e.y = post.1;
        }
    }
    let original = defense.assigned_sentry_posts().clone();
    let f = frame(&start, &s, &sites);
    let mut planner = DefensePlanner::default();
    defense.step(
        &f,
        &mut AiActions::for_frame(&f),
        &mut planner,
        &owned_sites(&f),
    );
    s.entities.push(second);
    s.tick += 9;
    let f = frame(&start, &s, &sites);
    let owned = owned_sites(&f);
    assert_eq!(defense.prepare_sentries(&f, &owned).len(), 4);
    for site in sites.iter().filter(|site| owned.contains(&site.id)) {
        assert_eq!(
            defense
                .assigned_sentry_posts()
                .values()
                .filter(|p| construction::dist2(**p, site.position) <= (9. * 32f32).det_powi(2))
                .count(),
            2
        );
    }
    let moved = defense
        .assigned_sentry_posts()
        .iter()
        .filter(|(id, p)| original.get(id) != Some(*p))
        .map(|(id, _)| *id)
        .collect::<BTreeSet<_>>();
    assert_eq!(moved.len(), 2);
    for e in s.entities.iter_mut() {
        if let Some(post) = defense.assigned_sentry_posts().get(&e.id) {
            e.x = post.0;
            e.y = post.1;
            e.state = "attack".into();
        }
    }
    let f = frame(&start, &s, &sites);
    let mut actions = AiActions::for_frame(&f);
    defense.step(&f, &mut actions, &mut planner, &owned);
    let held = actions
        .into_requests()
        .into_iter()
        .filter_map(|r| match r {
            AiActionRequest::HoldPosition { units, .. } => Some(units),
            _ => None,
        })
        .flatten()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        held, moved,
        "changed posts must take ownership once; unchanged guards keep shooting"
    );
}
