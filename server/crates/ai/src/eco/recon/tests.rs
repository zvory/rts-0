use super::*;
use crate::eco::tests::{fixture, frame};
use crate::sdk::actions::AiActionRequest;
use rts_sim::protocol::{AbilityCooldownView, Snapshot, StartPayload};
fn scene(cars: usize) -> (StartPayload, Snapshot, Vec<crate::sdk::AiBaseSite>) {
    let (start, mut s, sites) = fixture();
    let template = s
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "rifleman")
        .unwrap()
        .clone();
    s.entities
        .retain(|e| e.owner == 1 && e.kind == "resource_depot");
    s.upgrades.push("scout_plane_unlock".into());
    for i in 0..cars {
        let mut e = template.clone();
        e.id = 1000 + i as u32;
        e.kind = "command_car".into();
        e.abilities = vec![AbilityCooldownView {
            ability: "scoutPlane".into(),
            cooldown_left: 0,
            remaining_uses: None,
            available_tick: None,
            lockout_until_tick: None,
            charge_recharge_left: None,
            active_object_id: None,
            autocast_enabled: None,
            expires_in: None,
        }];
        s.entities.push(e);
    }
    (start, s, sites)
}
fn casts(
    recon: &mut Recon,
    start: &StartPayload,
    s: &Snapshot,
    sites: &[crate::sdk::AiBaseSite],
    incoming: Option<(f32, f32)>,
) -> Vec<(u32, (f32, f32))> {
    let f = frame(start, s, sites);
    let mut a = AiActions::for_frame(&f);
    recon.step(&f, &mut a, incoming);
    a.into_requests()
        .into_iter()
        .filter_map(|r| match r {
            AiActionRequest::UseAbility {
                ability: AbilityKind::ScoutPlane,
                units,
                x: Some(x),
                y: Some(y),
                ..
            } => Some((units[0], (x, y))),
            _ => None,
        })
        .collect()
}
#[test]
fn one_car_stays_reserved_but_launches_at_an_incoming_attack() {
    let (start, mut s, sites) = scene(1);
    let mut recon = Recon::default();
    assert!(casts(&mut recon, &start, &s, &sites, None).is_empty());
    let p = (s.entities[0].x + 128., s.entities[0].y + 128.);
    assert_eq!(
        casts(&mut recon, &start, &s, &sites, Some(p)),
        vec![(1000, p)]
    );
    s.tick += 9;
    assert!(casts(&mut recon, &start, &s, &sites, Some(p)).is_empty());
}
#[test]
fn spare_cars_scout_distinct_sites_without_spending_the_defensive_reserve() {
    let (start, mut s, sites) = scene(3);
    let mut recon = Recon::default();
    let first = casts(&mut recon, &start, &s, &sites, None);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].0, 1001);
    s.tick += 9;
    let second = casts(&mut recon, &start, &s, &sites, None);
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].0, 1002);
    assert_ne!(first[0].1, second[0].1);
    s.tick += 9;
    assert!(casts(&mut recon, &start, &s, &sites, None).is_empty());
    let p = (s.entities[0].x + 128., s.entities[0].y + 128.);
    assert_eq!(
        casts(&mut recon, &start, &s, &sites, Some(p)),
        vec![(1000, p)]
    );
}
#[test]
fn research_and_cooldown_gate_launches_and_existing_sorties_prevent_duplicates() {
    let (start, mut s, sites) = scene(2);
    let mut recon = Recon::default();
    let p = (s.entities[0].x + 128., s.entities[0].y + 128.);
    s.upgrades.clear();
    assert!(casts(&mut recon, &start, &s, &sites, Some(p)).is_empty());
    s.upgrades.push("scout_plane_unlock".into());
    assert_eq!(casts(&mut recon, &start, &s, &sites, Some(p)).len(), 1);
    s.tick += 9;
    assert!(casts(&mut recon, &start, &s, &sites, Some(p)).is_empty());
    s.tick += u32::from(rts_rules::balance::SCOUT_PLANE_LIFETIME_TICKS);
    assert_eq!(casts(&mut recon, &start, &s, &sites, Some(p)).len(), 1);
}
