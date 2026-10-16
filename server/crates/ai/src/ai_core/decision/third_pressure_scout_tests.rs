//! The scout Riflemen's trip, the siege's spotters and the stop-short view.

use super::*;

/// The scouts out alone at the end of their route, the lead standing at `at` (tiles).
fn scout_at_base(at: (f32, f32)) -> (AiDecisionMemory, AiObservation) {
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    step(JEFFS_AI_ID, &game(10_000), &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Scout);
    let state = &mut memory.third_pressure;
    state.leg = state.route.len();
    let lead = state.scouts[0];
    let mut observation = game(10_009);
    place(&mut observation, lead, at);
    (memory, observation)
}

fn lead(memory: &AiDecisionMemory) -> u32 {
    memory.third_pressure.scouts[0]
}

/// The scouts out on their way, the lead at `at` (tiles), far from home.
fn scouts_out(at: (f32, f32)) -> (AiDecisionMemory, AiObservation) {
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    step(JEFFS_AI_ID, &game(10_000), &mut memory);
    let mut observation = game(10_009);
    place(&mut observation, lead(&memory), at);
    (memory, observation)
}

#[test]
fn the_scouts_turn_away_from_enemies_before_they_see_them() {
    let (mut memory, mut observation) = scouts_out((60.0, 40.0));
    let id = lead(&memory);
    // A Rifleman 12 tiles off: inside the scouts' danger range, outside its own sight.
    observation
        .visible_enemies
        .push(unit(700, 2, EntityKind::Rifleman, (72.0, 40.0)));
    let commands = step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &memory.third_pressure;
    assert_eq!(state.phase, Phase::Scout);
    assert_eq!(state.flees, 1);
    assert!(state.fallback_until > 10_009);
    assert_eq!(state.dangers.len(), 1);
    let (x, _) = move_of(&commands, id).expect("the lead moves");
    assert!(x < 55.0 * ts(), "{x}");
    // It never attacks.
    assert!(!commands.iter().any(|command| matches!(
        command,
        Command::AttackMove { units, .. } | Command::Attack { units, .. } if units.contains(&id)
    )));
    // Found again and again, they give the trip up (and, near home, disband on the spot).
    memory.third_pressure.flees = 4;
    memory.third_pressure.fallback_until = 0;
    let mut observation = observation.clone();
    observation.tick = 10_400;
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert!(matches!(
        memory.third_pressure.phase,
        Phase::Return | Phase::Idle
    ));
    assert!(memory.third_pressure.tanks.is_empty());
}

#[test]
fn near_home_the_scouts_wait_raiders_out_without_giving_up() {
    // Raiders 10 tiles from a lead still beside home: home defense answers them; the scouts turn
    // away and wait, and the encounter does not count toward giving the trip up.
    let (mut memory, mut observation) = scouts_out((20.0, 20.0));
    memory.third_pressure.flees = 4;
    observation
        .visible_enemies
        .push(unit(700, 2, EntityKind::Rifleman, (30.0, 20.0)));
    step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &memory.third_pressure;
    assert_eq!(state.phase, Phase::Scout);
    assert_eq!(state.flees, 4);
    assert!(state.fallback_until > 10_009);
}

#[test]
fn the_scouts_walk_around_enemies_they_met_on_their_way() {
    let (mut memory, observation) = scouts_out((40.0, 30.0));
    step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &mut memory.third_pressure;
    let legs = state.route.len();
    let next = unrounded(state.route[state.leg]);
    // Enemies met lately stand right on the way to the next waypoint.
    let lead_at = (40.0 * ts(), 30.0 * ts());
    let midway = ((lead_at.0 + next.0) * 0.5, (lead_at.1 + next.1) * 0.5);
    state.dangers = vec![(rounded(midway), 10_009)];
    let mut observation = observation.clone();
    observation.tick = 10_018;
    let commands = step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &memory.third_pressure;
    assert_eq!(state.route.len(), legs + 1);
    assert_eq!(state.detours, 1);
    let around = unrounded(state.route[state.leg]);
    assert!(distance(around, midway) >= 16.0 * ts(), "{around:?}");
    let sent = move_of(&commands, lead(&memory)).expect("the lead walks around");
    assert!(distance(sent, around) < 1.0, "{sent:?}");
    // Once they are long gone, the way is walked straight again.
    let state = &mut memory.third_pressure;
    state.route.remove(state.leg);
    state.detours = 0;
    let mut later = observation.clone();
    later.tick = 10_018 + config::TICK_HZ * 31;
    step(JEFFS_AI_ID, &later, &mut memory);
    assert_eq!(memory.third_pressure.route.len(), legs);
}

