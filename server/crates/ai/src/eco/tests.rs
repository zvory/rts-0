use super::*;
use crate::sdk::actions::AiActionRequest;
use crate::sdk::{AiBaseSite, AiBuildObservation, AiBuildObservationPhase};
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::{Snapshot, StartPayload};

pub(super) fn fixture() -> (StartPayload, Snapshot, Vec<AiBaseSite>) {
    let players = [1, 2].map(|id| PlayerInit {
        id,
        team_id: id,
        faction_id: "kriegsia".into(),
        name: format!("P{id}"),
        color: "#4cc9f0".into(),
        is_ai: true,
    });
    let game = Game::new_without_ai_controllers(&players, 7);
    let start = game.start_payload();
    let mut s = game.snapshot_for(1);
    s.steel = 10000;
    s.oil = 10000;
    s.supply_cap = 200;
    let mut cache = crate::ai_core::map_analysis::AiStaticMapContextCache::default();
    let sites = cache.get_or_analyze(&start).base_sites().to_vec();
    (start, s, sites)
}
pub(super) fn frame(start: &StartPayload, s: &Snapshot, sites: &[AiBaseSite]) -> AiFrame {
    AiFrame::from_host(start, s, 1, [], Some(&[1, 2]))
        .unwrap()
        .with_base_sites(sites)
}
pub(super) fn building(
    s: &mut Snapshot,
    id: u32,
    kind: &str,
    p: (f32, f32),
    progress: Option<f32>,
) {
    let mut e = s
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "resource_depot")
        .unwrap()
        .clone();
    e.id = id;
    e.kind = kind.into();
    e.x = p.0;
    e.y = p.1;
    e.prod_kind = None;
    e.prod_upgrade = None;
    e.prod_queue = Some(0);
    e.build_progress = progress;
    e.build_active = progress.is_some();
    e.state = if progress.is_some() {
        "construct"
    } else {
        "idle"
    }
    .into();
    s.entities.push(e);
}
/// Fixtures exercising expansion/combat need infantry in addition to the new
/// four-per-base observation floor. Station those guards on the real SDK posts.
pub(super) fn add_observation_garrisons(
    start: &StartPayload,
    s: &mut Snapshot,
    sites: &[AiBaseSite],
) {
    let f = frame(start, s, sites);
    let owned = owned_sites(&f);
    let plan = DefensePlanner::with_garrison_posts_per_base(4).plan(&f, &owned, owned.len() * 4);
    let template = s
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "resource_depot")
        .unwrap()
        .clone();
    for (i, slot) in plan.slots.iter().enumerate() {
        let mut e = template.clone();
        e.id = 20000 + i as u32;
        e.kind = "rifleman".into();
        e.x = slot.position.0;
        e.y = slot.position.1;
        e.hp = 100;
        e.max_hp = 100;
        e.state = "idle".into();
        s.entities.push(e);
    }
}
#[test]
fn incomplete_depots_do_not_unlock_research_or_capacity() {
    let (start, mut s, sites) = fixture();
    let second = sites
        .iter()
        .find(|b| matches!(b.id, AiBaseSiteId::ResourceCluster(_)))
        .unwrap();
    building(&mut s, 800, "resource_depot", second.position, Some(0.5));
    let f = frame(&start, &s, &sites);
    assert_eq!(owned_sites(&f).len(), 1);
    assert_eq!(barracks_target(1), 1);
    assert_eq!(barracks_target(2), 2);
    assert_eq!(barracks_target(3), 3);
    s.upgrades = vec!["entrenchment".into()];
    let f = frame(&start, &s, &sites);
    let mut actions = AiActions::for_frame(&f);
    production::EcoProduction::default().step(&f, &mut actions);
    assert!(!actions.into_requests().iter().any(|r| matches!(
        r,
        AiActionRequest::Research {
            upgrade: UpgradeKind::Panzerfausts | UpgradeKind::Methamphetamines,
            ..
        }
    )));
}
#[test]
fn first_base_cannot_skip_entrenchment_even_with_rich_bank() {
    let (start, mut s, sites) = fixture();
    let home = s
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "resource_depot")
        .map(|e| (e.x, e.y))
        .unwrap();
    building(&mut s, 801, "barracks", (home.0 + 128., home.1), None);
    building(
        &mut s,
        802,
        "training_centre",
        (home.0 + 256., home.1),
        None,
    );
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    EcoStrategy::default().step(&f, &mut a);
    let requests = a.into_requests();
    assert!(requests.iter().any(|r| matches!(
        r,
        AiActionRequest::Research {
            upgrade: UpgradeKind::Entrenchment,
            ..
        }
    )));
    assert!(!requests.iter().any(|r| matches!(
        r,
        AiActionRequest::Build {
            building: EntityKind::ResourceDepot,
            ..
        }
    )));
}
#[test]
fn expansion_filters_enemy_starts_unreachable_and_known_occupied() {
    let (start, mut s, mut sites) = fixture();
    let home = sites
        .iter()
        .find(|b| b.id == AiBaseSiteId::Start(1))
        .unwrap()
        .position;
    sites.push(AiBaseSite {
        id: AiBaseSiteId::ResourceCluster(900),
        position: (home.0 + 64., home.1 + 64.),
        resource_ids: vec![],
        ground_distances_from_starts: Default::default(),
    });
    let f = frame(&start, &s, &sites);
    let target = expansion(&f).unwrap();
    assert!(!matches!(target.0,AiBaseSiteId::Start(id) if id!=1));
    assert_ne!(target.0, AiBaseSiteId::ResourceCluster(900));
    building(&mut s, 800, "resource_depot", target.1, Some(0.3));
    assert_ne!(
        expansion(&frame(&start, &s, &sites)).map(|b| b.0),
        Some(target.0)
    );
}
#[test]
fn builder_recovery_pays_for_one_worker_without_changing_automatic_extraction() {
    let (start, mut s, sites) = fixture();
    let f = frame(&start, &s, &sites);
    let mut eco = EcoStrategy::default();
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    s.entities.retain(|e| e.owner != 1 || e.kind != "worker");
    for d in s
        .entities
        .iter_mut()
        .filter(|e| e.owner == 1 && e.kind == "resource_depot")
    {
        d.prod_kind = None;
        d.prod_queue = Some(0);
    }
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    let requests = a.into_requests();
    assert!(!requests
        .iter()
        .any(|r| matches!(r, AiActionRequest::AdjustProductionRepeat { .. })));
    assert_eq!(
        requests
            .iter()
            .filter(|r| matches!(
                r,
                AiActionRequest::Train {
                    unit: EntityKind::Worker,
                    ..
                }
            ))
            .count(),
        1
    );
    // Automatic extractor jobs do not occupy the manual queue. An actual manual Worker
    // already in that queue does: never enqueue a duplicate replacement behind it.
    for d in s
        .entities
        .iter_mut()
        .filter(|e| e.owner == 1 && e.kind == "resource_depot")
    {
        d.prod_kind = Some("worker".into());
        d.prod_queue = Some(1);
    }
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    assert!(!a
        .into_requests()
        .iter()
        .any(|r| matches!(r, AiActionRequest::Train { .. })));
}

