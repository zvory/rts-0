use super::*;

pub(super) fn staged(bases: usize) -> (StartPayload, Snapshot, Vec<AiBaseSite>) {
    let (start, mut s, sites) = fixture();
    let f = frame(&start, &s, &sites);
    let extra = WorldQueries::new(&f)
        .ranked_bases(1)
        .into_iter()
        .filter(|b| !matches!(b.site.id, AiBaseSiteId::Start(_)))
        .take(bases.saturating_sub(1))
        .map(|b| b.site.position)
        .collect::<Vec<_>>();
    for (i, point) in extra.into_iter().enumerate() {
        building(&mut s, 800 + i as u32, "resource_depot", point, None);
    }
    let home = sites
        .iter()
        .find(|b| b.id == AiBaseSiteId::Start(1))
        .unwrap()
        .position;
    building(&mut s, 850, "barracks", (home.0 + 256., home.1), None);
    building(
        &mut s,
        851,
        "training_centre",
        (home.0, home.1 + 256.),
        None,
    );
    s.upgrades = vec![
        "entrenchment".into(),
        "panzerfausts".into(),
        "methamphetamines".into(),
        "tank_unlock".into(),
    ];
    (start, s, sites)
}

#[test]
fn works_stages_precede_expansion_without_overflow_before_territory_target() {
    for bases in 1..=6 {
        let (start, mut s, sites) = staged(bases);
        let f = frame(&start, &s, &sites);
        let home = f
            .owned()
            .iter()
            .find(|e| e.kind == EntityKind::ResourceDepot)
            .unwrap()
            .position;
        if bases >= 2 {
            assert_eq!(
                next_structure(&f, bases, 99),
                Some(EntityKind::EngineeringComplex)
            );
        }
        building(
            &mut s,
            860,
            "engineering_complex",
            (home.0 + 512., home.1 + 256.),
            None,
        );
        for works in 0..=works_target(bases) {
            let f = frame(&start, &s, &sites);
            let next = next_structure(&f, bases, 99);
            assert_eq!(
                next == Some(EntityKind::Factory),
                works < works_target(bases) || bases >= base_target(&f)
            );
            if works < works_target(bases) {
                building(
                    &mut s,
                    870 + works as u32,
                    "factory",
                    (home.0 + 640. + works as f32 * 256., home.1 + 256.),
                    None,
                );
            }
        }
    }
}

#[test]
fn tank_research_is_queued_on_scaffold_once_and_requeues_after_destruction() {
    let (start, mut s, sites) = staged(2);
    s.upgrades.retain(|u| u != "tank_unlock");
    s.steel = 0;
    s.oil = 0;
    let home = sites
        .iter()
        .find(|s| s.id == AiBaseSiteId::Start(1))
        .unwrap()
        .position;
    building(
        &mut s,
        860,
        "engineering_complex",
        (home.0 + 512., home.1 + 256.),
        Some(0.1),
    );
    let mut tank = production::EcoProduction::default();
    let f = frame(&start, &s, &sites);
    for expected in [true, false] {
        let mut a = AiActions::for_frame(&f);
        tank.step(&f, &mut a);
        assert_eq!(
            a.into_requests().iter().any(|r| matches!(
                r,
                AiActionRequest::Research {
                    building: 860,
                    upgrade: UpgradeKind::TankUnlock
                }
            )),
            expected
        );
    }
    s.entities.retain(|e| e.id != 860);
    building(
        &mut s,
        861,
        "engineering_complex",
        (home.0 + 512., home.1 + 256.),
        Some(0.1),
    );
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    tank.step(&f, &mut a);
    assert!(a.into_requests().iter().any(|r| matches!(
        r,
        AiActionRequest::Research {
            building: 861,
            upgrade: UpgradeKind::TankUnlock
        }
    )));
}

