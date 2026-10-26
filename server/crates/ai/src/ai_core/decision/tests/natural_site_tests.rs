use super::expansion_security_tests::security_observation;
use super::*;
use crate::ai_core::profiles::{
    JEFFS_AI, JEFFS_AI_PRE_GUARD_HANDOFF, JEFFS_AI_PRE_HELD_SITE_ID, JEFFS_AI_PRE_TANK_MICRO,
};

/// Secure the natural in the standard fixture and return it with the site.
fn secured_natural() -> (AiObservation, AiDecisionMemory, (u32, u32)) {
    let obs = security_observation();
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    expansion_security::prepare(
        &obs,
        &AiFacts::from_observation(&obs),
        &JEFFS_AI,
        &mut memory,
        None,
        &mut |_, _, _| true,
    );
    let site = memory.expansion_security.site.expect("natural site");
    (obs, memory, site)
}

#[test]
fn the_natural_is_only_ordered_with_its_full_cost_banked_and_a_quiet_route() {
    let (mut obs, memory, site) = secured_natural();
    let ready = |obs: &AiObservation| {
        expansion_security::natural_attempt_ready(obs, None, &memory, true, true)
    };
    obs.economy.steel = 450;
    obs.economy.oil = 99;
    assert!(!ready(&obs), "one Oil short");
    obs.economy.oil = 100;
    assert!(ready(&obs));
    assert!(
        !expansion_security::natural_attempt_ready(&obs, None, &memory, false, true),
        "unsecured"
    );

    // An enemy next to the builder keeps it home.
    let builder = obs
        .owned
        .iter()
        .find(|unit| unit.kind == EntityKind::Worker)
        .unwrap();
    let mut raider = combat_at(900, EntityKind::Rifleman, builder.x + 64.0, builder.y);
    raider.owner = 2;
    obs.visible_enemies.push(raider);
    assert!(!ready(&obs));

    // So does an order already on its way to the site.
    obs.visible_enemies.clear();
    obs.pending_builds = vec![crate::ai_core::observation::AiBuildIntent::to_site(
        1,
        EntityKind::ResourceDepot,
        site.0,
        site.1,
    )];
    assert!(!ready(&obs));
}

#[test]
fn a_dropped_natural_order_waits_out_the_cooldown_before_the_next_try() {
    let (mut obs, mut memory, site) = secured_natural();
    obs.economy.steel = 1000;
    obs.economy.oil = 1000;
    memory.expansion_security.note_build_attempt(obs.tick, 1);
    obs.tick += config::TICK_HZ * 4;
    expansion_security::prepare(
        &obs,
        &AiFacts::from_observation(&obs),
        &JEFFS_AI,
        &mut memory,
        None,
        &mut |_, _, _| true,
    );
    assert_eq!(
        memory.expansion_security.site,
        Some(site),
        "kept after one drop"
    );
    assert!(!expansion_security::natural_attempt_ready(
        &obs, None, &memory, true, true
    ));
    obs.tick += config::TICK_HZ * 20;
    assert!(expansion_security::natural_attempt_ready(
        &obs, None, &memory, true, true
    ));
}

#[test]
fn a_pending_natural_order_keeps_its_oil_from_the_next_tank() {
    let (mut obs, mut memory, site) = secured_natural();
    // Start the natural's bounded reserve, then let it expire, so only the pending order can be
    // what holds the Depot's cost back.
    decide(&obs, &JEFFS_AI, &mut memory);
    obs.tick += config::TICK_HZ * 61;
    let factory = obs
        .owned
        .iter_mut()
        .find(|unit| unit.kind == EntityKind::Factory)
        .unwrap();
    factory.production_kind = None;
    factory.production_queue_len = Some(0);
    // Enough for a Tank (425/175), but not once the Depot's 450/100 is set aside.
    obs.economy.steel = 700;
    obs.economy.oil = 250;
    let tank = AiIntent::Train {
        kind: EntityKind::Tank,
    };
    let mut control = memory.clone();
    assert!(
        decide(&obs, &JEFFS_AI, &mut control)
            .intents
            .contains(&tank),
        "with nothing pending the Tank is trained"
    );
    obs.pending_builds = vec![crate::ai_core::observation::AiBuildIntent::to_site(
        1,
        EntityKind::ResourceDepot,
        site.0,
        site.1,
    )];
    assert!(!decide(&obs, &JEFFS_AI, &mut memory).intents.contains(&tank));
}