#[test]
fn automatic_extractor_scaffolds_do_not_capture_the_builder() {
    for kind in ["steel_mine", "pump_jack"] {
        let (start, mut s, sites) = fixture();
        let home = sites
            .iter()
            .find(|b| b.id == AiBaseSiteId::Start(1))
            .unwrap()
            .position;
        building(&mut s, 900, kind, (home.0 + 128., home.1), Some(0.2));
        s.entities
            .iter_mut()
            .find(|e| e.id == 900)
            .unwrap()
            .build_active = false;
        let f = frame(&start, &s, &sites);
        let mut a = AiActions::for_frame(&f);
        EcoStrategy::default().step(&f, &mut a);
        assert!(a.into_requests().iter().any(|r| matches!(
            r,
            AiActionRequest::Build {
                building: EntityKind::Barracks,
                ..
            }
        )));
    }
}

#[test]
fn fixed_supply_cap_never_builds_useless_depots_and_scaffolds_resume() {
    let (start, mut s, sites) = fixture();
    s.supply_cap = s.supply_used;
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    EcoStrategy::default().step(&f, &mut a);
    assert!(a.into_requests().iter().any(|r| matches!(
        r,
        AiActionRequest::Build {
            building: EntityKind::Barracks,
            ..
        }
    )));
    s.supply_cap = 200;
    let home = sites
        .iter()
        .find(|b| b.id == AiBaseSiteId::Start(1))
        .unwrap()
        .position;
    building(&mut s, 901, "barracks", (home.0 + 128., home.1), Some(0.4));
    s.entities
        .iter_mut()
        .find(|e| e.id == 901)
        .unwrap()
        .build_active = false;
    let mut eco = EcoStrategy::default();
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    assert!(!a
        .into_requests()
        .iter()
        .any(|r| matches!(r, AiActionRequest::Build { .. })));
    s.tick += 90;
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    let before = a.remaining_budget();
    eco.step(&f, &mut a);
    assert_eq!(before, a.remaining_budget());
    assert!(a.into_requests().iter().any(|r| matches!(
        r,
        AiActionRequest::Build {
            building: EntityKind::Barracks,
            ..
        }
    )));
}

