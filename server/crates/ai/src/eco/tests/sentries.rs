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
    for i in 0..count {
        let mut e = template.clone();
        e.id = 1000 + i as u32;
        e.x = if i % 2 == 0 {
            home.position.0
        } else {
            second.position.0
        };
        e.y = if i % 2 == 0 {
            home.position.1
        } else {
            second.position.1
        };
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
