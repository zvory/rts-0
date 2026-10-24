use super::opening_rush::OpeningRush;
use super::*;
use crate::ai_core::profiles::{JEFFS_AI, JEFFS_AI_PRE_FAST_NATURAL};

/// Jeff at its early natural: the HQ, Barracks, Training Centre and Engineering Complex up, the four
/// opening guns home, no Factory yet, and the natural's site picked. Returns the site too.
fn early_natural(profile: &'static AiProfile) -> (AiObservation, AiDecisionMemory, (u32, u32)) {
    let ts = config::TILE_SIZE as f32;
    let mut units = vec![
        worker_at(1, AiEntityState::Idle, 9.0 * ts, 9.0 * ts),
        building_at(2, EntityKind::ResourceDepot, Some(0), 8.5 * ts, 8.5 * ts),
        building(3, EntityKind::Barracks, Some(0)),
        building(4, EntityKind::TrainingCentre, Some(0)),
        building(5, EntityKind::EngineeringComplex, Some(0)),
    ];
    units.extend((20..24).map(|id| combat_at(id, EntityKind::MachineGunner, 10.0 * ts, 10.0 * ts)));
    let mut obs = observation(
        AiEconomy {
            steel: 40,
            oil: 0,
            supply_used: 20,
            supply_cap: 80,
        },
        units,
    );
    obs.tick = 1800 * 3 / 2;
    obs.upgrades.push(UpgradeKind::Entrenchment);
    obs.resources
        .push(resource(300, EntityKind::Steel, 28.0 * ts, 28.0 * ts));
    obs.resources
        .push(resource(301, EntityKind::Oil, 30.0 * ts, 28.0 * ts));
    let mut memory = AiDecisionMemory::for_profile(profile);
    expansion_security::prepare(
        &obs,
        &AiFacts::from_observation(&obs),
        profile,
        &mut memory,
        None,
        &mut |_, _, _| true,
    );
    let site = memory.expansion_security.site.expect("natural site");
    (obs, memory, site)
}

/// The natural's builder (Engineer 1) is out at `site` with the Depot's order pending.
fn order_out(obs: &mut AiObservation, site: (u32, u32)) {
    let ts = config::TILE_SIZE as f32;
    let builder = obs.owned.iter_mut().find(|unit| unit.id == 1).unwrap();
    builder.state = AiEntityState::Move;
    builder.x = site.0 as f32 * ts;
    builder.y = site.1 as f32 * ts;
    obs.pending_builds = vec![crate::ai_core::observation::AiBuildIntent::to_site(
        1,
        EntityKind::ResourceDepot,
        site.0,
        site.1,
    )];
}

fn built(decision: &AiDecision, kind: EntityKind) -> Option<Vec<u32>> {
    decision.commands.iter().find_map(|command| match command {
        Command::Build {
            units, building, ..
        } if *building == kind => Some(units.clone()),
        _ => None,
    })
}

fn trains(decision: &AiDecision, kind: EntityKind) -> bool {
    decision.intents.contains(&AiIntent::Train { kind })
}

#[test]
fn with_the_natural_ordered_the_live_jeff_trains_a_second_engineer_who_builds_the_factory() {
    for profile in [&JEFFS_AI, &JEFFS_AI_PRE_FAST_NATURAL] {
        let live = profile.id == JEFFS_AI_ID;
        let (mut obs, mut memory, site) = early_natural(profile);
        // The bounded reserve is running.
        decide(&obs, profile, &mut memory);
        order_out(&mut obs, site);
        // The Depot's 450/100 is set aside, with a Factory (125/125) and an Engineer (50) to spare.
        obs.economy.steel = 450 + 200;
        obs.economy.oil = 100 + 140;
        let decision = decide(&obs, profile, &mut memory);
        assert_eq!(
            trains(&decision, EntityKind::Worker),
            live,
            "{}",
            profile.id
        );
        // No Engineer is home yet, so nothing is built.
        assert_eq!(
            built(&decision, EntityKind::Factory),
            None,
            "{}",
            profile.id
        );

        // The new Engineer is home: the live Jeff builds the Factory with it while the builder
        // walks; the freeze holds the Factory back until the foundation is down.
        obs.owned
            .push(worker_at(7, AiEntityState::Idle, 9.0 * 32.0, 10.0 * 32.0));
        let decision = decide(&obs, profile, &mut memory);
        assert_eq!(
            built(&decision, EntityKind::Factory),
            live.then(|| vec![7]),
            "{}",
            profile.id
        );
    }
}

