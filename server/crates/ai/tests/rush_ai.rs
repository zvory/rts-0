use rts_ai::{AiAlivePolicy, AiController, CanonicalAiTickDriver};
use rts_sim::game::command::SimCommand;
use rts_sim::game::entity::EntityKind;
use rts_sim::game::lab::LabOp;
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::Command;
use std::collections::BTreeSet;

#[test]
fn rush_opening_transitions_research_production_and_respects_engineer_standoff() {
    check_rush_opening(None);
}

#[test]
fn rush_classic_keeps_the_engineer_back_and_finishes_the_timed_transitions() {
    check_rush_opening(Some("Classic"));
}

fn check_rush_opening(map_name: Option<&str>) {
    let players = [1, 2].map(|id| PlayerInit {
        id,
        team_id: id,
        faction_id: "kriegsia".into(),
        name: format!("Player {id}"),
        color: "#4cc9f0".into(),
        is_ai: true,
    });
    // Only player 1 has a controller; player 2 is an inert ordinary starting army.
    let mut game = if let Some(name) = map_name {
        let map = rts_sim::game::map::Map::load_for_players(name, &[(1, 1), (2, 2)], 7).unwrap();
        Game::new_with_random_ai_profiles_and_map_metadata(
            &players,
            7,
            map,
            rts_sim::game::map::Map::metadata_for_name(name).unwrap(),
        )
    } else {
        Game::new_without_ai_controllers(&players, 7)
    };
    // Keep the passive base alive beyond 2:00 so all timed transitions are exercised.
    game.apply_lab_op(LabOp::SetPlayerGodMode {
        player_id: 2,
        enabled: true,
    })
    .unwrap();
    let start = game.start_payload();
    let enemy = start.players.iter().find(|p| p.id == 2).unwrap();
    let size = start.map.tile_size as f32;
    let target = (
        (enemy.start_tile_x as f32 + 0.5) * size,
        (enemy.start_tile_y as f32 + 0.5) * size,
    );
    let mut controllers = [AiController::with_profile_id(1, "rush_ai")];
    assert_eq!(controllers[0].profile_id(), "rush_ai");
    let mut training_started = None;
    let mut research_queued = None;
    let mut training_complete = None;
    let mut meth_ninety = None;
    let mut panzerfausts_complete = None;
    let mut closest_engineer_distance = f32::MAX;
    let mut panzerfaust_produced = false;
    let mut manual_rifle_queued = false;
    let mut rifle_finished_after_training = false;
    let mut seen_riflemen = BTreeSet::new();
    for _ in 0..6000 {
        CanonicalAiTickDriver::run(
            &mut game,
            &mut controllers,
            AiAlivePolicy::StartingPrimaryBase,
        );
        game.tick();
        let snapshot = game.snapshot_for(1);
        // Put an explicit Rifleman behind the current unit just before the transition.
        if !manual_rifle_queued
            && snapshot.entities.iter().any(|e| {
                e.owner == 1
                    && e.kind == "training_centre"
                    && e.build_progress.is_some_and(|progress| progress > 0.9)
            })
        {
            let barracks = snapshot
                .entities
                .iter()
                .find(|e| e.owner == 1 && e.kind == "barracks")
                .unwrap();
            game.enqueue(
                1,
                SimCommand::Train {
                    building: barracks.id,
                    unit: EntityKind::Rifleman,
                },
            );
            manual_rifle_queued = true;
        }
        for rifle in snapshot
            .entities
            .iter()
            .filter(|e| e.owner == 1 && e.kind == "rifleman")
        {
            if seen_riflemen.insert(rifle.id) && training_complete.is_some() {
                rifle_finished_after_training = true;
            }
        }
        for entity in snapshot.entities.iter().filter(|e| e.owner == 1) {
            if entity.kind == "worker" {
                closest_engineer_distance = closest_engineer_distance
                    .min((entity.x - target.0).hypot(entity.y - target.1) / size);
                assert!(closest_engineer_distance >= 30.0, "Engineer entered the 30-tile exclusion radius at tick {}: {closest_engineer_distance}", snapshot.tick);
            }
            panzerfaust_produced |= entity.kind == "panzerfaust";
        }
        if training_complete.is_none()
            && snapshot
                .entities
                .iter()
                .any(|e| e.owner == 1 && e.kind == "training_centre" && e.build_progress.is_none())
        {
            training_complete = Some(snapshot.tick);
        }
        if meth_ninety.is_none()
            && (snapshot
                .upgrades
                .iter()
                .any(|upgrade| upgrade == "methamphetamines")
                || snapshot.entities.iter().any(|entity| {
                    entity.owner == 1
                        && entity.prod_upgrade.as_deref() == Some("methamphetamines")
                        && entity.prod_progress.is_some_and(|progress| progress >= 0.9)
                }))
        {
            meth_ninety = Some(snapshot.tick);
        }
        if panzerfausts_complete.is_none()
            && snapshot
                .upgrades
                .iter()
                .any(|upgrade| upgrade == "panzerfausts")
        {
            panzerfausts_complete = Some(snapshot.tick);
        }
        if training_started.is_none()
            && snapshot
                .entities
                .iter()
                .any(|e| e.owner == 1 && e.kind == "training_centre")
        {
            training_started = Some(snapshot.tick);
        }
        if research_queued.is_none()
            && snapshot.entities.iter().any(|e| {
                e.owner == 1
                    && e.kind == "training_centre"
                    && e.prod_upgrade.as_deref() == Some("methamphetamines")
            })
        {
            research_queued = Some(snapshot.tick);
        }
    }
    let commands = game.command_log();
    let builds = commands
        .iter()
        .filter_map(|entry| match &entry.command {
            Command::Build {
                building,
                tile_x,
                tile_y,
                ..
            } if building == "barracks" || building == "training_centre" => {
                Some((entry.tick, building.as_str(), *tile_x, *tile_y))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    eprintln!("rush builds: {builds:?}; training={training_started:?}; meth={research_queued:?}");
    eprintln!(
        "closest Engineer={closest_engineer_distance}; final resources={}/{}; upgrades={:?}",
        game.snapshot_for(1).steel,
        game.snapshot_for(1).oil,
        game.snapshot_for(1).upgrades
    );
    assert_eq!(
        builds.iter().map(|b| b.1).collect::<Vec<_>>(),
        ["barracks", "training_centre", "barracks", "barracks"]
    );
    assert!(
        research_queued.unwrap() - training_started.unwrap() <= 9,
        "research must queue on first think after scaffold appears"
    );
    assert!(
        commands
            .iter()
            .filter(|entry| entry.tick < meth_ninety.unwrap())
            .filter(|entry| {
                matches!(&entry.command, Command::AttackMove { units, x, y, .. }
            if units.len() == 1 && (*x, *y) != target)
            })
            .count()
            >= 4,
        "starting infantry must attack-move into staging before the wave unlocks"
    );
    let mut previously_launched = BTreeSet::new();
    let waves = commands
        .iter()
        .filter_map(|entry| match &entry.command {
            Command::AttackMove { units, x, y, .. } if (*x, *y) == target => {
                // Staging attack-moves are assembly, not wave launches. Flow orders retask
                // already launched units individually; keep singleton opening launches and
                // every shared wave order aimed at the enemy start.
                let first_launch = units.iter().any(|id| !previously_launched.contains(id));
                previously_launched.extend(units.iter().copied());
                (first_launch || units.len() > 1).then_some((entry.tick, units, *x, *y))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    eprintln!(
        "waves={:?}",
        waves
            .iter()
            .map(|wave| (wave.0, wave.1.len()))
            .collect::<Vec<_>>()
    );
    assert!(!waves.is_empty());
    assert!(
        waves[0].0 >= meth_ninety.unwrap(),
        "opening wave must wait for 90% Methamphetamines research"
    );
    let mut launched = BTreeSet::new();
    for (index, (tick, units, x, y)) in waves.iter().enumerate() {
        assert_eq!((*x, *y), target);
        if index > 0 {
            assert!(
                units.len() >= 5,
                "wave {index} at {tick} has only {} units",
                units.len()
            );
        }
        for unit in units.iter() {
            assert!(
                launched.insert(*unit),
                "launched units must not be redirected into a later wave"
            );
        }
    }
    let research = commands
        .iter()
        .filter_map(|entry| match &entry.command {
            Command::Research { upgrade, .. } => Some((entry.tick, upgrade.as_str())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        research.iter().map(|entry| entry.1).collect::<Vec<_>>(),
        ["methamphetamines", "entrenchment", "panzerfausts"]
    );
    assert!(
        (3600..=3609).contains(&research[2].0),
        "Panzerfausts must queue on the first decision at/after 2:00: {research:?}"
    );
    let repeats = commands
        .iter()
        .filter_map(|entry| match &entry.command {
            Command::AdjustProductionRepeat {
                buildings,
                unit,
                delta,
            } if ["rifleman", "machine_gunner", "panzerfaust"].contains(&unit.as_str()) => {
                Some((entry.tick, buildings[0], unit.as_str(), *delta))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let retirements = repeats
        .iter()
        .filter(|entry| entry.2 == "rifleman" && entry.3 == -1)
        .collect::<Vec<_>>();
    assert_eq!(
        retirements.len(),
        1,
        "only the first Barracks should ever repeat Riflemen: {repeats:?}"
    );
    assert!(
        (training_complete.unwrap()..=training_complete.unwrap() + 9).contains(&retirements[0].0)
    );
    assert!(!repeats.iter().any(|entry| entry.2 == "rifleman"
        && entry.3 == 1
        && entry.0 >= training_complete.unwrap()));
    assert!(
        !commands
            .iter()
            .any(|entry| matches!(&entry.command, Command::Cancel { .. })),
        "production transition must not cancel queued Riflemen"
    );
    for kind in ["machine_gunner", "panzerfaust"] {
        let additions = repeats
            .iter()
            .filter(|entry| entry.2 == kind && entry.3 == 1)
            .collect::<Vec<_>>();
        assert_eq!(
            additions.len(),
            3,
            "each Barracks must enable {kind} exactly once: {repeats:?}"
        );
        for addition in additions {
            if kind == "panzerfaust" {
                assert!(addition.0 >= panzerfausts_complete.unwrap());
            }
            assert!(commands.iter().any(|entry| matches!(&entry.command, Command::SetRally { building, x, y, kind, .. } if *building == addition.1 && (*x, *y) != target && (((*x-target.0).hypot(*y-target.1)/size)-20.0).abs()<0.01 && kind.as_deref() == Some("attackMove"))));
        }
    }
    assert!(
        manual_rifle_queued && rifle_finished_after_training,
        "an already queued Rifleman must finish after the Training Centre completes"
    );
    assert!(
        panzerfaust_produced,
        "the researched unit must actually enter play"
    );
    eprintln!("research={research:?}; repeat transitions={repeats:?}; closest Engineer={closest_engineer_distance:.2} tiles");
}