#[test]
fn target_base_busy_vehicle_works_add_capacity_and_remember_destroyed_overflow() {
    let (start, mut s, sites) = fixture();
    let home = sites
        .iter()
        .find(|b| b.id == AiBaseSiteId::Start(1))
        .unwrap()
        .position;
    let candidates: Vec<_> = sites
        .iter()
        .filter(|b| matches!(b.id, AiBaseSiteId::ResourceCluster(_)))
        .take(6)
        .collect();
    assert_eq!(candidates.len(), 6);
    for (i, b) in candidates.iter().enumerate() {
        building(&mut s, 800 + i as u32, "resource_depot", b.position, None);
    }
    for i in 0..4 {
        building(
            &mut s,
            850 + i,
            "barracks",
            (home.0 + 128. + i as f32 * 128., home.1),
            None,
        );
    }
    building(
        &mut s,
        860,
        "training_centre",
        (home.0, home.1 + 128.),
        None,
    );
    for e in s.entities.iter_mut().filter(|e| (850..854).contains(&e.id)) {
        e.prod_kind = Some("machine_gunner".into());
        e.prod_queue = Some(1);
    }
    s.upgrades = vec![
        "entrenchment".into(),
        "panzerfausts".into(),
        "methamphetamines".into(),
    ];
    s.upgrades.push("tank_unlock".into());
    building(
        &mut s,
        870,
        "engineering_complex",
        (home.0, home.1 + 256.),
        None,
    );
    for i in 0..3 {
        building(
            &mut s,
            880 + i,
            "factory",
            (home.0 + 256. + i as f32 * 256., home.1 + 256.),
            None,
        );
        let e = s.entities.iter_mut().find(|e| e.id == 880 + i).unwrap();
        e.prod_kind = Some("tank".into());
        e.prod_queue = Some(1);
    }
    let f = frame(&start, &s, &sites);
    assert_eq!(owned_sites(&f).len(), 7);
    let mut eco = EcoStrategy::default();
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    assert_eq!(eco.overflow_works, 4);
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    assert!(a.into_requests().iter().any(|r| matches!(
        r,
        AiActionRequest::Build {
            building: EntityKind::Factory,
            ..
        }
    )));
    s.entities.retain(|e| e.id != 880);
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    assert_eq!(eco.overflow_works, 4);
    assert!(a.into_requests().iter().any(|r| matches!(
        r,
        AiActionRequest::Build {
            building: EntityKind::Factory,
            ..
        }
    )));
}

#[test]
fn second_base_completes_both_required_upgrades_before_third_expansion() {
    let (start, mut s, sites) = fixture();
    let home = sites
        .iter()
        .find(|b| b.id == AiBaseSiteId::Start(1))
        .unwrap()
        .position;
    let second = sites
        .iter()
        .find(|b| matches!(b.id, AiBaseSiteId::ResourceCluster(_)))
        .unwrap();
    building(&mut s, 800, "resource_depot", second.position, None);
    building(&mut s, 801, "barracks", (home.0 + 256., home.1), None);
    building(&mut s, 802, "barracks", (home.0 + 384., home.1), None);
    building(
        &mut s,
        803,
        "training_centre",
        (home.0, home.1 + 128.),
        None,
    );
    building(
        &mut s,
        804,
        "engineering_complex",
        (home.0, home.1 + 256.),
        None,
    );
    building(&mut s, 805, "factory", (home.0 + 512., home.1 + 256.), None);
    add_observation_garrisons(&start, &mut s, &sites);
    for upgrades in [vec!["entrenchment"], vec!["entrenchment", "panzerfausts"]] {
        s.upgrades = upgrades.into_iter().map(str::to_string).collect();
        let f = frame(&start, &s, &sites);
        assert_eq!(owned_sites(&f).len(), 2);
        let mut a = AiActions::for_frame(&f);
        EcoStrategy::default().step(&f, &mut a);
        assert!(!a.into_requests().iter().any(|r| matches!(
            r,
            AiActionRequest::Build {
                building: EntityKind::ResourceDepot,
                ..
            }
        )));
    }
    s.upgrades = vec![
        "entrenchment".into(),
        "panzerfausts".into(),
        "methamphetamines".into(),
    ];
    let mut eco = EcoStrategy::default();
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    assert!(!a.into_requests().iter().any(|r| matches!(
        r,
        AiActionRequest::Build {
            building: EntityKind::ResourceDepot,
            ..
        }
    )));
    s.tick += 150;
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    assert!(a.into_requests().iter().any(|r| matches!(
        r,
        AiActionRequest::Build {
            building: EntityKind::ResourceDepot,
            ..
        }
    )));
}

