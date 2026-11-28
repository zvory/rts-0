use super::*;
use crate::eco::tests::{fixture, frame};
use crate::sdk::actions::AiActionRequest;
use rts_sim::protocol::{AbilityCooldownView, Snapshot, StartPayload};
fn scene() -> (StartPayload, Snapshot, Vec<crate::sdk::AiBaseSite>) {
    let (mut start, mut s, sites) = fixture();
    start.map.terrain.fill(0);
    start.map.unwalkable_tiles.clear();
    start.map.no_vehicle_tiles.clear();
    start.map.doodads.clear();
    start.map.resources.clear();
    let template = s
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "rifleman")
        .unwrap()
        .clone();
    s.entities
        .retain(|e| e.owner == 1 && e.kind == "resource_depot");
    for (id, kind, p) in [
        (1000, "command_car", (1200., 600.)),
        (1001, "tank", (1100., 1600.)),
        (1002, "tank", (2300., 600.)),
        (1003, "tank", (2364., 600.)),
        (1004, "tank", (2300., 664.)),
    ] {
        let mut e = template.clone();
        e.id = id;
        e.kind = kind.into();
        e.x = p.0;
        e.y = p.1;
        e.hp = 100;
        e.max_hp = 100;
        e.abilities.clear();
        e.state = "idle".into();
        s.entities.push(e);
    }
    (start, s, sites)
}
#[test]
fn active_tanks_take_priority_over_a_larger_idle_reserve() {
    let (start, s, sites) = scene();
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    CommandSupport::default().step(&f, &mut a, &BTreeSet::from([1001]));
    let end = a
        .into_requests()
        .into_iter()
        .filter_map(|r| match r {
            AiActionRequest::Move { x, y, .. } => Some((x, y)),
            _ => None,
        })
        .last()
        .unwrap();
    assert!(dist2(end, (1100., 1600.)) < dist2(end, (2300., 600.)));
    assert!(dist2(end, (1100., 1600.)) <= (3. * 32.) * (3. * 32.));
}
#[test]
fn breakthrough_needs_a_nearby_engaged_group_enemy_and_ready_cooldown() {
    let (start, mut s, sites) = scene();
    let car = s.entities.iter_mut().find(|e| e.id == 1000).unwrap();
    car.x = 1100.;
    car.y = 1536.;
    car.abilities = vec![AbilityCooldownView {
        ability: "breakthrough".into(),
        cooldown_left: 0,
        remaining_uses: None,
        available_tick: None,
        lockout_until_tick: None,
        charge_recharge_left: None,
        active_object_id: None,
        autocast_enabled: None,
        expires_in: None,
    }];
    let mut enemy = s.entities.iter().find(|e| e.id == 1001).unwrap().clone();
    enemy.id = 9999;
    enemy.owner = 2;
    enemy.x = 1228.;
    s.entities.push(enemy);
    let cast = |s: &Snapshot, active: &BTreeSet<u32>| {
        let f = frame(&start, s, &sites);
        let mut a = AiActions::for_frame(&f);
        CommandSupport::default().step(&f, &mut a, active);
        a.into_requests().iter().any(|r| {
            matches!(
                r,
                AiActionRequest::UseAbility {
                    ability: AbilityKind::Breakthrough,
                    ..
                }
            )
        })
    };
    assert!(cast(&s, &BTreeSet::from([1001])));
    assert!(!cast(&s, &BTreeSet::new()));
    s.entities
        .iter_mut()
        .find(|e| e.id == 1000)
        .unwrap()
        .abilities[0]
        .cooldown_left = 30;
    assert!(!cast(&s, &BTreeSet::from([1001])));
}

#[test]
fn cars_rotate_without_overlapping_breakthrough() {
    let (start, mut s, sites) = scene();
    let mut car = s.entities.iter().find(|e| e.id == 1000).unwrap().clone();
    car.x = 1100.;
    car.y = 1536.;
    car.abilities = vec![AbilityCooldownView {
        ability: "breakthrough".into(),
        cooldown_left: 0,
        remaining_uses: None,
        available_tick: None,
        lockout_until_tick: None,
        charge_recharge_left: None,
        active_object_id: None,
        autocast_enabled: None,
        expires_in: None,
    }];
    s.entities.retain(|e| e.id != 1000);
    s.entities.push(car.clone());
    car.id = 2000;
    s.entities.push(car);
    let mut support = CommandSupport::default();
    let mut casts = |s: &Snapshot| {
        let f = frame(&start, s, &sites);
        let mut a = AiActions::for_frame(&f);
        support.step_mobilized(&f, &mut a, &BTreeSet::from([1001]), true);
        a.into_requests()
            .into_iter()
            .filter_map(|r| match r {
                AiActionRequest::UseAbility { units, .. } => Some(units),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(casts(&s), vec![vec![1000]]);
    s.tick += 9;
    assert!(casts(&s).is_empty());
    s.tick += u32::from(rts_rules::balance::BREAKTHROUGH_DURATION_TICKS);
    assert_eq!(casts(&s), vec![vec![2000]]);
    s.tick += u32::from(rts_rules::balance::BREAKTHROUGH_DURATION_TICKS);
    s.entities
        .iter_mut()
        .find(|e| e.id == 2000)
        .unwrap()
        .abilities[0]
        .expires_in = Some(9);
    assert!(casts(&s).is_empty());
}
