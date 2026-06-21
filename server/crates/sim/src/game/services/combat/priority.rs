use std::cmp::Ordering;

use crate::game::entity::EntityKind;
use crate::rules::combat as combat_rules;
use crate::rules::defs::{ArmorClass, WeaponClass};

#[derive(Debug, Clone, Copy)]
pub(super) struct AttackPriorityContext {
    pub attacker_kind: EntityKind,
    pub attacker_is_unit: bool,
    pub prefers_armored: bool,
    pub can_retain_moving_target: bool,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(super) struct TargetCandidate {
    pub id: u32,
    pub kind: EntityKind,
    pub owner: u32,
    pub pos_x: f32,
    pub pos_y: f32,
    pub distance_sq: f32,
    pub is_unit: bool,
    pub is_building: bool,
    pub armor_class: Option<ArmorClass>,
    pub weapon_class: WeaponClass,
    pub in_weapon_range: bool,
    pub tank_trap_auto_relevant: bool,
    pub retained_target: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TargetRank {
    priority_bucket: u8,
    policy_order: u8,
    distance_sq: f32,
    id: u32,
}

pub(super) fn choose_target<'a>(
    context: &AttackPriorityContext,
    candidates: impl IntoIterator<Item = &'a TargetCandidate>,
) -> Option<u32> {
    candidates
        .into_iter()
        .min_by(|left, right| compare_candidates(context, left, right))
        .map(|candidate| candidate.id)
}

fn compare_candidates(
    context: &AttackPriorityContext,
    left: &TargetCandidate,
    right: &TargetCandidate,
) -> Ordering {
    let left_rank = rank_candidate(context, left);
    let right_rank = rank_candidate(context, right);
    left_rank
        .priority_bucket
        .cmp(&right_rank.priority_bucket)
        .then_with(|| left_rank.policy_order.cmp(&right_rank.policy_order))
        .then_with(|| left_rank.distance_sq.total_cmp(&right_rank.distance_sq))
        .then_with(|| left_rank.id.cmp(&right_rank.id))
}

fn rank_candidate(context: &AttackPriorityContext, candidate: &TargetCandidate) -> TargetRank {
    let (priority_bucket, policy_order) =
        if let Some(order) = tank_priority_order(context, candidate) {
            (0, order)
        } else if context.can_retain_moving_target && candidate.retained_target {
            (1, 0)
        } else if context.prefers_armored && candidate.kind == EntityKind::Tank {
            (2, 0)
        } else if context.attacker_is_unit && candidate.is_unit {
            (3, 0)
        } else {
            (4, 0)
        };

    TargetRank {
        priority_bucket,
        policy_order,
        distance_sq: candidate.distance_sq,
        id: candidate.id,
    }
}

fn tank_priority_order(context: &AttackPriorityContext, candidate: &TargetCandidate) -> Option<u8> {
    if context.attacker_kind != EntityKind::Tank || !candidate.in_weapon_range {
        return None;
    }
    combat_rules::TANK_TARGET_PRIORITY_ORDER
        .iter()
        .position(|kind| *kind == candidate.kind)
        .and_then(|index| u8::try_from(index).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(attacker_kind: EntityKind) -> AttackPriorityContext {
        AttackPriorityContext {
            attacker_kind,
            attacker_is_unit: true,
            prefers_armored: combat_rules::prefers_armored_targets(attacker_kind),
            can_retain_moving_target: matches!(
                attacker_kind,
                EntityKind::Tank | EntityKind::ScoutCar
            ),
        }
    }

    fn candidate(
        id: u32,
        kind: EntityKind,
        distance_sq: f32,
        retained_target: bool,
    ) -> TargetCandidate {
        TargetCandidate {
            id,
            kind,
            owner: 2,
            pos_x: 0.0,
            pos_y: 0.0,
            distance_sq,
            is_unit: kind.is_unit(),
            is_building: kind.is_building(),
            armor_class: combat_rules::armor_class(kind),
            weapon_class: combat_rules::weapon_class(kind),
            in_weapon_range: true,
            tank_trap_auto_relevant: kind == EntityKind::TankTrap,
            retained_target,
        }
    }

    #[test]
    fn tank_priority_beats_retained_lower_priority_target() {
        let candidates = [
            candidate(10, EntityKind::Worker, 900.0, true),
            candidate(11, EntityKind::AntiTankGun, 3_600.0, false),
        ];

        assert_eq!(
            choose_target(&context(EntityKind::Tank), &candidates),
            Some(11)
        );
    }

    #[test]
    fn retained_moving_target_beats_nearer_fallback_target() {
        let candidates = [
            candidate(10, EntityKind::Worker, 2_500.0, true),
            candidate(11, EntityKind::Worker, 900.0, false),
        ];

        assert_eq!(
            choose_target(&context(EntityKind::ScoutCar), &candidates),
            Some(10)
        );
    }

    #[test]
    fn anti_tank_gun_prefers_tank_before_unit_fallback() {
        let candidates = [
            candidate(10, EntityKind::Rifleman, 400.0, false),
            candidate(11, EntityKind::Tank, 2_500.0, false),
        ];

        assert_eq!(
            choose_target(&context(EntityKind::AntiTankGun), &candidates),
            Some(11)
        );
    }
}