/// Opening contract: use the real ordinary-income start on either side of Classic.
/// Eventual four-base progression alone cannot catch an Engineer wasting the first minute.
#[test]
fn eco_classic_opening_builds_promptly_without_resuming_automatic_extractors() {
    use crate::live::{
        AiAlivePolicy, AiController, AiControllerTickInvocation, CanonicalAiTickDriver,
    };
    use rts_sim::game::command::SimCommand;
    for eco_player in [1, 2] {
        let players = [1, 2].map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: "kriegsia".into(),
            name: format!("P{id}"),
            color: "#4cc9f0".into(),
            is_ai: true,
        });
        let map =
            rts_sim::game::map::Map::load_for_players("Classic", &[(1, 1), (2, 2)], 7).unwrap();
        let mut game = Game::new_with_random_ai_profiles_and_map_metadata(
            &players,
            7,
            map,
            rts_sim::game::map::Map::metadata_for_name("Classic").unwrap(),
        );
        let mut controllers = [AiController::with_profile_id(eco_player, "eco_ai")];
        let mut first_barracks = None;
        let mut complete_barracks = None;
        let mut infantry_ordered = false;
        let mut underfunded_travel = false;
        for _ in 0..1800 {
            let report = CanonicalAiTickDriver::run_with_report(
                &mut game,
                &mut controllers,
                AiAlivePolicy::StartingPrimaryBase,
            );
            for controller in report.controllers {
                if let AiControllerTickInvocation::Invoked {
                    emitted_commands,
                    snapshot,
                    ..
                } = controller.invocation
                {
                    for command in emitted_commands {
                        match command {
                            SimCommand::Build { building, .. } => {
                                assert!(
                                    !building.is_resource_extractor(),
                                    "Eco must not commandeer automatic extractors"
                                );
                                if building == rts_sim::game::entity::EntityKind::Barracks {
                                    if first_barracks.is_none() {
                                        let cost = rts_rules::economy::cost(EntityKind::Barracks);
                                        assert!(
                                            snapshot.steel < cost.0,
                                            "opening should dispatch before full funding"
                                        );
                                        eprintln!(
                                            "Eco P{eco_player} dispatch bank={}/{}",
                                            snapshot.steel, snapshot.oil
                                        );
                                    }
                                    first_barracks.get_or_insert(game.current_tick());
                                }
                            }
                            SimCommand::Move { .. } | SimCommand::HoldPosition { .. } => {
                                infantry_ordered = true
                            }
                            _ => {}
                        }
                    }
                }
            }
            let events = game.tick();
            let snapshot = game.snapshot_for(eco_player);
            if first_barracks == Some(game.current_tick().saturating_sub(1)) {
                eprintln!(
                    "First build response: workers={:?}, events={:?}",
                    snapshot
                        .entities
                        .iter()
                        .filter(|e| e.owner == eco_player && e.kind == "worker")
                        .map(|e| (&e.state, e.x, e.y))
                        .collect::<Vec<_>>(),
                    events.iter().find(|(player, _)| *player == eco_player)
                );
            }
            if first_barracks.is_some()
                && snapshot.steel < rts_rules::economy::cost(EntityKind::Barracks).0
                && snapshot
                    .entities
                    .iter()
                    .any(|e| e.owner == eco_player && e.kind == "worker" && e.state == "build")
                && !snapshot
                    .entities
                    .iter()
                    .any(|e| e.owner == eco_player && e.kind == "barracks")
            {
                underfunded_travel = true;
            }
            if game.snapshot_for(eco_player).entities.iter().any(|e| {
                e.owner == eco_player && e.kind == "barracks" && e.build_progress.is_none()
            }) {
                complete_barracks.get_or_insert(game.current_tick());
            }
        }
        assert!(
            first_barracks.is_some_and(|tick| tick <= 360),
            "P{eco_player}: Barracks request {first_barracks:?} must be by 12s"
        );
        assert!(underfunded_travel,"simulation must actually send the Engineer before full funding; a speculative placement request alone is insufficient");
        assert!(
            complete_barracks.is_some(),
            "P{eco_player}: Barracks must finish by 60s"
        );
        assert!(
            infantry_ordered,
            "starting Riflemen must receive defensive orders"
        );
        eprintln!("Eco P{eco_player}: Barracks requested tick {first_barracks:?}, completed {complete_barracks:?}");
    }
}