#[test]
fn a_hurt_scout_goes_home_and_the_other_leads_on() {
    let (mut memory, mut observation) = scouts_out((60.0, 40.0));
    let first = lead(&memory);
    let second = memory.third_pressure.scouts[1];
    observation
        .owned
        .iter_mut()
        .find(|u| u.id == first)
        .unwrap()
        .hp = 30;
    step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &memory.third_pressure;
    assert_eq!(state.phase, Phase::Scout);
    assert_eq!(state.scouts, vec![second]);
    assert!(!state.is_reserved(first));
    // Both hurt, the trip is over.
    let (mut memory, mut observation) = scouts_out((60.0, 40.0));
    let both = memory.third_pressure.scouts.clone();
    for id in &both {
        observation
            .owned
            .iter_mut()
            .find(|u| u.id == *id)
            .unwrap()
            .hp = 30;
    }
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert_ne!(memory.third_pressure.phase, Phase::Scout);
    assert!(memory.third_pressure.scouts.is_empty());
}

#[test]
fn the_raid_leaves_the_home_pockets_riflemen_and_takes_those_not_dug_in_first() {
    // Riflemen 24 and 25 have stood still long enough to dig in; the others are walking.
    let mut earlier = game(9_800);
    earlier.upgrades.push(UpgradeKind::Entrenchment);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    memory.sync_defender_posture(&earlier);
    let mut observation = game(10_000);
    observation.upgrades.push(UpgradeKind::Entrenchment);
    for unit in observation
        .owned
        .iter_mut()
        .filter(|unit| unit.kind == EntityKind::Rifleman && unit.id != 24 && unit.id != 25)
    {
        unit.state = AiEntityState::Move;
    }
    memory.sync_defender_posture(&observation);
    let home = (10.5 * ts(), 10.5 * ts());
    let picked = party_riflemen(&observation, &memory, home, 5, &[]).unwrap();
    // The four oldest (20-23) hold the pocket; the rest, dug-in ones last.
    assert!(picked.iter().all(|id| !(20..24).contains(id)), "{picked:?}");
    let mut walking = picked[..4].to_vec();
    walking.sort_unstable();
    assert_eq!(walking, vec![50, 51, 52, 53], "{picked:?}");
    assert!([24, 25].contains(&picked[4]), "{picked:?}");
}

#[test]
fn hurt_riflemen_are_not_sent_scouting() {
    let mut observation = game(10_000);
    for unit in observation
        .owned
        .iter_mut()
        .filter(|unit| unit.kind == EntityKind::Rifleman)
    {
        unit.hp = 30;
    }
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Idle);
}

#[test]
fn the_lead_scout_finds_the_depot_watches_it_from_clear_ground_and_calls_the_party() {
    let (mut memory, mut observation) = scout_at_base((95.0, 62.0));
    let id = lead(&memory);
    observation.visible_enemies.extend([
        unit(500, 2, EntityKind::ResourceDepot, (121.0, 62.0)),
        unit(510, 2, EntityKind::SteelMine, (110.0, 64.0)),
    ]);
    let commands = step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &memory.third_pressure;
    assert_eq!(state.phase, Phase::Scout);
    assert_eq!(state.depot, Some(500));
    assert!(state.tanks.is_empty());
    let watch = move_of(&commands, id).expect("the lead goes to watch");
    let depot = (121.0 * ts(), 62.0 * ts());
    let mine = (110.0 * ts(), 64.0 * ts());
    assert!(distance(watch, depot) <= 10.0 * ts(), "{watch:?}");
    assert!(distance(watch, mine) >= 8.0 * ts(), "{watch:?}");
    // There, with no garrison in sight, it sends the party; the scouts wait at the staging point.
    let mut observation = observation.clone();
    observation.tick = 10_018;
    place(&mut observation, id, (watch.0 / ts(), watch.1 / ts()));
    step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &memory.third_pressure;
    assert_eq!(state.phase, Phase::March);
    assert_eq!(state.tanks.len(), 2);
    assert_eq!(state.riflemen.len(), 5);
    assert_eq!(state.watch, state.route.last().copied());
    // The party marches; the scouts do not march with it.
    let mut observation = observation.clone();
    observation.tick = 10_027;
    let commands = step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &memory.third_pressure;
    assert!(commands.iter().any(|command| {
        matches!(command, Command::Move { units, .. } if units.contains(&state.tanks[0]))
    }));
    assert!(!commands.iter().any(|command| {
        matches!(command, Command::Move { units, .. }
            if units.contains(&id) && units.contains(&state.tanks[0]))
    }));
}