#[test]
fn expansion_plan_keeps_rear_posts_and_completion_returns_to_owned_base_plan() {
    let (start, mut s, sites) = staged(2);
    let specimen = s
        .entities
        .iter()
        .find(|e| e.kind == "rifleman")
        .unwrap()
        .clone();
    for id in 1000..1030 {
        let mut e = specimen.clone();
        e.id = id;
        e.kind = "machine_gunner".into();
        s.entities.push(e);
    }
    let f = frame(&start, &s, &sites);
    let owned = owned_sites(&f);
    let prospective = expansion(&f).unwrap().0;
    let mut defense = tactics::DefenseAssignments::default();
    let mut planner = DefensePlanner::with_firing_range_tiles(4.8);
    defense.prepare_expansion(prospective);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned);
    assert!(!defense.screen_ids().is_empty());
    assert!(
        defense
            .assigned_posts()
            .keys()
            .any(|id| !defense.screen_ids().contains(id)),
        "rear defenders must not all be screened to the new base"
    );
    let tile = defense.depot_tile().unwrap();
    let ts = f.map().tile_size as f32;
    let foot = AiRulebook::for_frame(&f)
        .unwrap()
        .entity(EntityKind::ResourceDepot)
        .unwrap()
        .footprint
        .unwrap();
    building(
        &mut s,
        900,
        "resource_depot",
        (
            (tile.x() as f32 + foot.width_tiles as f32 / 2.) * ts,
            (tile.y() as f32 + foot.height_tiles as f32 / 2.) * ts,
        ),
        None,
    );
    let f = frame(&start, &s, &sites);
    let owned = owned_sites(&f);
    assert_eq!(owned.len(), 3);
    defense.finish_completed_expansion(&owned);
    assert_eq!(defense.frontier(), None);
    let expected = DefensePlanner::with_firing_range_tiles(4.8).plan(&f, &owned, 34);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned);
    assert_eq!(defense.assigned_posts().len(), expected.slots.len());
    assert!(defense
        .assigned_posts()
        .values()
        .all(|point| expected.slots.iter().any(|slot| slot.position == *point)));
}

#[test]
fn standing_tanks_configure_scaffolds_once_and_replace_destroyed_works() {
    let (mut start, mut s, sites) = staged(2);
    let home = sites
        .iter()
        .find(|s| s.id == AiBaseSiteId::Start(1))
        .unwrap()
        .position;
    building(
        &mut s,
        870,
        "factory",
        (home.0 + 512., home.1 + 256.),
        Some(0.1),
    );
    // Queue/rally state is tested on open legal terrain, independent of fixture mining walls.
    start.map.terrain.fill(0);
    start.map.unwalkable_tiles.clear();
    start.map.resources.clear();
    s.entities
        .retain(|e| e.owner == 1 && matches!(e.kind.as_str(), "resource_depot" | "factory"));
    let mut tank = production::EcoProduction::default();
    let f = frame(&start, &s, &sites);
    for expected in [true, false] {
        let mut a = AiActions::for_frame(&f);
        tank.step(&f, &mut a);
        let requests = a.into_requests();
        assert_eq!(
            requests.iter().any(|r| matches!(
                r,
                AiActionRequest::AdjustProductionRepeat {
                    unit: EntityKind::Tank,
                    delta: 1,
                    ..
                }
            )),
            expected
        );
    }
    s.entities.retain(|e| e.id != 870);
    building(
        &mut s,
        871,
        "factory",
        (home.0 + 512., home.1 + 256.),
        Some(0.1),
    );
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    tank.step(&f, &mut a);
    assert!(a.into_requests().iter().any(|r|matches!(r,AiActionRequest::AdjustProductionRepeat {buildings,unit:EntityKind::Tank,delta:1} if buildings==&vec![871])));
}

#[test]
fn small_army_keeps_rear_defense_and_has_a_screen_before_future_mines_exist() {
    let (start, s, sites) = staged(2);
    let f = frame(&start, &s, &sites);
    let owned = owned_sites(&f);
    let site = expansion(&f).unwrap().0;
    let mut defense = tactics::DefenseAssignments::default();
    defense.prepare_expansion(site);
    let mut planner = DefensePlanner::with_firing_range_tiles(4.8);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned);
    assert!(
        !defense.screen_ids().is_empty(),
        "future value must not round every expansion sector down to zero staff"
    );
    assert!(
        defense
            .assigned_posts()
            .keys()
            .any(|id| !defense.screen_ids().contains(id)),
        "the rest of the small army still staffs owned-base sectors"
    );
}