#[test]
fn recovery_waits_for_travel_and_avoids_threatened_scaffolds() {
    let (start, mut s, sites) = fixture();
    let home = sites
        .iter()
        .find(|b| b.id == AiBaseSiteId::Start(1))
        .unwrap()
        .position;
    building(&mut s, 901, "barracks", (home.0 + 128., home.1), Some(0.4));
    s.entities
        .iter_mut()
        .find(|e| e.id == 901)
        .unwrap()
        .build_active = false;
    let mut eco = EcoStrategy::default();
    let f = frame(&start, &s, &sites);
    eco.step(&f, &mut AiActions::for_frame(&f));
    s.tick += 90;
    let mut enemy = s
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "rifleman")
        .unwrap()
        .clone();
    enemy.id = 902;
    enemy.owner = 2;
    enemy.x = home.0 + 128.;
    enemy.y = home.1;
    s.entities.push(enemy);
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    assert!(!a
        .into_requests()
        .iter()
        .any(|r| matches!(r, AiActionRequest::Build { .. })));
    s.entities.retain(|e| e.id != 902);
    let worker = s
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "worker")
        .unwrap()
        .id;
    let foot = AiRulebook::new("kriegsia")
        .unwrap()
        .entity(EntityKind::Barracks)
        .unwrap()
        .footprint
        .unwrap();
    let pending = AiBuildObservation {
        worker_id: worker,
        kind: EntityKind::Barracks,
        tile_x: ((home.0 + 128.) / start.map.tile_size as f32 - foot.width_tiles as f32 / 2.)
            .round() as u32,
        tile_y: (home.1 / start.map.tile_size as f32 - foot.height_tiles as f32 / 2.).round()
            as u32,
        phase: AiBuildObservationPhase::TravelingToSite,
    };
    let f = AiFrame::from_host(&start, &s, 1, [pending], Some(&[1, 2]))
        .unwrap()
        .with_base_sites(&sites);
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    assert!(!a
        .into_requests()
        .iter()
        .any(|r| matches!(r, AiActionRequest::Build { .. })));
}

#[test]
fn assigned_expansion_screen_advances_before_engineer_while_rear_posts_remain() {
    let (start, mut s, sites) = fixture();
    let home = sites
        .iter()
        .find(|b| b.id == AiBaseSiteId::Start(1))
        .unwrap()
        .position;
    building(&mut s, 801, "barracks", (home.0 + 256., home.1), None);
    building(
        &mut s,
        802,
        "training_centre",
        (home.0, home.1 + 128.),
        None,
    );
    s.upgrades = vec!["entrenchment".into()];
    let rifle = s
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "rifleman")
        .unwrap()
        .clone();
    for (id, kind) in [(803, "machine_gunner"), (804, "panzerfaust")] {
        let mut e = rifle.clone();
        e.id = id;
        e.kind = kind.into();
        s.entities.push(e);
    }
    let ids: BTreeMap<_, _> = s
        .entities
        .iter_mut()
        .filter(|e| {
            e.owner == 1
                && matches!(
                    e.kind.as_str(),
                    "rifleman" | "machine_gunner" | "panzerfaust"
                )
        })
        .map(|e| {
            e.state = "attack".into();
            (e.id, ())
        })
        .collect();
    let mut eco = EcoStrategy::default();
    let f = frame(&start, &s, &sites);
    let prospective = expansion(&f).unwrap().0;
    let depot_tile = WorldQueries::new(&f)
        .base_depot_site(prospective, &Default::default())
        .unwrap();
    let line = DefensePlanner::with_firing_range_tiles(4.8).plan_with_expansion(
        &f,
        &owned_sites(&f),
        prospective,
        ids.len(),
        &[crate::sdk::DefenseReservation {
            building: EntityKind::ResourceDepot,
            tile: depot_tile,
        }],
    );
    assert!(
        !line.slots.is_empty(),
        "vacant public expansion must have prospective defense slots"
    );
    assert_eq!(
        owned_sites(&f).len(),
        1,
        "prospective planning must not invent Depot ownership"
    );
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    let requests = a.into_requests();

    let tile = WorldQueries::new(&f)
        .base_depot_site(prospective, &KnownBuildSiteExclusions::default())
        .unwrap();
    let foot = AiRulebook::for_frame(&f)
        .unwrap()
        .entity(EntityKind::ResourceDepot)
        .unwrap()
        .footprint
        .unwrap();
    let ts = f.map().tile_size as f32;
    let rect = (
        tile.x() as f32 * ts,
        tile.y() as f32 * ts,
        (tile.x() + foot.width_tiles) as f32 * ts,
        (tile.y() + foot.height_tiles) as f32 * ts,
    );
    for order in &requests {
        if let AiActionRequest::AttackMove { units, x, y, .. } = order {
            for id in units.iter().filter(|id| ids.contains_key(id)) {
                let kind = f.owned().iter().find(|e| e.id == *id).unwrap().kind;
                assert!(
                    !crate::sdk::unit_circle_touches_rect(
                        (*x, *y),
                        crate::sdk::unit_placement_radius(kind) + 0.25 * ts,
                        rect
                    ),
                    "infantry {id} must leave the future Depot footprint clear"
                );
            }
        }
    }
    assert!(!eco.defense.screen_ids().is_empty());
    for id in eco.defense.screen_ids() {
        assert!(
            requests.iter().any(|r| matches!(r,
                AiActionRequest::AttackMove { units,x,y,.. } if units.contains(id)
                    && eco.defense.assigned_posts().get(id).is_some_and(|point| construction::dist2(*point,(*x,*y))<1.)
            )),
            "infantry {id} must attack-move to the future line even while fighting"
        );
    }
    assert!(!requests.iter().any(|r| matches!(
        r,
        AiActionRequest::Build {
            building: EntityKind::ResourceDepot,
            ..
        }
    )));
    // Reinforcements join the advance without postponing the initial cover wave.
    s.tick += 60;
    let mut reinforce = rifle;
    reinforce.id = 805;
    s.entities.push(reinforce);
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    assert!(a
        .into_requests()
        .iter()
        .any(|r| matches!(r,AiActionRequest::AttackMove { units,.. } if units.contains(&805))));
    s.tick += 30;
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    assert!(!a.into_requests().iter().any(|r| matches!(
        r,
        AiActionRequest::Build {
            building: EntityKind::ResourceDepot,
            ..
        }
    )));
    s.tick += 30;
    let mut latest = s.entities.iter().find(|e| e.id == 805).unwrap().clone();
    latest.id = 806;
    s.entities.push(latest);
    let f = frame(&start, &s, &sites);
    eco.step(&f, &mut AiActions::for_frame(&f));
    s.tick += 30;
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    assert!(a.into_requests().iter().any(|r| matches!(
        r, AiActionRequest::Build {building:EntityKind::ResourceDepot,..}
    )),"fresh reinforcements every two seconds must not keep restarting the original five-second lead");
}