#[test]
fn sized_up_with_no_tanks_to_spare_the_scouts_wait_for_them_out_of_sight() {
    let (mut memory, mut observation) = scout_at_base((95.0, 62.0));
    let id = lead(&memory);
    observation
        .visible_enemies
        .push(unit(500, 2, EntityKind::ResourceDepot, (121.0, 62.0)));
    let commands = step(JEFFS_AI_ID, &observation, &mut memory);
    let watch = move_of(&commands, id).unwrap();
    // Only two Tanks: none can be spared yet.
    let mut observation = observation.clone();
    observation.tick = 10_018;
    observation
        .owned
        .retain(|unit| unit.id != 12 && unit.id != 13);
    place(&mut observation, id, (watch.0 / ts(), watch.1 / ts()));
    step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &memory.third_pressure;
    assert_eq!(state.phase, Phase::Scout);
    assert!(state.sized_up.is_some());
    // They wait at the staging point...
    let mut waiting = observation.clone();
    waiting.tick = 10_027;
    let commands = step(JEFFS_AI_ID, &waiting, &mut memory);
    let staging = unrounded(*memory.third_pressure.route.last().unwrap());
    let sent = move_of(&commands, id).expect("the lead falls back to wait");
    assert!(distance(sent, staging) < 1.0, "{sent:?}");
    // ...and the party leaves once two Tanks can be spared.
    let mut ready = game(10_036 + config::TICK_HZ * 60);
    place(&mut ready, id, (staging.0 / ts(), staging.1 / ts()));
    step(JEFFS_AI_ID, &ready, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::March);
    assert_eq!(memory.third_pressure.tanks.len(), 2);
}

#[test]
fn a_garrison_the_party_would_lose_to_keeps_every_tank_home() {
    let (mut memory, mut observation) = scout_at_base((95.0, 62.0));
    let id = lead(&memory);
    observation
        .visible_enemies
        .push(unit(500, 2, EntityKind::ResourceDepot, (121.0, 62.0)));
    let commands = step(JEFFS_AI_ID, &observation, &mut memory);
    let watch = move_of(&commands, id).unwrap();
    let mut observation = observation.clone();
    observation.tick = 10_018;
    // Three Tanks and six Machine Gunners behind the Depot, out of the scouts' danger range.
    observation
        .visible_enemies
        .extend((0..3).map(|i| unit(600 + i, 2, EntityKind::Tank, (132.0, 58.0 + 3.0 * i as f32))));
    observation.visible_enemies.extend((0..6).map(|i| {
        unit(
            610 + i,
            2,
            EntityKind::MachineGunner,
            (130.0, 57.0 + 2.0 * i as f32),
        )
    }));
    place(&mut observation, id, (watch.0 / ts(), watch.1 / ts()));
    step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &memory.third_pressure;
    assert!(state.tanks.is_empty());
    assert_eq!(state.phase, Phase::Return);
    assert!(state
        .defended
        .iter()
        .any(|(site, _)| distance(unrounded(*site), (112.0 * ts(), 62.0 * ts())) < 2.0 * ts()));
}