#[test]
fn a_changed_screen_post_restarts_the_full_engineer_lead() {
    let (start, mut s, sites) = staged(2);
    let f = frame(&start, &s, &sites);
    let site = expansion(&f).unwrap().0;
    let mut defense = tactics::DefenseAssignments::default();
    defense.prepare_expansion(site);
    let mut planner = DefensePlanner::with_firing_range_tiles(4.8);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned_sites(&f));
    let old = defense.assigned_posts().clone();
    let old_screen = defense.screen_ids().clone();
    s.tick += SCREEN_LEAD_TICKS;
    let f = frame(&start, &s, &sites);
    assert!(defense.ready_for_builder(&f, site));
    let post = old[old_screen.first().unwrap()];
    building(&mut s, 950, "barracks", post, None);
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    defense.step(&f, &mut a, &mut planner, &owned_sites(&f));
    assert!(
        defense
            .screen_ids()
            .iter()
            .any(|id| !old_screen.contains(id) || old.get(id) != defense.assigned_posts().get(id)),
        "fixture must change an expansion screen assignment"
    );
    assert!(
        !defense.ready_for_builder(&f, site),
        "fresh screen movement cannot use a stale timestamp"
    );
    s.tick += SCREEN_LEAD_TICKS - 1;
    let f = frame(&start, &s, &sites);
    assert!(!defense.ready_for_builder(&f, site));
    s.tick += 1;
    let f = frame(&start, &s, &sites);
    assert!(defense.ready_for_builder(&f, site));
}

#[test]
fn barracks_auto_production_stays_configured_through_unfunded_build_and_research() {
    let (start, mut s, sites) = staged(2);
    s.steel = 0;
    s.oil = 0;
    s.upgrades.retain(|u| {
        !matches!(
            u.as_str(),
            "panzerfausts" | "methamphetamines" | "tank_unlock"
        )
    });
    let home = sites
        .iter()
        .find(|b| b.id == AiBaseSiteId::Start(1))
        .unwrap()
        .position;
    building(
        &mut s,
        860,
        "engineering_complex",
        (home.0 + 512., home.1),
        Some(0.1),
    );
    let mut eco = EcoStrategy::default();
    let worker = s
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "worker")
        .unwrap()
        .id;
    s.entities
        .iter_mut()
        .find(|e| e.id == worker)
        .unwrap()
        .state = "build".into();
    let pending = AiBuildObservation {
        worker_id: worker,
        kind: EntityKind::EngineeringComplex,
        tile_x: 20,
        tile_y: 10,
        phase: AiBuildObservationPhase::TravelingToSite,
    };
    let f = AiFrame::from_host(&start, &s, 1, [pending], Some(&[1, 2]))
        .unwrap()
        .with_base_sites(&sites);
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    let requests = a.into_requests();
    assert!(requests.iter().any(|r|matches!(r,AiActionRequest::AdjustProductionRepeat {buildings,unit:EntityKind::MachineGunner,delta:1} if buildings.contains(&850))));
    assert!(requests.iter().any(|r| matches!(
        r,
        AiActionRequest::Research {
            building: 860,
            upgrade: UpgradeKind::TankUnlock
        }
    )));
    assert!(!requests.iter().any(|r| matches!(
        r,
        AiActionRequest::Train {
            unit: EntityKind::MachineGunner,
            ..
        }
    )));
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    assert!(
        !a.into_requests().iter().any(|r| matches!(
            r,
            AiActionRequest::AdjustProductionRepeat {
                unit: EntityKind::MachineGunner,
                ..
            }
        )),
        "travel decisions must neither disable nor duplicate the standing MG order"
    );
    building(&mut s, 852, "barracks", (home.0 + 768., home.1), None);
    s.upgrades.push("panzerfausts".into());
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    assert!(a.into_requests().iter().any(|r| matches!(
        r,
        AiActionRequest::AdjustProductionRepeat {
            unit: EntityKind::Panzerfaust,
            delta: 1,
            ..
        }
    )));
}

