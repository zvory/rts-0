use super::*;

fn actions(steel: u32, oil: u32, free_supply: u32) -> AiActions {
    AiActions::with_budget(ActionBudget::new(steel, oil, 0, free_supply))
}

fn state(actions: &AiActions) -> (ActionBudgetSnapshot, ReservationCounts, usize) {
    (
        actions.remaining_budget(),
        actions.reservation_counts(),
        actions.len(),
    )
}

#[test]
fn unit_group_is_sorted_deduplicated_and_non_empty() {
    assert_eq!(UnitGroup::new([9, 2, 9, 5]).unwrap().units(), &[2, 5, 9]);
    assert_eq!(
        UnitGroup::new([]).unwrap_err().blocker(),
        &ActionBlocker::EmptyGroup
    );
}

#[test]
fn local_blockers_leave_the_entire_batch_unchanged() {
    let mut batch = actions(0, 0, 0);
    let before = state(&batch);
    assert_eq!(
        batch
            .paid_build(&[1], EntityKind::Depot, 4, 5)
            .unwrap_err()
            .blocker(),
        &ActionBlocker::InsufficientBudget {
            steel: rts_rules::economy::cost(EntityKind::Depot).0,
            oil: rts_rules::economy::cost(EntityKind::Depot).1,
            supply: 0,
        }
    );
    assert_eq!(state(&batch), before);

    assert_eq!(
        batch
            .paid_build(&[1], EntityKind::Worker, 4, 5)
            .unwrap_err()
            .blocker(),
        &ActionBlocker::UnsupportedKind(EntityKind::Worker)
    );
    assert_eq!(state(&batch), before);

    assert_eq!(
        batch.gather(&[], &[20], false).unwrap_err().blocker(),
        &ActionBlocker::EmptyCandidates(ReservationNamespace::Actor)
    );
    assert_eq!(state(&batch), before);
}

#[test]
fn known_candidate_and_producer_blockers_are_mutation_free() {
    let mut batch = actions(500, 500, 20);
    batch.resource_ids.insert(40);
    batch.owned_kinds.insert(20, EntityKind::Barracks);
    let before = state(&batch);

    assert_eq!(
        batch.gather(&[1], &[41], false).unwrap_err().blocker(),
        &ActionBlocker::NoKnownCandidate(ReservationNamespace::ResourceNode)
    );
    assert_eq!(state(&batch), before);

    assert_eq!(
        batch.train(&[20], EntityKind::Tank).unwrap_err().blocker(),
        &ActionBlocker::NoCompatibleProducer
    );
    assert_eq!(state(&batch), before);
}

#[test]
fn empty_frame_knowledge_does_not_disable_candidate_validation() {
    let mut batch = actions(500, 500, 20);
    let before = state(&batch);

    assert_eq!(
        batch.gather(&[1], &[40], false).unwrap_err().blocker(),
        &ActionBlocker::NoKnownCandidate(ReservationNamespace::ResourceNode)
    );
    assert_eq!(state(&batch), before);

    assert_eq!(
        batch
            .train(&[20], EntityKind::Worker)
            .unwrap_err()
            .blocker(),
        &ActionBlocker::NoCompatibleProducer
    );
    assert_eq!(state(&batch), before);
}

#[test]
fn reserved_candidate_blocker_identifies_the_known_candidate() {
    let mut batch = actions(500, 500, 20);
    batch.resource_ids.insert(40);
    assert_eq!(batch.gather(&[1], &[40], false), Ok((1, 40)));
    let before = state(&batch);

    assert_eq!(
        batch.gather(&[2], &[999, 40], false).unwrap_err().blocker(),
        &ActionBlocker::AlreadyReserved {
            namespace: ReservationNamespace::ResourceNode,
            id: 40,
        }
    );
    assert_eq!(state(&batch), before);
}