#[test]
fn an_empty_base_costs_only_the_scouts_trip() {
    let (mut memory, observation) = scout_at_base((95.0, 62.0));
    let id = lead(&memory);
    step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &mut memory.third_pressure;
    assert!(state.spots_total > 20);
    let last = *state.unseen.first().unwrap();
    state.unseen = vec![last];
    let mut observation = observation.clone();
    observation.tick = 10_018;
    place(
        &mut observation,
        id,
        (last.0 as f32 / ts(), last.1 as f32 / ts()),
    );
    step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &memory.third_pressure;
    assert_eq!(state.empty_bases, 1);
    assert_eq!(state.phase, Phase::Return);
    assert!(state.tanks.is_empty());
}

#[test]
fn on_the_march_the_party_lets_distant_enemies_pass() {
    let mut memory = launched(&game(10_000));
    // A Rifleman 11 tiles from the nearest Rifleman: near, but not closing in.
    let mut observation = game(10_009);
    observation
        .visible_enemies
        .push(unit(700, 2, EntityKind::Rifleman, (27.0, 10.0)));
    let commands = step(JEFFS_AI_ID, &observation, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::March);
    assert!(commands
        .iter()
        .any(|command| matches!(command, Command::HoldPosition { .. })));
    assert!(!commands
        .iter()
        .any(|command| matches!(command, Command::AttackMove { .. })));
    // Once it closes in, the party fights it.
    let mut close = game(10_018);
    close
        .visible_enemies
        .push(unit(700, 2, EntityKind::Rifleman, (21.0, 10.0)));
    let commands = step(JEFFS_AI_ID, &close, &mut memory);
    assert!(commands
        .iter()
        .any(|command| matches!(command, Command::AttackMove { .. })));
    // Standing in the way past the wait, it is fought too.
    let mut memory = launched(&game(10_000));
    step(JEFFS_AI_ID, &observation, &mut memory);
    let mut later = observation.clone();
    later.tick = 10_009 + config::TICK_HZ * 21;
    let commands = step(JEFFS_AI_ID, &later, &mut memory);
    assert!(commands
        .iter()
        .any(|command| matches!(command, Command::AttackMove { .. })));
}

#[test]
fn the_raid_takes_no_scout_car_and_needs_none() {
    // With the push holding the only Scout Car, or with none at all, the scouts still go.
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    memory.containment.active_scout = Some(30);
    step(JEFFS_AI_ID, &game(10_000), &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Scout);
    assert!(!memory.third_pressure.is_reserved(30));
    let mut observation = game(10_000);
    observation
        .owned
        .retain(|unit| unit.kind != EntityKind::ScoutCar);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Scout);
}

#[test]
fn waiting_scouts_turn_away_while_the_party_marches_on() {
    let (mut memory, mut observation) = scout_at_base((95.0, 62.0));
    let id = lead(&memory);
    observation
        .visible_enemies
        .push(unit(500, 2, EntityKind::ResourceDepot, (121.0, 62.0)));
    let commands = step(JEFFS_AI_ID, &observation, &mut memory);
    let watch = move_of(&commands, id).unwrap();
    let mut observation = observation.clone();
    observation.tick = 10_018;
    place(&mut observation, id, (watch.0 / ts(), watch.1 / ts()));
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::March);
    // The scouts wait at the staging point; a Rifleman walks up to them, far from the party.
    let staging = unrounded(memory.third_pressure.watch.unwrap());
    let mut observation = observation.clone();
    observation.tick = 10_027;
    for scout in memory.third_pressure.scouts.clone() {
        place(
            &mut observation,
            scout,
            (staging.0 / ts(), staging.1 / ts()),
        );
    }
    observation.visible_enemies.push(unit(
        700,
        2,
        EntityKind::Rifleman,
        (staging.0 / ts() + 10.0, staging.1 / ts()),
    ));
    let commands = step(JEFFS_AI_ID, &observation, &mut memory);
    let (x, _) = move_of(&commands, id).expect("the scout turns away");
    assert!(x < staging.0 - 4.0 * ts(), "{x}");
    // The party does not stop for it.
    assert!(!commands
        .iter()
        .any(|command| matches!(command, Command::HoldPosition { .. })));
    assert_eq!(memory.third_pressure.phase, Phase::March);
}

