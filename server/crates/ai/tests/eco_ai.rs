//! Controlled authoritative progression, with ordinary income and no injected economy.
use rts_ai::{AiAlivePolicy, AiController, CanonicalAiTickDriver};
use rts_sim::game::lab::LabOp;
use rts_sim::game::{Game, PlayerInit};

#[test]
fn eco_classic_opening_reaches_three_bases_with_staged_vehicle_works() {
    run_economy_progression(3);
}

#[test]
fn eco_extended_economy_claims_seven_bases() {
    // Match the repository's full-AI opt-in so the soak runs in --full-ai coverage.
    let full_ai = ["RTS_FULL_AI_TESTS", "RTS_SELFPLAY_FULL"]
        .into_iter()
        .any(|name| {
            matches!(
                std::env::var(name)
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .as_str(),
                "1" | "true" | "yes" | "on"
            )
        });
    if !full_ai {
        eprintln!(
            "skipping seven-base Eco progression; set RTS_FULL_AI_TESTS=1 to run full AI coverage"
        );
        return;
    }
    run_economy_progression(7);
}

fn run_economy_progression(goal: usize) {
    let players = [1, 2].map(|id| PlayerInit {
        id,
        team_id: id,
        faction_id: "kriegsia".into(),
        name: format!("Player {id}"),
        color: "#4cc9f0".into(),
        is_ai: true,
    });
    let map = rts_sim::game::map::Map::load_for_players("Classic", &[(1, 1), (2, 2)], 7).unwrap();
    let mut game = Game::new_with_random_ai_profiles_and_map_metadata(
        &players,
        7,
        map,
        rts_sim::game::map::Map::metadata_for_name("Classic").unwrap(),
    );
    game.apply_lab_op(LabOp::SetPlayerGodMode {
        player_id: 2,
        enabled: true,
    })
    .unwrap();
    let resources = game.start_payload().map.resources;
    let mut research_scaffold_checked = false;
    let mut seen_commands = 0;
    let mut checked_depots = std::collections::BTreeSet::new();
    let mut controllers = [AiController::with_profile_id(1, "eco_ai")];
    let mut first_two = None;
    let mut first_three = None;
    let mut first_four = None;
    let mut first_five = None;
    let mut first_six = None;
    let mut third_works = None;
    let mut meth_started_at_bases = None;
    let mut entrenched = false;
    let mut pf_seen = false;
    let mut tank_seen = false;
    let mut first_works = None;
    let mut second_works = None;
    let mut mg_seen = false;
    let mut mg_at_complex = None;
    let mut tank_first_seen = std::collections::BTreeMap::new();
    let mut commanded_tanks = std::collections::BTreeSet::new();
    let tick_budget = if goal > 3 { 30_000 } else { 18_000 };
    for _ in 0..tick_budget {
        CanonicalAiTickDriver::run(
            &mut game,
            &mut controllers,
            AiAlivePolicy::StartingPrimaryBase,
        );
        game.tick();
        for entry in &game.command_log()[seen_commands..] {
            if entry.player_id == 1 {
                match &entry.command {
                    rts_sim::protocol::Command::AttackMove { units, .. }
                    | rts_sim::protocol::Command::Move { units, .. }
                    | rts_sim::protocol::Command::HoldPosition { units, .. } => {
                        commanded_tanks.extend(units.iter().copied());
                    }
                    _ => {}
                }
            }
            if let rts_sim::protocol::Command::Research { building, upgrade } = &entry.command {
                if entry.player_id == 1 && upgrade == "tank_unlock" {
                    let snapshot = game.snapshot_for(1);
                    assert!(
                        snapshot
                            .entities
                            .iter()
                            .any(|e| e.id == *building && e.build_progress.is_some()),
                        "Tank Production must be queued on the Engineering Complex scaffold"
                    );
                    research_scaffold_checked = true;
                }
            }
        }
        seen_commands = game.command_log().len();
        if game.current_tick() % 30 != 0 {
            continue;
        }
        let s = game.snapshot_for(1);
        let owned: Vec<_> = s.entities.iter().filter(|e| e.owner == 1).collect();
        let bases = owned
            .iter()
            .filter(|e| e.kind == "resource_depot" && e.build_progress.is_none())
            .count();
        for depot in owned
            .iter()
            .filter(|e| e.kind == "resource_depot" && e.build_progress.is_none())
        {
            if checked_depots.insert(depot.id) {
                let reach = rts_rules::balance::MINING_ANCHOR_RANGE_TILES
                    * rts_rules::balance::TILE_SIZE as f32;
                let covered = |kind: &str| {
                    resources
                        .iter()
                        .filter(|r| {
                            r.kind == kind
                                && (r.x - depot.x) * (r.x - depot.x)
                                    + (r.y - depot.y) * (r.y - depot.y)
                                    <= reach * reach
                        })
                        .count()
                };
                assert!(
                    covered("steel") >= rts_rules::balance::STEEL_PATCHES_PER_BASE as usize,
                    "Depot {} is outside its Steel patches",
                    depot.id
                );
                assert!(
                    covered("oil") >= rts_rules::balance::OIL_PATCHES_PER_BASE as usize,
                    "Depot {} is outside its Oil patches",
                    depot.id
                );
            }
        }
        let barracks = owned
            .iter()
            .filter(|e| e.kind == "barracks" && e.build_progress.is_none())
            .count();
        assert!(
            owned.iter().filter(|e| e.kind == "barracks").count() <= 3,
            "Eco must never construct a fourth Barracks"
        );
        if bases >= 2 && first_two.is_none() {
            first_two = Some(s.tick);
            assert!(s.upgrades.iter().any(|u| u == "entrenchment"));
        }
        if bases >= 3 && first_three.is_none() {
            first_three = Some(s.tick);
        }
        let works = owned.iter().filter(|e| e.kind == "factory").count();
        if works >= 1 && first_works.is_none() {
            first_works = Some((s.tick, bases));
            assert_eq!(bases, 2);
        }
        if works >= 2 && second_works.is_none() {
            second_works = Some((s.tick, bases));
            assert_eq!(bases, 3);
        }
        if works >= 3 && third_works.is_none() {
            third_works = Some((s.tick, bases));
            assert_eq!(bases, 4, "the third Works belongs to the four-base stage");
        }
        if bases >= 5 && first_five.is_none() {
            first_five = Some(s.tick);
            assert!(
                owned
                    .iter()
                    .filter(|e| e.kind == "factory" && e.build_progress.is_none())
                    .count()
                    >= 3,
                "complete the third Works before expanding to base five"
            );
        }
        if bases >= 6 && first_six.is_none() {
            first_six = Some(s.tick);
        }
        assert!(
            bases >= 7 || works <= works_target_for_test(bases),
            "no surplus Works before seven completed bases"
        );
        assert!(bases <= 7, "seven bases is the Classic expansion cap");
        let mg_count = owned.iter().filter(|e| e.kind == "machine_gunner").count();
        if mg_at_complex.is_none() && owned.iter().any(|e| e.kind == "engineering_complex") {
            mg_at_complex = Some(mg_count);
        }
        for tank in owned.iter().filter(|e| e.kind == "tank") {
            let first = *tank_first_seen.entry(tank.id).or_insert(s.tick);
            if s.tick.saturating_sub(first) >= 60 {
                assert!(commanded_tanks.contains(&tank.id),
                    "new Tank {} never received a defensive or response order; tick={} pos=({}, {}) state={}",
                    tank.id,s.tick,tank.x,tank.y,tank.state);
            }
        }
        tank_seen |= owned.iter().any(|e| e.kind == "tank");
        if bases >= 4 && first_four.is_none() {
            first_four = Some(s.tick);
            assert!(
                barracks >= 3,
                "three-base stage must complete three Barracks before base four"
            );
        }
        if meth_started_at_bases.is_none()
            && owned
                .iter()
                .any(|e| e.prod_upgrade.as_deref() == Some("methamphetamines"))
        {
            meth_started_at_bases = Some(bases);
            assert_eq!(
                bases, 2,
                "second-base tech must not be deferred until later expansion"
            );
        }
        assert!(
            owned
                .iter()
                .filter(|e| e.kind == "barracks")
                .all(|e| e.prod_queue.unwrap_or(0) <= 1),
            "Eco must never create an unobservable second troop queue item"
        );
        mg_seen |= owned.iter().any(|e| e.kind == "machine_gunner");
        pf_seen |= owned.iter().any(|e| e.kind == "panzerfaust");
        entrenched |= owned
            .iter()
            .any(|e| e.kind == "machine_gunner" && e.occupied_trench_id.is_some());
        if s.tick % 1500 == 0 {
            eprintln!("Eco tick={} bases={bases} barracks={barracks} bank={}/{} supply={}/{} upgrades={:?} troops={}/{} trenches={entrenched}",s.tick,s.steel,s.oil,s.supply_used,s.supply_cap,s.upgrades,owned.iter().filter(|e|e.kind=="machine_gunner").count(),owned.iter().filter(|e|e.kind=="panzerfaust").count());
        }
        if bases >= goal
            && works >= works_target_for_test(goal)
            && tank_seen
            && mg_seen
            && pf_seen
            && entrenched
            && meth_started_at_bases == Some(2)
            && s.upgrades.iter().any(|u| u == "methamphetamines")
        {
            eprintln!("Eco milestones: two={first_two:?} three={first_three:?} four={first_four:?} five={first_five:?} six={first_six:?}, tank production and complete defense tick={}",s.tick);
            assert!(
                first_works.is_some()
                    && second_works.is_some()
                    && (goal < 4 || third_works.is_some())
                    && research_scaffold_checked
            );
            assert!(
                mg_count > mg_at_complex.unwrap_or(usize::MAX),
                "standing MG production must continue after the Complex is ordered"
            );
            assert!(
                game.command_log().iter().all(|e| e.player_id != 1
                    || !matches!(
                &e.command, rts_sim::protocol::Command::Train {unit, ..}
                    if unit == "machine_gunner" || unit == "panzerfaust" || unit == "tank")),
                "combat production must use native standing orders"
            );
            return;
        }
    }
    let s = game.snapshot_for(1);
    panic!("Eco incomplete: two={first_two:?} three={first_three:?} four={first_four:?} MG={mg_seen} PF={pf_seen} entrenched={entrenched} bank={}/{} upgrades={:?} own={:?}",s.steel,s.oil,s.upgrades,s.entities.iter().filter(|e|e.owner==1).map(|e|(&e.kind,&e.state,e.build_progress)).collect::<Vec<_>>());
}