#[test]
fn a_pending_natural_order_holds_back_only_its_oil_for_the_live_jeff() {
    for profile in [&JEFFS_AI, &JEFFS_AI_PRE_FAST_NATURAL] {
        let live = profile.id == JEFFS_AI_ID;
        let (mut obs, mut memory, site) = early_natural(profile);
        // Six Riflemen home, two of them the natural's party: no recruits.
        let ts = config::TILE_SIZE as f32;
        obs.owned
            .extend((10..16).map(|id| combat_at(id, EntityKind::Rifleman, 14.0 * ts, 14.0 * ts)));
        decide(&obs, profile, &mut memory);
        order_out(&mut obs, site);
        // The second Engineer (50) and a fifth gun (75/10) fit beside the order's 450/100, but not
        // beside it held twice.
        obs.economy.steel = 450 + 50 + 80;
        obs.economy.oil = 100 + 20;
        let decision = decide(&obs, profile, &mut memory);
        assert_eq!(
            trains(&decision, EntityKind::MachineGunner),
            live,
            "{}",
            profile.id
        );
    }
}

#[test]
fn the_live_jeffs_home_builds_take_the_engineer_nearest_the_hq() {
    let ts = config::TILE_SIZE as f32;
    let (mut obs, mut memory, site) = early_natural(&JEFFS_AI);
    // The natural stands with its builder idle beside it; Engineer 7 is home.
    obs.owned.push(building_at(
        6,
        EntityKind::ResourceDepot,
        Some(0),
        site.0 as f32 * ts,
        site.1 as f32 * ts,
    ));
    let builder = obs.owned.iter_mut().find(|unit| unit.id == 1).unwrap();
    builder.x = site.0 as f32 * ts;
    builder.y = site.1 as f32 * ts;
    obs.owned
        .push(worker_at(7, AiEntityState::Idle, 9.0 * ts, 10.0 * ts));
    obs.economy.steel = 400;
    obs.economy.oil = 300;
    let decision = decide(&obs, &JEFFS_AI, &mut memory);
    assert_eq!(built(&decision, EntityKind::Factory), Some(vec![7]));
}

#[test]
fn the_live_jeff_trains_its_opening_guns_before_the_party_and_no_riflemen_beyond_it() {
    for profile in [&JEFFS_AI, &JEFFS_AI_PRE_FAST_NATURAL] {
        let live = profile.id == JEFFS_AI_ID;
        let (mut obs, mut memory, _) = early_natural(profile);
        // Two of the four opening guns are lost and no Rifleman is home: the starters fell on the
        // rush. There is Steel for one more unit and no natural reserve yet.
        obs.owned.retain(|unit| !matches!(unit.id, 22 | 23));
        obs.economy.steel = 110;
        obs.economy.oil = 30;
        let decision = decide(&obs, profile, &mut memory);
        // The live Jeff trains the gun; the freeze tops its Riflemen up to six first.
        assert_eq!(
            trains(&decision, EntityKind::MachineGunner),
            live,
            "{}",
            profile.id
        );
        assert_eq!(
            trains(&decision, EntityKind::Rifleman),
            !live,
            "{}",
            profile.id
        );
    }

    // The rush took starters 30-33 and lost them. Four guns are home and the natural is due: Steel
    // above its reserve goes to the guard party first.
    let ts = config::TILE_SIZE as f32;
    let (mut obs, mut memory, _) = early_natural(&JEFFS_AI);
    memory.opening_rush = OpeningRush::ended_with_starters(&[30, 31, 32, 33]);
    obs.economy.steel = 450 + 120;
    obs.economy.oil = 100 + 30;
    assert!(trains(
        &decide(&obs, &JEFFS_AI, &mut memory),
        EntityKind::Rifleman
    ));
    // With the party's two Riflemen home, the freeze still tops Riflemen up to six; the live Jeff
    // trains its fifth gun instead.
    obs.owned
        .extend((10..12).map(|id| combat_at(id, EntityKind::Rifleman, 14.0 * ts, 14.0 * ts)));
    let decision = decide(&obs, &JEFFS_AI, &mut memory);
    assert_eq!(memory.expansion_security.riflemen.len(), 2);
    assert!(!trains(&decision, EntityKind::Rifleman));
    assert!(trains(&decision, EntityKind::MachineGunner));
    let (_, mut frozen, _) = early_natural(&JEFFS_AI_PRE_FAST_NATURAL);
    frozen.opening_rush = OpeningRush::ended_with_starters(&[30, 31, 32, 33]);
    let decision = decide(&obs, &JEFFS_AI_PRE_FAST_NATURAL, &mut frozen);
    assert!(trains(&decision, EntityKind::Rifleman));
    assert!(!trains(&decision, EntityKind::MachineGunner));
}