#[test]
fn strategy_queues_building_without_waiting_for_income_or_cash() {
    let (start, mut s, sites) = fixture();
    s.steel = 0;
    s.oil = 0;
    for e in &mut s.entities {
        if e.kind == "steel_mine" || e.kind == "pump_jack" {
            e.extractor_active = Some(false);
        }
    }
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

#[test]
fn losing_the_initial_screen_requires_a_fresh_replacement_lead() {
    let (start, mut s, sites) = staged(2);
    let f = frame(&start, &s, &sites);
    let site = expansion(&f).unwrap().0;
    let mut defense = tactics::DefenseAssignments::default();
    defense.prepare_expansion(site);
    let mut planner = DefensePlanner::with_firing_range_tiles(4.8);
    defense.step(
        &f,
        &mut AiActions::for_frame(&f),
        &mut planner,
        &owned_sites(&f),
    );
    let initial = defense.screen_ids().clone();
    assert!(!initial.is_empty());
    let mut replacement = s
        .entities
        .iter()
        .find(|e| initial.contains(&e.id))
        .unwrap()
        .clone();
    replacement.id = 9800;
    s.entities.retain(|e| !initial.contains(&e.id));
    s.entities.push(replacement);
    s.tick += SCREEN_LEAD_TICKS;
    let f = frame(&start, &s, &sites);
    defense.step(
        &f,
        &mut AiActions::for_frame(&f),
        &mut planner,
        &owned_sites(&f),
    );
    assert!(
        !defense.ready_for_builder(&f, site),
        "a vanished initial wave cannot authorize a brand-new screen immediately"
    );
    s.tick += SCREEN_LEAD_TICKS;
    let f = frame(&start, &s, &sites);
    assert!(defense.ready_for_builder(&f, site));
}

#[test]
fn command_car_is_requested_after_three_completed_bases_once_and_replaced_after_loss() {
    let (start, mut s, sites) = staged(2);
    let home = frame(&start, &s, &sites)
        .owned()
        .iter()
        .find(|e| e.kind == EntityKind::ResourceDepot)
        .unwrap()
        .position;
    building(&mut s, 860, "factory", (home.0 + 512., home.1 + 256.), None);
    let mut p = production::EcoProduction::default();
    let run = |p: &mut production::EcoProduction, s: &Snapshot| {
        let f = frame(&start, s, &sites);
        let mut a = AiActions::for_frame(&f);
        p.step(&f, &mut a);
        a.into_requests()
    };
    let car = |orders: &[AiActionRequest]| {
        orders.iter().any(|r| {
            matches!(
                r,
                AiActionRequest::Train {
                    unit: EntityKind::CommandCar,
                    ..
                }
            )
        })
    };
    assert!(!car(&run(&mut p, &s)));
    let f = frame(&start, &s, &sites);
    let next = WorldQueries::new(&f)
        .ranked_bases(1)
        .into_iter()
        .find(|b| {
            WorldQueries::new(&f)
                .known_base_depots(b.site.id)
                .is_empty()
        })
        .unwrap()
        .site
        .position;
    building(&mut s, 803, "resource_depot", next, Some(0.5));
    assert!(!car(&run(&mut p, &s)));
    s.entities
        .iter_mut()
        .find(|e| e.id == 803)
        .unwrap()
        .build_progress = None;
    assert!(car(&run(&mut p, &s)));
    assert!(!car(&run(&mut p, &s)));
    let mut vehicle = s
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "rifleman")
        .unwrap()
        .clone();
    vehicle.id = 999;
    vehicle.kind = "command_car".into();
    s.entities.push(vehicle);
    assert!(!car(&run(&mut p, &s)));
    s.entities.retain(|e| e.id != 999);
    assert!(car(&run(&mut p, &s)));
}

#[test]
fn command_car_target_is_completed_bases_minus_two() {
    for bases in 3..=7 {
        let (start, mut s, sites) = staged(bases);
        let home = sites
            .iter()
            .find(|b| b.id == AiBaseSiteId::Start(1))
            .unwrap()
            .position;
        building(&mut s, 860, "factory", (home.0 + 512., home.1 + 256.), None);
        let template = s
            .entities
            .iter()
            .find(|e| e.kind == "rifleman" && e.owner == 1)
            .unwrap()
            .clone();
        let mut p = production::EcoProduction::default();
        for n in 0..=bases - 2 {
            let f = frame(&start, &s, &sites);
            let mut a = AiActions::for_frame(&f);
            p.step(&f, &mut a);
            let queued = a.into_requests().iter().any(|r| {
                matches!(
                    r,
                    AiActionRequest::Train {
                        unit: EntityKind::CommandCar,
                        ..
                    }
                )
            });
            assert_eq!(queued, n < bases - 2, "bases={bases}, cars={n}");
            let mut car = template.clone();
            car.kind = "command_car".into();
            car.id = 2000 + n as u32;
            s.entities.push(car);
        }
    }
}