/// The site of the natural, with Tanks at `(id, x tile, y tile)` and an enemy unit of `kind` on it.
fn natural_held_by(
    kind: EntityKind,
    tanks: &[(u32, f32, f32)],
) -> (AiObservation, AiDecisionMemory) {
    let ts = config::TILE_SIZE as f32;
    let mut obs = security_observation();
    for (id, x, y) in tanks {
        obs.owned
            .push(combat_at(*id, EntityKind::Tank, x * ts, y * ts));
    }
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    expansion_security::prepare(
        &obs,
        &AiFacts::from_observation(&obs),
        &JEFFS_AI,
        &mut memory,
        None,
        &mut |_, _, _| true,
    );
    let site = memory.expansion_security.site.unwrap();
    let center = building_center(site, EntityKind::ResourceDepot, obs.map.tile_size).unwrap();
    let mut enemy = combat_at(900, kind, center.0 + 2.0 * ts, center.1);
    enemy.owner = 2;
    obs.visible_enemies.push(enemy);
    (obs, memory)
}

#[test]
fn enemy_infantry_on_the_natural_is_cleared_by_the_two_nearest_tanks() {
    // The Tanks stand at home, too far to count toward the site's defence, so the Rifleman holds it.
    let (obs, mut memory) = natural_held_by(
        EntityKind::Rifleman,
        &[(60, 5.0, 5.0), (61, 4.0, 4.0), (62, 2.0, 2.0)],
    );
    let mut obs = obs;
    for unit in obs
        .owned
        .iter_mut()
        .filter(|unit| unit.kind == EntityKind::Rifleman)
    {
        unit.x = 4.0 * config::TILE_SIZE as f32;
    }
    let decision = decide(&obs, &JEFFS_AI, &mut memory);
    assert_eq!(memory.expansion_security.contested_tick, Some(obs.tick));
    assert_eq!(memory.expansion_security.clearers, vec![60, 61]);
    for tank in [60, 61] {
        assert!(
            decision.commands.iter().any(|command| matches!(
                command,
                Command::AttackMove { units, .. } if units == &[tank]
            )),
            "Tank {tank} is sent: {:?}",
            decision.commands
        );
    }
    // No Depot money is held back while the site is held.
    assert!(!expansion_security::reserve_expansion(
        &obs,
        &AiFacts::from_observation(&obs),
        &JEFFS_AI,
        &mut memory,
    ));
}

#[test]
fn enemy_armor_on_the_natural_is_left_to_the_tank_fight() {
    let (obs, mut memory) = natural_held_by(EntityKind::Tank, &[(60, 5.0, 5.0)]);
    decide(&obs, &JEFFS_AI, &mut memory);
    assert_eq!(memory.expansion_security.contested_tick, Some(obs.tick));
    assert!(memory.expansion_security.clearers.is_empty());
}

fn stage(obs: &AiObservation, memory: &mut AiDecisionMemory, profile_id: &str) -> bool {
    let facts = AiFacts::from_observation(obs);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(1000, 1000, 20, 80));
    expansion_security::update_and_stage(obs, None, memory, &mut actions, profile_id)
}

#[test]
fn a_passing_enemy_the_live_jeffs_guards_and_tanks_beat_does_not_hold_the_natural() {
    let ts = config::TILE_SIZE as f32;
    let (mut obs, mut memory) = natural_held_by(EntityKind::Rifleman, &[]);
    let points = expansion_security::positions(&obs, None, &memory.expansion_security);
    for (id, point) in memory
        .expansion_security
        .riflemen
        .clone()
        .iter()
        .zip(&points)
    {
        let unit = obs.owned.iter_mut().find(|unit| unit.id == *id).unwrap();
        unit.x = point.0;
        unit.y = point.1;
    }
    let site = memory.expansion_security.site.unwrap();
    let center = building_center(site, EntityKind::ResourceDepot, obs.map.tile_size).unwrap();
    obs.owned.push(combat_at(
        60,
        EntityKind::Tank,
        center.0 - 4.0 * ts,
        center.1 - 4.0 * ts,
    ));
    let mut freeze = memory.clone();
    assert!(!stage(&obs, &mut memory, JEFFS_AI_ID));
    assert!(!stage(&obs, &mut freeze, JEFFS_AI_PRE_HELD_SITE_ID));
    obs.tick += config::TICK_HZ * 3;
    assert!(
        stage(&obs, &mut memory, JEFFS_AI_ID),
        "two guards and a Tank beat one Rifleman: the site still secures"
    );
    assert_ne!(memory.expansion_security.contested_tick, Some(obs.tick));
    assert!(
        !stage(&obs, &mut freeze, JEFFS_AI_PRE_HELD_SITE_ID),
        "the freeze counts any enemy on the site as holding it"
    );
    assert_eq!(freeze.expansion_security.contested_tick, Some(obs.tick));

    // Two enemy Tanks are more than the guards and one Tank beat: they hold it.
    for id in [901, 902] {
        let mut tank = combat_at(id, EntityKind::Tank, center.0 + 3.0 * ts, center.1);
        tank.owner = 2;
        obs.visible_enemies.push(tank);
    }
    obs.tick += 9;
    assert!(!stage(&obs, &mut memory, JEFFS_AI_ID));
    assert_eq!(memory.expansion_security.contested_tick, Some(obs.tick));
}