#[test]
fn the_scouts_keep_out_of_a_parked_tanks_reach() {
    for (kind, flees) in [(EntityKind::Tank, true), (EntityKind::Rifleman, false)] {
        let (mut memory, mut observation) = scouts_out((60.0, 40.0));
        observation
            .visible_enemies
            .push(unit(700, 2, kind, (75.0, 40.0)));
        step(JEFFS_AI_ID, &observation, &mut memory);
        assert_eq!(memory.third_pressure.flees == 1, flees, "{kind:?}");
    }
}

#[test]
fn one_rifleman_spots_for_the_tanks_at_a_time_and_the_next_walks_up_after_a_wait() {
    let mut memory = launched(&game(10_000));
    let site = (112.0 * ts(), 62.0 * ts());
    let state = &mut memory.third_pressure;
    state.phase = Phase::Siege;
    state.phase_since = 12_000;
    state.target = Some(rounded(site));
    state.route = vec![rounded((90.0 * ts(), 62.0 * ts()))];
    state.watch = None;
    state.scouts.clear();
    let mut observation = game(12_000);
    party_at(&mut observation, &memory, (96.0, 62.0));
    observation
        .visible_enemies
        .push(unit(500, 2, EntityKind::ResourceDepot, (112.0, 62.0)));
    step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &memory.third_pressure;
    let first = state.spotter.expect("a spotter walks up");
    let post = |state: &ThirdPressure, id: u32| unrounded(state.posts[&id]);
    assert!(distance(post(state, first), site) <= 10.5 * ts());
    for id in state.riflemen.iter().filter(|id| **id != first) {
        assert!(distance(post(state, *id), site) >= 14.0 * ts(), "{id}");
    }
    // It falls: nobody walks up until the wait is over.
    let mut observation = observation.clone();
    observation.tick = 12_009;
    observation.owned.retain(|unit| unit.id != first);
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert_eq!(memory.third_pressure.spotter, None);
    let mut later = observation.clone();
    later.tick = 12_000 + SPOTTER_FEED_TICKS + 9;
    step(JEFFS_AI_ID, &later, &mut memory);
    let next = memory.third_pressure.spotter.expect("the next walks up");
    assert_ne!(next, first);
    // Hurt, a spotter steps back behind the Tanks.
    let mut hurt = later.clone();
    hurt.tick += 9;
    hurt.owned.iter_mut().find(|u| u.id == next).unwrap().hp = 20;
    step(JEFFS_AI_ID, &hurt, &mut memory);
    let state = &memory.third_pressure;
    assert_ne!(state.spotter, Some(next));
    assert!(distance(post(state, next), site) >= 14.0 * ts());
}

#[test]
fn a_scout_that_stops_short_of_its_view_still_sees_the_spot_and_goes_on() {
    let (mut memory, mut observation) = scout_at_base((100.0, 48.0));
    let id = lead(&memory);
    let commands = step(JEFFS_AI_ID, &observation, &mut memory);
    let view = move_of(&commands, id).expect("sent to a view");
    let unseen = memory.third_pressure.unseen.len();
    assert!(unseen > 0);
    // It stops 2 tiles short, coming in from outside the base.
    let center = locate::area_center(&memory.third_pressure);
    let out = normalized_direction(center, view).unwrap();
    place(
        &mut observation,
        id,
        (
            (view.0 + out.0 * 2.0 * ts()) / ts(),
            (view.1 + out.1 * 2.0 * ts()) / ts(),
        ),
    );
    observation.tick += 9;
    let commands = step(JEFFS_AI_ID, &observation, &mut memory);
    assert!(memory.third_pressure.unseen.len() < unseen);
    let next = move_of(&commands, id).expect("sent on to the next view");
    assert!(distance(next, view) > 1.0 * ts(), "{next:?} {view:?}");
}

