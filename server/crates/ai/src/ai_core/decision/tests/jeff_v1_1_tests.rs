use super::jeff_v1_1::{barracks_batches, gun_cap, rifle_line};
use super::*;
use crate::ai_core::facts::ProductionBuildingFact;
use crate::ai_core::profiles::{JEFFS_AI, JEFFS_AI_V1_1, JEFFS_AI_V1_1_ID};

fn barracks(id: u32) -> ProductionBuildingFact {
    ProductionBuildingFact {
        id,
        kind: EntityKind::Barracks,
        queue_len: 0,
    }
}

fn economy() -> AiEconomy {
    AiEconomy {
        steel: 500,
        oil: 100,
        supply_used: 20,
        supply_cap: 80,
    }
}

#[test]
fn v1_1_trains_guns_from_its_oldest_barracks_and_no_guns_from_the_others() {
    let buildings = [barracks(9), barracks(4), barracks(12)];
    let priorities = [
        EntityKind::Tank,
        EntityKind::MachineGunner,
        EntityKind::Rifleman,
    ];
    let caps = [(EntityKind::MachineGunner, 10)];
    let short = [(EntityKind::MachineGunner, 3)];
    let batches = barracks_batches(
        JEFFS_AI_V1_1_ID,
        EntityKind::Barracks,
        &buildings,
        &priorities,
        (&short, &caps),
    );
    assert_eq!(
        batches,
        vec![
            (vec![barracks(4)], vec![EntityKind::MachineGunner]),
            (
                vec![barracks(9), barracks(12)],
                vec![EntityKind::Tank, EntityKind::Rifleman]
            ),
        ]
    );
    // With every gun post filled the oldest Barracks trains like the others.
    let full = [(EntityKind::MachineGunner, 10)];
    let batches = barracks_batches(
        JEFFS_AI_V1_1_ID,
        EntityKind::Barracks,
        &buildings,
        &priorities,
        (&full, &caps),
    );
    assert_eq!(batches[0].1, vec![EntityKind::Tank, EntityKind::Rifleman]);
    // V1.0, and V1.1 with one Barracks, train from every Barracks as before.
    for (profile_id, buildings) in [
        (JEFFS_AI_ID, &buildings[..]),
        (JEFFS_AI_V1_1_ID, &buildings[..1]),
    ] {
        let batches = barracks_batches(
            profile_id,
            EntityKind::Barracks,
            buildings,
            &priorities,
            (&short, &caps),
        );
        assert_eq!(batches, vec![(buildings.to_vec(), priorities.to_vec())]);
    }
}

#[test]
fn v1_1_builds_guns_for_the_home_pocket_each_bases_line_and_a_home_reserve() {
    // HQ and natural up, five guns out: six home, four at the natural and four held home.
    let mut owned = vec![
        building(1, EntityKind::ResourceDepot, Some(0)),
        building(2, EntityKind::ResourceDepot, Some(0)),
    ];
    owned.extend((10..15).map(|id| combat(id, EntityKind::MachineGunner)));
    let obs = observation(economy(), owned);
    let facts = AiFacts::from_observation(&obs);
    let mut caps = vec![(EntityKind::MachineGunner, 6)];
    gun_cap(&JEFFS_AI_V1_1, &facts, None, &[], &mut caps);
    assert_eq!(caps, vec![(EntityKind::MachineGunner, 14)]);
    let mut caps = vec![(EntityKind::MachineGunner, 6)];
    gun_cap(&JEFFS_AI, &facts, None, &[], &mut caps);
    assert_eq!(caps, vec![(EntityKind::MachineGunner, 6)]);
}

#[test]
fn v1_1_keeps_two_riflemen_for_every_tank_and_four_at_home() {
    let mut owned: Vec<_> = (10..13).map(|id| combat(id, EntityKind::Tank)).collect();
    owned.extend((20..25).map(|id| combat(id, EntityKind::Rifleman)));
    let obs = observation(economy(), owned);
    let facts = AiFacts::from_observation(&obs);
    let mut priorities = vec![EntityKind::Tank];
    assert_eq!(
        rifle_line(JEFFS_AI_V1_1_ID, &facts, &mut priorities),
        Some(10)
    );
    assert_eq!(priorities, vec![EntityKind::Tank, EntityKind::Rifleman]);
    let mut priorities = vec![EntityKind::Tank];
    assert_eq!(rifle_line(JEFFS_AI_ID, &facts, &mut priorities), None);
    assert_eq!(priorities, vec![EntityKind::Tank]);
}
