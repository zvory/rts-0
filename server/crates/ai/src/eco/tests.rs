use super::*;
use crate::sdk::actions::AiActionRequest;
use crate::sdk::{AiBaseSite, AiBuildObservation, AiBuildObservationPhase};
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::{Snapshot, StartPayload};

fn fixture() -> (StartPayload, Snapshot, Vec<AiBaseSite>) {
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
fn frame(start: &StartPayload, s: &Snapshot, sites: &[AiBaseSite]) -> AiFrame {
    AiFrame::from_host(start, s, 1, [], Some(&[1, 2]))
        .unwrap()
        .with_base_sites(sites)
}
fn building(s: &mut Snapshot, id: u32, kind: &str, p: (f32, f32), progress: Option<f32>) {
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
    assert_eq!(desired_research(&f, 1), Some(UpgradeKind::Entrenchment));
    assert_eq!(barracks_target(1), 1);
    assert_eq!(barracks_target(2), 2);
    assert_eq!(barracks_target(3), 4);
    s.upgrades = vec!["entrenchment".into()];
    let f = frame(&start, &s, &sites);
    assert_eq!(desired_research(&f, owned_sites(&f).len()), None);
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
fn troop_mix_counts_each_current_queue_once() {
    let (start, mut s, sites) = fixture();
    let home = sites[0].position;
    building(&mut s, 801, "barracks", home, None);
    building(&mut s, 802, "barracks", home, None);
    for e in s.entities.iter_mut().filter(|e| e.id == 801 || e.id == 802) {
        e.prod_kind = Some(
            if e.id == 801 {
                "machine_gunner"
            } else {
                "panzerfaust"
            }
            .into(),
        );
        e.prod_queue = Some(1);
    }
    let f = frame(&start, &s, &sites);
    let (mg, pf) = infantry_counts(&f);
    assert_eq!(mg, count(&f, EntityKind::MachineGunner) + 1);
    assert_eq!(pf, count(&f, EntityKind::Panzerfaust) + 1);
    assert!(!f
        .owned()
        .iter()
        .find(|e| e.id == 801)
        .is_some_and(idle_producer));
}
#[test]
fn traveling_build_keeps_bank_reserved_and_stale_build_worker_can_retry() {
    let (start, mut s, sites) = fixture();
    let worker = s
        .entities
        .iter_mut()
        .find(|e| e.owner == 1 && e.kind == "worker")
        .unwrap();
    let id = worker.id;
    worker.state = "build".into();
    let f = AiFrame::from_host(
        &start,
        &s,
        1,
        [AiBuildObservation {
            worker_id: id,
            kind: EntityKind::Barracks,
            tile_x: 10,
            tile_y: 10,
            phase: AiBuildObservationPhase::TravelingToSite,
        }],
        Some(&[1, 2]),
    )
    .unwrap()
    .with_base_sites(&sites);
    let mut a = AiActions::for_frame(&f);
    let mut eco = EcoStrategy::default();
    eco.step(&f, &mut a);
    assert!(!a.into_requests().iter().any(|r| matches!(
        r,
        AiActionRequest::Train { .. }
            | AiActionRequest::Research { .. }
            | AiActionRequest::Build { .. }
    )));
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    assert!(a.into_requests().iter().any(|r| matches!(
        r,
        AiActionRequest::Build {
            building: EntityKind::Barracks,
            ..
        }
    )));
}
#[test]
fn builder_recovery_disables_extractor_repeats_and_pays_for_one_worker() {
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
    assert!(requests
        .iter()
        .any(|r| matches!(r, AiActionRequest::AdjustProductionRepeat { delta: -1, .. })));
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
}

#[test]
fn automatic_extractor_scaffolds_do_not_capture_the_builder() {
    let (start, mut s, sites) = fixture();
    let home = sites
        .iter()
        .find(|b| b.id == AiBaseSiteId::Start(1))
        .unwrap()
        .position;
    building(
        &mut s,
        900,
        "steel_mine",
        (home.0 + 128., home.1),
        Some(0.2),
    );
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
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    let before = a.remaining_budget();
    EcoStrategy::default().step(&f, &mut a);
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
fn four_base_surplus_adds_capacity_and_remembers_destroyed_overflow() {
    let (start, mut s, sites) = fixture();
    let home = sites
        .iter()
        .find(|b| b.id == AiBaseSiteId::Start(1))
        .unwrap()
        .position;
    let candidates: Vec<_> = sites
        .iter()
        .filter(|b| matches!(b.id, AiBaseSiteId::ResourceCluster(_)))
        .take(3)
        .collect();
    assert_eq!(candidates.len(), 3);
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
    let f = frame(&start, &s, &sites);
    assert_eq!(owned_sites(&f).len(), 4);
    let mut eco = EcoStrategy::default();
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    assert_eq!(eco.overflow_barracks, 5);
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    assert!(a.into_requests().iter().any(|r| matches!(
        r,
        AiActionRequest::Build {
            building: EntityKind::Barracks,
            ..
        }
    )));
    s.entities.retain(|e| e.id != 850);
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    eco.step(&f, &mut a);
    assert_eq!(eco.overflow_barracks, 5);
    assert!(a.into_requests().iter().any(|r| matches!(
        r,
        AiActionRequest::Build {
            building: EntityKind::Barracks,
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
    building(&mut s, 801, "barracks", (home.0 + 128., home.1), None);
    building(&mut s, 802, "barracks", (home.0 + 256., home.1), None);
    building(
        &mut s,
        803,
        "training_centre",
        (home.0, home.1 + 128.),
        None,
    );
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
    let f = frame(&start, &s, &sites);
    let mut a = AiActions::for_frame(&f);
    EcoStrategy::default().step(&f, &mut a);
    assert!(a.into_requests().iter().any(|r| matches!(
        r,
        AiActionRequest::Build {
            building: EntityKind::ResourceDepot,
            ..
        }
    )));
}

#[test]
fn blocked_construction_preserves_expansion_money_but_trains_from_surplus() {
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
    building(&mut s, 801, "barracks", (home.0 + 128., home.1), None);
    building(&mut s, 802, "barracks", (home.0 + 256., home.1), None);
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
        "training_centre",
        (home.0, home.1 + 256.),
        Some(0.5),
    );
    s.entities
        .iter_mut()
        .find(|e| e.owner == 1 && e.kind == "worker")
        .unwrap()
        .state = "build".into();
    s.upgrades = vec![
        "entrenchment".into(),
        "panzerfausts".into(),
        "methamphetamines".into(),
    ];
    let rules = AiRulebook::new("kriegsia").unwrap();
    let reserve = rules.cost(EntityKind::ResourceDepot).unwrap();
    let troop = rules.cost(EntityKind::MachineGunner).unwrap();
    for surplus in [false, true] {
        s.steel = reserve.steel + if surplus { troop.steel } else { 0 };
        s.oil = reserve.oil + if surplus { troop.oil } else { 0 };
        let f = frame(&start, &s, &sites);
        let mut a = AiActions::for_frame(&f);
        EcoStrategy::default().step(&f, &mut a);
        assert!(a.remaining_budget().steel >= reserve.steel);
        assert!(a.remaining_budget().oil >= reserve.oil);
        assert_eq!(
            a.into_requests()
                .iter()
                .filter(|r| matches!(r, AiActionRequest::Train { .. }))
                .count(),
            usize::from(surplus)
        );
    }
}