#[test]
fn the_live_jeff_sends_the_builder_past_enemies_on_a_site_it_holds() {
    let ts = config::TILE_SIZE as f32;
    let (mut obs, memory, site) = secured_natural();
    obs.economy.steel = 450;
    obs.economy.oil = 100;
    let center = building_center(site, EntityKind::ResourceDepot, obs.map.tile_size).unwrap();
    let mut raider = combat_at(900, EntityKind::Rifleman, center.0 + 3.0 * ts, center.1);
    raider.owner = 2;
    obs.visible_enemies.push(raider);
    assert!(!expansion_security::natural_attempt_ready(
        &obs, None, &memory, true, true
    ));
    assert!(expansion_security::natural_attempt_ready(
        &obs, None, &memory, true, false
    ));
}

/// A guard out at its far post keeps it when another Rifleman passes nearer the site: the live
/// Jeff fills only vacancies, where the frozen Jeff re-picks the nearest two every decision.
#[test]
fn a_live_guard_keeps_its_post_when_another_rifleman_passes_nearer() {
    for (profile, kept) in [(&JEFFS_AI, true), (&JEFFS_AI_PRE_TANK_MICRO, false)] {
        let mut obs = security_observation();
        // Beyond the four oldest, kept home, four Riflemen can guard the natural.
        let ts = config::TILE_SIZE as f32;
        obs.owned
            .extend((16..18).map(|id| combat_at(id, EntityKind::Rifleman, 14.0 * ts, 14.0 * ts)));
        let mut memory = AiDecisionMemory::for_profile(profile);
        let prepare = |obs: &AiObservation, memory: &mut AiDecisionMemory| {
            let facts = AiFacts::from_observation(obs);
            expansion_security::prepare(obs, &facts, profile, memory, None, &mut |_, _, _| true);
        };
        prepare(&obs, &mut memory);
        let guards = memory.expansion_security.riflemen.clone();
        assert_eq!(guards.len(), 2, "{}", profile.id);
        let site = memory.expansion_security.site.unwrap();
        let center = building_center(site, EntityKind::ResourceDepot, obs.map.tile_size).unwrap();
        let passer = (14..18).find(|id| !guards.contains(id)).unwrap();
        for unit in obs.owned.iter_mut() {
            if unit.id == guards[0] {
                unit.x = 60.0 * ts;
                unit.y = 60.0 * ts;
            } else if unit.id == passer {
                unit.x = center.0 + ts;
                unit.y = center.1;
            }
        }
        obs.tick += 9;
        prepare(&obs, &mut memory);
        assert_eq!(
            memory.expansion_security.riflemen.contains(&guards[0]),
            kept,
            "{}: {:?}",
            profile.id,
            memory.expansion_security.riflemen
        );
    }
}

#[test]
fn the_live_jeff_leaves_a_guard_raid_defense_holds_to_raid_defense() {
    for (profile, live) in [(&JEFFS_AI, true), (&JEFFS_AI_PRE_GUARD_HANDOFF, false)] {
        let obs = security_observation();
        let facts = AiFacts::from_observation(&obs);
        let mut memory = AiDecisionMemory::for_profile(profile);
        expansion_security::prepare(&obs, &facts, profile, &mut memory, None, &mut |_, _, _| {
            true
        });
        let guards = memory.expansion_security.riflemen.clone();
        assert_eq!(guards.len(), 2, "{}", profile.id);
        // Raid defense has pulled the first guard back: its post is not re-sent over that.
        memory.raid_defense.hold_for_test(guards[0]);
        let mut actions = AiActionContext::new(&facts, SpendBudget::new(1000, 1000, 20, 80));
        expansion_security::update_and_stage(&obs, None, &mut memory, &mut actions, profile.id);
        let commands = actions.into_commands();
        let ordered = |id: u32| {
            commands.iter().any(|command| match command {
                Command::AttackMove { units, .. } | Command::HoldPosition { units, .. } => {
                    units.contains(&id)
                }
                _ => false,
            })
        };
        assert_eq!(ordered(guards[0]), !live, "{}: {commands:?}", profile.id);
        assert!(ordered(guards[1]), "{}: {commands:?}", profile.id);
        // Nor is a Rifleman raid defense holds picked as a new guard.
        let mut memory = AiDecisionMemory::for_profile(profile);
        memory.raid_defense.hold_for_test(guards[0]);
        expansion_security::prepare(&obs, &facts, profile, &mut memory, None, &mut |_, _, _| {
            true
        });
        assert_eq!(
            memory.expansion_security.riflemen.contains(&guards[0]),
            !live,
            "{}: {:?}",
            profile.id,
            memory.expansion_security.riflemen
        );
    }
}