#[test]
fn pressure_response_mobilizes_mgs_keeps_ownership_and_rejoins_expansion_screen() {
    let (start, mut s, sites) = fixture();
    let rifle = s
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "rifleman")
        .unwrap()
        .clone();
    s.entities
        .retain(|e| !(e.owner == 1 && e.kind == "rifleman"));
    for id in 1000..1008 {
        let mut own = rifle.clone();
        own.id = id;
        own.kind = "machine_gunner".into();
        own.x = rifle.x + (id - 1000) as f32 * 32.;
        s.entities.push(own);
    }
    let mut enemy = rifle.clone();
    enemy.id = 999;
    enemy.owner = 2;
    enemy.kind = "machine_gunner".into();
    enemy.x = rifle.x + 64.;
    s.entities.push(enemy.clone());
    let f = frame(&start, &s, &sites);
    let site = expansion(&f).unwrap().0;
    let mut defense = tactics::DefenseAssignments::default();
    defense.prepare_expansion(site);
    let mut planner = DefensePlanner::with_firing_range_tiles(4.8);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned_sites(&f));
    let orders = a.into_requests();
    let response_ids: std::collections::BTreeSet<_> = orders
        .iter()
        .filter_map(|r| match r {
            AiActionRequest::AttackMove { units, x, y, .. } if (*x, *y) == (enemy.x, enemy.y) => {
                Some(units.iter().copied())
            }
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(
        response_ids.len(),
        2,
        "one enemy MG requires two whole MGs for at least 150% value"
    );
    assert!(
        !defense.ready_for_builder(&f, site),
        "the Engineer must not leave while its screen is fighting"
    );
    assert!(
        !defense.waiting_for_builder(&f, site),
        "enemy response must not pause reinforcement training"
    );
    for e in s
        .entities
        .iter_mut()
        .filter(|e| response_ids.contains(&e.id))
    {
        e.state = "move".into();
    }
    s.tick += 9;
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned_sites(&f));
    assert!(
        !a.into_requests().iter().any(|r| match r {
            AiActionRequest::Move { units, .. }
            | AiActionRequest::AttackMove { units, .. }
            | AiActionRequest::HoldPosition { units, .. } =>
                units.iter().any(|id| response_ids.contains(id)),
            _ => false,
        }),
        "cooldown decisions must not let frontier orders steal responders"
    );
    enemy.id = 998;
    s.entities.push(enemy.clone());
    s.tick += 9;
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned_sites(&f));
    let new_ids: std::collections::BTreeSet<_> = a
        .into_requests()
        .iter()
        .filter_map(|r| match r {
            AiActionRequest::AttackMove { units, x, y, .. } if (*x, *y) == (enemy.x, enemy.y) => {
                Some(units.iter().copied())
            }
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(
        new_ids.len(),
        1,
        "a second enemy MG requires recruiting a third responder"
    );
    let lost = *response_ids.first().unwrap();
    s.entities.retain(|e| e.id != lost);
    s.tick += 9;
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned_sites(&f));
    let replacement = a
        .into_requests()
        .iter()
        .filter_map(|r| match r {
            AiActionRequest::AttackMove { units, x, y, .. } if (*x, *y) == (enemy.x, enemy.y) => {
                Some(units.clone())
            }
            _ => None,
        })
        .flatten()
        .collect::<Vec<_>>();
    assert!(
        replacement
            .iter()
            .any(|id| !response_ids.contains(id) && !new_ids.contains(id)),
        "replace a dead responder"
    );
    let last_seen = s.tick;
    s.entities.retain(|e| e.owner != 2);
    s.visible_tiles.fill(0);
    s.tick = last_seen + 899;
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned_sites(&f));
    assert!(
        defense.has_responders(),
        "unseen contacts retain Pressure's bounded response memory without a global builder lock"
    );
    s.tick = last_seen + 900;
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned_sites(&f));
    let released = a.into_requests();
    for id in response_ids
        .iter()
        .chain(&new_ids)
        .chain(&replacement)
        .filter(|id| **id != lost)
    {
        assert!(
            released
                .iter()
                .any(|r| matches!(r,AiActionRequest::AttackMove {units,..} if units.contains(id))),
            "released responder {id} must receive a fresh frontier attack-move"
        );
    }
    // New screen posts can also be assigned to non-responders; each receives a fresh lead.
    s.tick += SCREEN_LEAD_TICKS;
    let f = frame(&start, &s, &sites);
    assert!(
        defense.ready_for_builder(&f, site),
        "regrouped infantry retain the full Engineer lead"
    );
}