#[test]
fn the_scouts_walk_around_one_enemy_group_once() {
    let (mut memory, observation) = scouts_out((40.0, 30.0));
    step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &mut memory.third_pressure;
    let next = unrounded(state.route[state.leg]);
    let lead_at = (40.0 * ts(), 30.0 * ts());
    let midway = ((lead_at.0 + next.0) * 0.5, (lead_at.1 + next.1) * 0.5);
    state.dangers = vec![(rounded(midway), 10_009)];
    for tick in [10_018, 10_027, 10_036] {
        let mut later = observation.clone();
        later.tick = tick;
        step(JEFFS_AI_ID, &later, &mut memory);
    }
    assert_eq!(memory.third_pressure.detours, 1);
}

#[test]
fn on_the_way_the_second_scout_keeps_two_legs_behind() {
    let (mut memory, mut observation) = scouts_out((42.0, 31.0));
    let state = &mut memory.third_pressure;
    state.route = [
        (20.0, 20.0),
        (30.0, 25.0),
        (40.0, 30.0),
        (50.0, 35.0),
        (60.0, 40.0),
    ]
    .iter()
    .map(|p| rounded((p.0 * ts(), p.1 * ts())))
    .collect();
    state.leg = 3;
    let trailer = state.scouts[1];
    let two_back = unrounded(state.route[1]);
    place(&mut observation, trailer, (25.0, 22.0));
    let commands = step(JEFFS_AI_ID, &observation, &mut memory);
    let sent = move_of(&commands, trailer).expect("the trailer follows");
    assert!(distance(sent, two_back) < 1.0, "{sent:?} {two_back:?}");
}

#[test]
fn waiting_for_the_party_the_scouts_stay_a_leg_apart() {
    let (mut memory, mut observation) = scout_at_base((95.0, 62.0));
    let id = lead(&memory);
    let trailer = memory.third_pressure.scouts[1];
    observation
        .visible_enemies
        .push(unit(500, 2, EntityKind::ResourceDepot, (121.0, 62.0)));
    let commands = step(JEFFS_AI_ID, &observation, &mut memory);
    let watch = move_of(&commands, id).unwrap();
    // Only two Tanks: they wait.
    let mut observation = observation.clone();
    observation.tick = 10_018;
    observation
        .owned
        .retain(|unit| unit.id != 12 && unit.id != 13);
    place(&mut observation, id, (watch.0 / ts(), watch.1 / ts()));
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert!(memory.third_pressure.sized_up.is_some());
    let mut waiting = observation.clone();
    waiting.tick = 10_027;
    let commands = step(JEFFS_AI_ID, &waiting, &mut memory);
    let route = &memory.third_pressure.route;
    let staging = unrounded(route[route.len() - 1]);
    let back = unrounded(route[route.len() - 2]);
    let lead_to = move_of(&commands, id).unwrap();
    let trail_to = move_of(&commands, trailer).unwrap();
    assert!(distance(lead_to, staging) < 1.0);
    assert!(distance(trail_to, back) < 1.0);
    assert!(
        distance(lead_to, trail_to) >= 6.0 * ts(),
        "{lead_to:?} {trail_to:?}"
    );
}

#[test]
fn no_party_is_sent_into_a_fight_at_home() {
    let (mut memory, mut observation) = scout_at_base((95.0, 62.0));
    let id = lead(&memory);
    observation
        .visible_enemies
        .push(unit(500, 2, EntityKind::ResourceDepot, (121.0, 62.0)));
    let commands = step(JEFFS_AI_ID, &observation, &mut memory);
    let watch = move_of(&commands, id).unwrap();
    // Enemy Riflemen beside the home Tanks, outside the base's defense zone.
    let mut observation = observation.clone();
    observation.tick = 10_018;
    place(&mut observation, id, (watch.0 / ts(), watch.1 / ts()));
    let trailer = memory.third_pressure.scouts[1];
    place(&mut observation, trailer, (90.0, 62.0));
    observation
        .visible_enemies
        .push(unit(700, 2, EntityKind::Rifleman, (24.0, 13.0)));
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Scout);
    assert!(memory.third_pressure.tanks.is_empty());
    // Once they are gone, it goes.
    let mut clear = observation.clone();
    clear.tick = 10_027;
    clear.visible_enemies.retain(|enemy| enemy.id != 700);
    step(JEFFS_AI_ID, &clear, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::March);
}