#[test]
fn the_live_jeff_leaves_a_guard_in_a_home_fight_to_home_defense() {
    for (profile, live) in [(&JEFFS_AI, true), (&JEFFS_AI_PRE_GUARD_HANDOFF, false)] {
        let obs = security_observation();
        let facts = AiFacts::from_observation(&obs);
        let mut memory = AiDecisionMemory::for_profile(profile);
        expansion_security::prepare(&obs, &facts, profile, &mut memory, None, &mut |_, _, _| {
            true
        });
        let guards = memory.expansion_security.riflemen.clone();
        assert_eq!(guards.len(), 2, "{}", profile.id);
        // Home defense met a raid beside both guards a moment ago, and raid defense saw it.
        let guard = obs.owned.iter().find(|unit| unit.id == guards[0]).unwrap();
        memory.note_defensive_contact(obs.tick, (guard.x, guard.y), 100, false);
        memory
            .raid_defense
            .see_raid_for_test(obs.tick, (guard.x, guard.y));
        let mut actions = AiActionContext::new(&facts, SpendBudget::new(1000, 1000, 20, 80));
        expansion_security::update_and_stage(&obs, None, &mut memory, &mut actions, profile.id);
        let commands = actions.into_commands();
        let ordered = |id: u32| {
            commands.iter().any(|command| match command {
                Command::AttackMove { units, .. } | Command::HoldPosition { units, .. } => {
                    units.contains(&id)
                }
                _ => false,
            })
        };
        assert_eq!(ordered(guards[0]), !live, "{}: {commands:?}", profile.id);
        // Once the contact is old, the guards go out again.
        let mut later = obs.clone();
        later.tick += config::TICK_HZ * 3;
        let mut actions = AiActionContext::new(&facts, SpendBudget::new(1000, 1000, 20, 80));
        expansion_security::update_and_stage(&later, None, &mut memory, &mut actions, profile.id);
        let commands = actions.into_commands();
        assert!(
            commands.iter().any(|command| matches!(
                command,
                Command::AttackMove { units, .. } if units.contains(&guards[0])
            )),
            "{}: {commands:?}",
            profile.id
        );
    }
}

#[test]
fn the_live_jeff_keeps_guards_out_of_a_raid_raid_defense_leaves_to_the_line() {
    for (profile, live) in [(&JEFFS_AI, true), (&JEFFS_AI_PRE_GUARD_HANDOFF, false)] {
        let obs = security_observation();
        let facts = AiFacts::from_observation(&obs);
        let mut memory = AiDecisionMemory::for_profile(profile);
        expansion_security::prepare(&obs, &facts, profile, &mut memory, None, &mut |_, _, _| {
            true
        });
        let guards = memory.expansion_security.riflemen.clone();
        // A raid raid defense saw but did not answer, three tiles from the first guard.
        let guard = obs.owned.iter().find(|unit| unit.id == guards[0]).unwrap();
        let ts = config::TILE_SIZE as f32;
        memory
            .raid_defense
            .see_raid_for_test(obs.tick, (guard.x + 3.0 * ts, guard.y));
        let mut actions = AiActionContext::new(&facts, SpendBudget::new(1000, 1000, 20, 80));
        expansion_security::update_and_stage(&obs, None, &mut memory, &mut actions, profile.id);
        let commands = actions.into_commands();
        let ordered = commands.iter().any(|command| {
            matches!(command, Command::AttackMove { units, .. } if units.contains(&guards[0]))
        });
        assert_eq!(ordered, !live, "{}: {commands:?}", profile.id);
    }
}