#[test]
fn cleared_visible_contact_releases_screen_without_thirty_second_expansion_delay() {
    let (start, mut s, sites) = fixture();
    add_observation_garrisons(&start, &mut s, &sites);
    s.visible_tiles = vec![1; (start.map.width * start.map.height) as usize];
    let mut enemy = s
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "rifleman")
        .unwrap()
        .clone();
    enemy.id = 999;
    enemy.owner = 2;
    let qframe = frame(&start, &s, &sites);
    let size = qframe.map().tile_size as f32;
    let point = (0..qframe.map().height)
        .flat_map(|y| (0..qframe.map().width).map(move |x| (x, y)))
        .filter(|(x, y)| !qframe.map().concealment_tiles.contains(&(*x, *y)))
        .map(|(x, y)| ((x as f32 + 0.5) * size, (y as f32 + 0.5) * size))
        .min_by(|a, b| {
            construction::dist2(*a, (enemy.x, enemy.y))
                .total_cmp(&construction::dist2(*b, (enemy.x, enemy.y)))
        })
        .unwrap();
    enemy.x = point.0;
    enemy.y = point.1;
    s.entities.push(enemy);
    let f = frame(&start, &s, &sites);
    let site = expansion(&f).unwrap().0;
    let mut defense = tactics::DefenseAssignments::default();
    defense.prepare_expansion(site);
    let mut planner = DefensePlanner::with_firing_range_tiles(4.8);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned_sites(&f));
    assert!(!defense.ready_for_builder(&f, site));
    s.entities.retain(|e| e.id != 999);
    s.tick += 9;
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned_sites(&f));
    assert!(
        defense.waiting_for_builder(&f, site),
        "cleared contacts must release the response and begin the regroup immediately"
    );
    s.tick += SCREEN_LEAD_TICKS;
    let f = frame(&start, &s, &sites);
    assert!(
        defense.ready_for_builder(&f, site),
        "no phantom 30-second response may hold the Engineer"
    );
}

#[test]
fn contact_clearance_requires_visible_open_ground_without_smoke() {
    let (mut start, mut s, sites) = fixture();
    start
        .map
        .concealment_tiles
        .push(rts_sim::protocol::MapTile { x: 1, y: 1 });
    let size = start.map.tile_size as f32;
    let f = frame(&start, &s, &sites);
    let open = (0..f.map().height)
        .flat_map(|y| (0..f.map().width).map(move |x| (x, y)))
        .find(|tile| !f.map().concealment_tiles.contains(tile))
        .unwrap();
    let point = ((open.0 as f32 + 0.5) * size, (open.1 as f32 + 0.5) * size);
    let observable = |snapshot: &Snapshot, position: (f32, f32)| {
        let f = frame(&start, snapshot, &sites);
        let q = WorldQueries::new(&f);
        q.ground_position_is_observable(q.world_point(position.0, position.1).unwrap())
    };
    s.visible_tiles.clear();
    assert!(!observable(&s, point), "missing grid is uncertain");
    s.visible_tiles = vec![0; (start.map.width * start.map.height) as usize];
    assert!(!observable(&s, point), "fog retains the contact");
    s.visible_tiles.fill(1);
    assert!(observable(&s, point));
    s.smokes.push(rts_sim::protocol::SmokeCloudView {
        id: 9876,
        x: point.0,
        y: point.1,
        radius_tiles: 2.,
        expires_in: 100,
    });
    assert!(!observable(&s, point), "smoke retains the contact");
    s.smokes.clear();
    let forest = *f
        .map()
        .concealment_tiles
        .iter()
        .next()
        .expect("fixture has forest");
    assert!(
        !observable(
            &s,
            (
                (forest.0 as f32 + 0.5) * size,
                (forest.1 as f32 + 0.5) * size
            )
        ),
        "forest retains the contact"
    );
}

