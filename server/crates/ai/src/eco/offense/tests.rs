use super::*;
use crate::eco::tests::{fixture, frame};
use crate::sdk::actions::AiActionRequest;
use rts_sim::protocol::{EntityView, Snapshot, StartPayload};
fn scene() -> (StartPayload, Snapshot, Vec<crate::sdk::AiBaseSite>) {
    let (mut start, mut s, sites) = fixture();
    start.map.terrain.fill(0);
    start.map.unwalkable_tiles.clear();
    start.map.no_vehicle_tiles.clear();
    start.map.doodads.clear();
    start.map.resources.clear();
    let sample = s
        .entities
        .iter()
        .find(|e| e.kind == "rifleman" && e.owner == 1)
        .unwrap()
        .clone();
    s.entities
        .retain(|e| e.kind == "resource_depot" && e.owner == 1);
    for id in 1000..1015 {
        let mut e = sample.clone();
        e.id = id;
        e.kind = if id < 1010 { "machine_gunner" } else { "tank" }.into();
        e.hp = 100;
        e.max_hp = 100;
        e.x = 1000. + (id - 1000) as f32 * 64.;
        e.y = 600.;
        e.state = "idle".into();
        s.entities.push(e);
    }
    s.supply_cap = 300;
    s.supply_used = 300;
    s.visible_tiles = vec![1; (start.map.width * start.map.height) as usize];
    (start, s, sites)
}
fn step(
    off: &mut EcoOffense,
    start: &StartPayload,
    s: &Snapshot,
    sites: &[crate::sdk::AiBaseSite],
    tactical: TacticalOwnership,
) -> Vec<AiActionRequest> {
    let f = frame(start, s, sites);
    let pool = off.controlled(&f, &BTreeSet::new());
    let mut a = AiActions::for_frame(&f);
    off.step(&f, &mut a, &pool, &tactical);
    a.into_requests()
}
#[test]
fn capacity_latches_and_uses_half_map_geographic_rank() {
    for total in [10, 12] {
        let (start, mut s, mut sites) = scene();
        while sites.len() > total {
            let i = sites
                .iter()
                .rposition(|site| matches!(site.id, AiBaseSiteId::ResourceCluster(_)))
                .unwrap();
            sites.remove(i);
        }
        assert_eq!(sites.len(), total);
        let mut off = EcoOffense::default();
        s.supply_used = 299;
        off.observe(&frame(&start, &s, &sites));
        assert!(!off.activated);
        s.supply_used = 300;
        let f = frame(&start, &s, &sites);
        off.observe(&f);
        assert_eq!(
            off.targets[0],
            WorldQueries::new(&f).base(2, total / 2).unwrap().site.id
        );
        assert_eq!(*off.targets.last().unwrap(), AiBaseSiteId::Start(2));
        s.supply_used = 100;
        off.observe(&frame(&start, &s, &sites));
        assert!(off.activated);
    }
}
#[test]
fn staging_waits_for_a_cohort_and_new_production_does_not_postpone_it() {
    let (start, mut s, sites) = scene();
    let mut off = EcoOffense::default();
    off.observe(&frame(&start, &s, &sites));
    let orders = step(&mut off, &start, &s, &sites, TacticalOwnership::default());
    assert!(!orders.is_empty());
    assert!(off.launched.is_empty());
    assert_eq!(off.cohort.len(), 15);
    assert_eq!(off.slots.len(), 15);
    let origin = off.slots[&1010];
    let next = off.slots[&1011];
    assert!((distance(origin, next) - 64.).abs() < 0.1);
    let direction = ((next.0 - origin.0) / 64., (next.1 - origin.1) / 64.);
    let depths = off
        .slots
        .values()
        .map(|p| {
            (((p.0 - origin.0) * direction.0 + (p.1 - origin.1) * direction.1) / 32.).round() as i32
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(depths, BTreeSet::from([0, 2, 4]));
    let positions = off.slots.values().copied().collect::<Vec<_>>();
    for (i, a) in positions.iter().enumerate() {
        assert!(positions[i + 1..].iter().all(|b| distance(*a, *b) > 32.));
    }
    let mut new = s.entities.iter().find(|e| e.id == 1000).unwrap().clone();
    new.id = 2000;
    s.entities.push(new);
    for e in &mut s.entities {
        if let Some(p) = off.slots.get(&e.id) {
            e.x = p.0;
            e.y = p.1;
        }
    }
    s.tick += 9;
    let orders = step(&mut off, &start, &s, &sites, TacticalOwnership::default());
    let wave = orders
        .iter()
        .find_map(|r| match r {
            AiActionRequest::AttackMove { units, .. } if units.len() > 1 => Some(units),
            _ => None,
        })
        .unwrap();
    assert_eq!(wave.len(), 15);
    assert!(!wave.contains(&2000));
    assert!(off.cohort.is_empty());
    // Home defense borrows a launched unit, then releases it while it is still moving.
    s.tick += 9;
    s.entities.iter_mut().find(|e| e.id == 1000).unwrap().state = "move".into();
    let orders = step(
        &mut off,
        &start,
        &s,
        &sites,
        TacticalOwnership {
            active: BTreeSet::new(),
            released: BTreeSet::from([1000]),
        },
    );
    assert!(orders
        .iter()
        .any(|r| matches!(r,AiActionRequest::AttackMove{units,..}if units==&[1000])));
}
#[test]
fn base_clear_requires_observation_and_removes_enemy_buildings_before_restaging() {
    let (start, mut s, sites) = scene();
    let mut off = EcoOffense::default();
    off.observe(&frame(&start, &s, &sites));
    let target = off.target(&frame(&start, &s, &sites)).unwrap();
    off.launched.insert(1000);
    let e = s.entities.iter_mut().find(|e| e.id == 1000).unwrap();
    e.x = target.0;
    e.y = target.1;
    s.visible_tiles.fill(0);
    assert!(!off.clear(&frame(&start, &s, &sites), target));
    s.visible_tiles.fill(1);
    let mut depot: EntityView = s
        .entities
        .iter()
        .find(|e| e.kind == "resource_depot")
        .unwrap()
        .clone();
    depot.id = 9999;
    depot.owner = 2;
    depot.x = target.0;
    depot.y = target.1;
    s.entities.push(depot);
    assert!(!off.clear(&frame(&start, &s, &sites), target));
    s.entities.retain(|e| e.id != 9999);
    assert!(off.clear(&frame(&start, &s, &sites), target));
    step(&mut off, &start, &s, &sites, TacticalOwnership::default());
    assert_eq!(off.index, 1);
    assert!(off.launched.is_empty());
}
#[test]
fn outbound_troops_yield_to_micro_without_receiving_reserve_or_post_orders() {
    let (start, s, sites) = scene();
    let f = frame(&start, &s, &sites);
    let mut off = EcoOffense::default();
    off.observe(&f);
    let mut defense = crate::eco::tactics::DefenseAssignments::default();
    let owned = crate::eco::owned_sites(&f);
    let guards = defense.prepare_sentries(&f, &owned);
    let pool = off.controlled(&f, &guards);
    let mut planner = crate::sdk::DefensePlanner::new();
    let mut a = AiActions::for_frame(&f);
    let feedback = defense.step_with_outbound(&f, &mut a, &mut planner, &owned, &pool);
    assert_eq!(feedback.active, guards);
    assert!(a.into_requests().iter().all(|r| match r {
        AiActionRequest::Move { units, .. } | AiActionRequest::HoldPosition { units, .. } =>
            units.iter().all(|id| guards.contains(id)),
        _ => false,
    }));
    let mut a = AiActions::for_frame(&f);
    off.step(&f, &mut a, &pool, &feedback);
    assert!(!a.is_empty());
}

#[test]
fn new_tanks_get_staging_orders_after_infantry_fills_the_center() {
    let (start, mut s, sites) = scene();
    let mut off = EcoOffense::default();
    off.observe(&frame(&start, &s, &sites));
    step(&mut off, &start, &s, &sites, TacticalOwnership::default());
    let template = s.entities.iter().find(|e| e.id == 1010).unwrap().clone();
    for id in 2000..2006 {
        let mut e = template.clone();
        e.id = id;
        s.entities.push(e);
    }
    s.tick += 90;
    let orders = step(&mut off, &start, &s, &sites, TacticalOwnership::default());
    for id in 2000..2006 {
        assert!(
            off.slots.contains_key(&id),
            "new Tank {id} has no staging slot"
        );
        assert!(
            orders
                .iter()
                .any(|r| matches!(r, AiActionRequest::Move {units,..} if units.contains(&id))),
            "new Tank {id} has no movement order"
        );
    }
}

#[test]
fn armored_wave_launches_with_ninety_percent_of_cohort_tanks_without_waiting_for_infantry() {
    let (start, mut s, sites) = scene();
    let template = s.entities.iter().find(|e| e.id == 1010).unwrap().clone();
    for id in 2000..2005 {
        let mut e = template.clone();
        e.id = id;
        s.entities.push(e);
    }
    let mut off = EcoOffense::default();
    off.observe(&frame(&start, &s, &sites));
    step(&mut off, &start, &s, &sites, TacticalOwnership::default());
    let tanks = s
        .entities
        .iter()
        .filter(|e| e.kind == "tank")
        .map(|e| e.id)
        .collect::<Vec<_>>();
    assert_eq!(tanks.len(), 10);
    for id in tanks.iter().take(8) {
        let p = off.slots[id];
        let e = s.entities.iter_mut().find(|e| e.id == *id).unwrap();
        e.x = p.0;
        e.y = p.1;
    }
    s.tick += 9;
    step(&mut off, &start, &s, &sites, TacticalOwnership::default());
    assert!(off.launched.is_empty(), "80% of Tanks must keep assembling");
    let p = off.slots[&tanks[8]];
    let e = s.entities.iter_mut().find(|e| e.id == tanks[8]).unwrap();
    // Nearly parked armor counts for departure without requiring pixel-perfect alignment.
    e.x = p.0 + 48.;
    e.y = p.1;
    // Ongoing production must not add another Tank to the frozen wave's denominator.
    let mut late = template.clone();
    late.id = 3000;
    s.entities.push(late);
    s.tick += 9;
    let orders = step(&mut off, &start, &s, &sites, TacticalOwnership::default());
    assert_eq!(off.launched.len(), 9);
    assert!(!off.launched.contains(&3000));
    assert!(orders
        .iter()
        .any(|r| matches!(r,AiActionRequest::AttackMove{units,..}if units.len()==9)));
}