#[test]
fn reservations_are_independent_and_conflicts_are_mutation_free() {
    let mut batch = actions(500, 500, 20);
    batch.resource_ids.extend([7, 8]);
    batch.owned_kinds.insert(7, EntityKind::CityCentre);
    assert_eq!(batch.gather(&[7], &[7], false), Ok((7, 7)));
    assert_eq!(batch.train(&[7], EntityKind::Worker), Ok(7));
    assert_eq!(
        batch.reservation_counts(),
        ReservationCounts {
            actors: 1,
            resource_nodes: 1,
            producers: 1,
        }
    );

    let before = state(&batch);
    assert_eq!(
        batch.gather(&[7], &[8], false).unwrap_err().blocker(),
        &ActionBlocker::AlreadyReserved {
            namespace: ReservationNamespace::Actor,
            id: 7,
        }
    );
    assert_eq!(state(&batch), before);

    assert_eq!(
        batch.gather(&[8], &[7], false).unwrap_err().blocker(),
        &ActionBlocker::AlreadyReserved {
            namespace: ReservationNamespace::ResourceNode,
            id: 7,
        }
    );
    assert_eq!(state(&batch), before);

    assert_eq!(
        batch.train(&[7], EntityKind::Worker).unwrap_err().blocker(),
        &ActionBlocker::AlreadyReserved {
            namespace: ReservationNamespace::Producer,
            id: 7,
        }
    );
    assert_eq!(state(&batch), before);
}

#[test]
fn typed_helpers_preserve_mixed_call_order_and_flags() {
    let mut batch = actions(10_000, 10_000, 100);
    batch.owned_kinds.insert(20, EntityKind::CityCentre);
    batch.owned_kinds.insert(21, EntityKind::TrainingCentre);
    batch.resource_ids.insert(30);
    batch
        .paid_build(&[4, 3], EntityKind::Depot, 10, 11)
        .unwrap();
    batch
        .resume_build(&[5], EntityKind::Barracks, 12, 13)
        .unwrap();
    batch.train(&[20], EntityKind::Worker).unwrap();
    batch.research(&[21], UpgradeKind::Entrenchment).unwrap();
    batch.gather(&[6], &[30], true).unwrap();
    let movers = UnitGroup::new([9, 8, 9]).unwrap();
    batch.move_group(&movers, 1.0, 2.0, true).unwrap();
    let attackers = UnitGroup::new([11, 10]).unwrap();
    batch.attack_move(&attackers, 3.0, 4.0, false).unwrap();
    let direct = UnitGroup::new([12]).unwrap();
    batch.attack(&direct, 99, true).unwrap();
    let holders = UnitGroup::new([13]).unwrap();
    batch.hold_position(&holders, false).unwrap();
    let guns = UnitGroup::new([14]).unwrap();
    batch.setup_anti_tank_guns(&guns, 5.0, 6.0, true).unwrap();

    assert!(matches!(
        batch.requests.as_slice(),
        [
            AiActionRequest::Build { units: build, queued: false, .. },
            AiActionRequest::Build { units: resume, queued: false, .. },
            AiActionRequest::Train { building: 20, .. },
            AiActionRequest::Research { building: 21, .. },
            AiActionRequest::Gather { units: gather, node: 30, queued: true },
            AiActionRequest::Move { units: moved, queued: true, .. },
            AiActionRequest::AttackMove { units: attack_move, queued: false, .. },
            AiActionRequest::Attack { units: attack, target: 99, queued: true },
            AiActionRequest::HoldPosition { units: hold, queued: false },
            AiActionRequest::SetupAntiTankGuns { units: setup, queued: true, .. },
        ] if build == &[4]
            && resume == &[5]
            && gather == &[6]
            && moved == &[8, 9]
            && attack_move == &[10, 11]
            && attack == &[12]
            && hold == &[13]
            && setup == &[14]
    ));
}

#[test]
fn capacity_blocker_is_mutation_free() {
    let mut batch = actions(1_000, 1_000, 20);
    for id in 0..MAX_ACTIONS_PER_STEP as u32 {
        batch.emit_compat(AiActionRequest::Move {
            units: vec![id],
            x: 0.0,
            y: 0.0,
            queued: false,
        });
    }
    let before = state(&batch);
    let group = UnitGroup::new([999]).unwrap();
    assert_eq!(
        batch
            .move_group(&group, 1.0, 1.0, false)
            .unwrap_err()
            .blocker(),
        &ActionBlocker::ActionCapacity
    );
    assert_eq!(state(&batch), before);
}