/// Mirrors the release matchup rather than lib-test Jeff (which enables extra ablation features).
#[test]
fn eco_expands_through_jeff_opening_without_blocking_its_footprint() {
    let players =
        [(1, "eco_ai", "#4cc9f0"), (2, "jeffs_ai_v1_1", "#f72585")].map(|(id, name, color)| {
            PlayerInit {
                id,
                team_id: id,
                faction_id: "kriegsia".into(),
                name: name.into(),
                color: color.into(),
                is_ai: true,
            }
        });
    let map = rts_sim::game::map::Map::load_for_players("Classic", &[(1, 1), (2, 2)], 7).unwrap();
    let mut game = Game::new_with_random_ai_profiles_and_map_metadata(
        &players,
        7,
        map,
        rts_sim::game::map::Map::metadata_for_name("Classic").unwrap(),
    );
    let mut controllers = [
        AiController::with_profile_id(1, "eco_ai"),
        AiController::with_profile_id(2, "jeffs_ai_v1_1"),
    ];
    for _ in 0..4050 {
        CanonicalAiTickDriver::run(
            &mut game,
            &mut controllers,
            AiAlivePolicy::StartingPrimaryBase,
        );
        game.tick();
    }
    let restarted = game
        .command_log()
        .iter()
        .filter(|e| e.player_id == 1)
        .any(|e| {
            matches!(&e.command,
        rts_sim::protocol::Command::Build {building,..} if building=="resource_depot")
        });
    assert!(restarted, "queue the expansion by 2:15");
    let s = game.snapshot_for(1);
    let depots = s
        .entities
        .iter()
        .filter(|e| e.owner == 1 && e.kind == "resource_depot")
        .count();
    let worker_building = s.entities.iter().any(|e| {
        e.owner == 1
            && e.kind == "worker"
            && e.state == "build"
            && e.order_plan.iter().any(|order| {
                order.kind == "build" && order.building_kind.as_deref() == Some("resource_depot")
            })
    });
    assert!(
        s.entities
            .iter()
            .filter(|e| e.owner == 1 && e.kind == "resource_depot" && e.build_progress.is_none())
            .count()
            >= 2,
        "the expansion must be complete by 2:15, without a garrison obstructing its footprint"
    );
    eprintln!(
        "Eco 2:15: bank={}/{} depots={depots} worker_building={worker_building}",
        s.steel, s.oil
    );
}

fn works_target_for_test(bases: usize) -> usize {
    match bases {
        0 | 1 => 0,
        2 => 1,
        3 => 2,
        _ => 3,
    }
}