#[test]
fn engineer_uses_its_route_instead_of_waiting_for_every_combat_contact() {
    let (start, mut s, sites) = fixture();
    add_observation_garrisons(&start, &mut s, &sites);
    s.visible_tiles.fill(0);
    let f = frame(&start, &s, &sites);
    let site = expansion(&f).unwrap().0;
    let mut defense = tactics::DefenseAssignments::default();
    defense.prepare_expansion(site);
    let mut planner = DefensePlanner::with_firing_range_tiles(4.8);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned_sites(&f));
    s.tick += SCREEN_LEAD_TICKS;
    let f = frame(&start, &s, &sites);
    assert!(defense.ready_for_builder(&f, site));
    let q = WorldQueries::new(&f);
    let worker = f
        .owned()
        .iter()
        .find(|e| e.kind == EntityKind::Worker)
        .unwrap();
    let route = q
        .ground_route_to_build_site(
            q.world_point(worker.position.0, worker.position.1).unwrap(),
            EntityKind::ResourceDepot,
            defense.depot_tile().unwrap(),
            EntityKind::Worker,
        )
        .unwrap();
    let mid = route[route.len() / 2];
    let mut enemy = s
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "rifleman")
        .unwrap()
        .clone();
    enemy.target_id = Some(enemy.id); // a known assigned guard, even away from its post
    enemy.id = 999;
    enemy.owner = 2;
    enemy.x = mid.x();
    enemy.y = mid.y();
    s.entities.push(enemy);
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned_sites(&f));
    assert!(defense.has_responders());
    assert!(
        !defense.ready_for_builder(&f, site),
        "a visible enemy on the route must delay dispatch"
    );
    // Wait out the new screen assignments with the visible enemy still on the route.
    s.tick += SCREEN_LEAD_TICKS;
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned_sites(&f));
    assert!(
        !defense.ready_for_builder(&f, site),
        "the visible threat blocks dispatch after the lead"
    );
    s.entities.retain(|e| e.id != 999);
    s.tick += 9;
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned_sites(&f));
    assert!(defense.has_responders(), "fog retains the tactical contact");
    assert!(
        defense.ready_for_builder(&f, site),
        "contact memory alone cannot hold a clear Engineer route"
    );
}

#[test]
fn retry_footprint_is_shared_by_defense_and_the_engineer() {
    let (start, mut s, sites) = fixture();
    add_observation_garrisons(&start, &mut s, &sites);
    let f = frame(&start, &s, &sites);
    let q = WorldQueries::new(&f);
    let site = expansion(&f).unwrap().0;
    let original = q.base_depot_site(site, &Default::default()).unwrap();
    let mut eco = EcoStrategy::default();
    eco.exclusions.exclude(EntityKind::ResourceDepot, original);
    eco.defense.prepare_expansion(site);
    let rules = AiRulebook::for_frame(&f).unwrap();
    eco.refresh_depot_reservation(&f, rules);
    let actual = eco.defense.depot_tile().unwrap();
    assert_ne!(original, actual);
    let mut a = AiActions::for_frame(&f);
    eco.defense
        .step(&f, &mut a, &mut eco.planner, &owned_sites(&f));
    let foot = rules
        .entity(EntityKind::ResourceDepot)
        .unwrap()
        .footprint
        .unwrap();
    let ts = f.map().tile_size as f32;
    let rect = (
        actual.x() as f32 * ts,
        actual.y() as f32 * ts,
        (actual.x() + foot.width_tiles) as f32 * ts,
        (actual.y() + foot.height_tiles) as f32 * ts,
    );
    let mut destinations = Vec::new();
    for request in a.into_requests() {
        if let AiActionRequest::AttackMove { x, y, .. } = request {
            assert!(!crate::sdk::unit_circle_touches_rect(
                (x, y),
                crate::sdk::unit_placement_radius(EntityKind::MachineGunner) + ts,
                rect
            ));
            assert!(!destinations.contains(&(x, y)));
            destinations.push((x, y));
        }
    }
    s.tick += SCREEN_LEAD_TICKS;
    let f = frame(&start, &s, &sites);
    assert!(eco.defense.ready_for_builder(&f, site));
    let mut a = AiActions::for_frame(&f);
    assert!(eco.construct(
        &f,
        &mut a,
        rules,
        Some((EntityKind::ResourceDepot, expansion(&f).unwrap().1))
    ));
    assert!(a.into_requests().iter().any(|r|matches!(r,AiActionRequest::Build {building:EntityKind::ResourceDepot,tile_x,tile_y,..} if (*tile_x,*tile_y)==actual.as_tuple())));
}

#[path = "tests/tanks.rs"]
mod tanks;

mod armor;

mod micro;

mod sentries;
