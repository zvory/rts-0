use std::cmp::Ordering;

use crate::rules::combat as combat_rules;
use crate::rules::defs::WeaponClass;
use crate::rules::target::TargetFacts;

use super::target_policy::TargetPriorityPolicy;

#[derive(Debug, Clone, Copy)]
pub(super) struct AttackPriorityContext {
    pub attacker_is_unit: bool,
    /// The ranking policy applies to the current default attack only. Future
    /// grenades, satchels, or melee demolition profiles should build their own
    /// activation/ranking context instead of changing this default profile.
    pub attacker_weapon_class: WeaponClass,
    pub policy_id: combat_rules::TargetPriorityPolicyId,
    /// Moving-fire units may keep a still-legal target inside the same material
    /// rank, but higher-rank default-weapon threats are allowed to steal focus.
    pub can_retain_moving_target: bool,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(super) struct TargetCandidate {
    pub id: u32,
    pub owner: u32,
    pub pos_x: f32,
    pub pos_y: f32,
    pub distance_sq: f32,
    pub facts: TargetFacts,
    pub in_weapon_range: bool,
    pub tank_trap_obstructs_vehicle_route: bool,
    pub retained_target: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TargetRank {
    priority_bucket: u8,
    target_group_order: u8,
    weapon_fit_order: u8,
    retention_order: u8,
    distance_sq: f32,
    id: u32,
}

#[derive(Debug, Clone, Copy)]
struct RankedTarget {
    id: u32,
    rank: TargetRank,
}

pub(super) fn choose_target<'a>(
    context: &AttackPriorityContext,
    candidates: impl IntoIterator<Item = &'a TargetCandidate>,
) -> Option<u32> {
    candidates
        .into_iter()
        .filter_map(|candidate| {
            rank_candidate(context, candidate).map(|rank| RankedTarget {
                id: candidate.id,
                rank,
            })
        })
        .min_by(|left, right| compare_ranks(&left.rank, &right.rank))
        .map(|target| target.id)
}

fn compare_ranks(left_rank: &TargetRank, right_rank: &TargetRank) -> Ordering {
    left_rank
        .priority_bucket
        .cmp(&right_rank.priority_bucket)
        .then_with(|| {
            left_rank
                .target_group_order
                .cmp(&right_rank.target_group_order)
        })
        .then_with(|| left_rank.weapon_fit_order.cmp(&right_rank.weapon_fit_order))
        .then_with(|| left_rank.retention_order.cmp(&right_rank.retention_order))
        .then_with(|| left_rank.distance_sq.total_cmp(&right_rank.distance_sq))
        .then_with(|| left_rank.id.cmp(&right_rank.id))
}

fn rank_candidate(
    context: &AttackPriorityContext,
    candidate: &TargetCandidate,
) -> Option<TargetRank> {
    let policy = TargetPriorityPolicy::for_id(context.policy_id);
    if !policy.allows_candidate(candidate.facts) {
        return None;
    }
    let (priority_bucket, target_group_order, weapon_fit_order) = if let Some(order) = policy
        .immediate_threat_order(
            candidate.facts,
            candidate.in_weapon_range,
            candidate.tank_trap_obstructs_vehicle_route,
        ) {
        (0, order, 0)
    } else if let Some(order) = policy.vehicle_route_obstruction_order(
        candidate.facts,
        candidate.tank_trap_obstructs_vehicle_route,
    ) {
        (0, order, 0)
    } else {
        (
            1,
            policy.target_group_order(context.attacker_is_unit, candidate.facts),
            policy.weapon_fit_order(
                context.attacker_weapon_class,
                candidate.facts,
                candidate.in_weapon_range,
            ),
        )
    };

    Some(TargetRank {
        priority_bucket,
        target_group_order,
        weapon_fit_order,
        retention_order: policy
            .retention_order(context.can_retain_moving_target, candidate.retained_target),
        distance_sq: candidate.distance_sq,
        id: candidate.id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::EntityKind;

    fn context(attacker_kind: EntityKind) -> AttackPriorityContext {
        AttackPriorityContext {
            attacker_is_unit: true,
            attacker_weapon_class: combat_rules::weapon_class(attacker_kind),
            policy_id: combat_rules::default_target_priority_policy(attacker_kind),
            can_retain_moving_target: matches!(
                attacker_kind,
                EntityKind::Tank | EntityKind::ScoutCar
            ),
        }
    }

    fn coax_context() -> AttackPriorityContext {
        AttackPriorityContext {
            attacker_is_unit: true,
            attacker_weapon_class: WeaponClass::SmallArms,
            policy_id: combat_rules::TargetPriorityPolicyId::TankCoaxMachineGun,
            can_retain_moving_target: false,
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
            owner: 2,
            pos_x: 0.0,
            pos_y: 0.0,
            distance_sq,
            facts: crate::rules::target::target_facts(kind),
            in_weapon_range: true,
            tank_trap_obstructs_vehicle_route: false,
            retained_target,
        }
    }

    fn obstructing_tank_trap(id: u32, distance_sq: f32) -> TargetCandidate {
        TargetCandidate {
            tank_trap_obstructs_vehicle_route: true,
            ..candidate(id, EntityKind::TankTrap, distance_sq, false)
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
    fn retained_moving_target_beats_nearer_equal_rank_target() {
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
    fn materially_better_default_target_beats_retained_lower_rank_target() {
        let candidates = [
            candidate(10, EntityKind::TankTrap, 400.0, true),
            candidate(11, EntityKind::Worker, 2_500.0, false),
        ];

        assert_eq!(
            choose_target(&context(EntityKind::ScoutCar), &candidates),
            Some(11)
        );
    }

    #[test]
    fn retained_moving_target_does_not_affect_first_acquisition_tie_breaks() {
        let first = candidate(10, EntityKind::Worker, 900.0, false);
        let second = candidate(11, EntityKind::Worker, 900.0, false);

        assert_eq!(
            choose_target(&context(EntityKind::ScoutCar), &[second, first]),
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

    #[test]
    fn anti_armor_prefers_threat_over_generic_armored_target() {
        let candidates = [
            candidate(10, EntityKind::Barracks, 400.0, false),
            candidate(11, EntityKind::Tank, 2_500.0, false),
        ];

        assert_eq!(
            choose_target(&context(EntityKind::AntiTankGun), &candidates),
            Some(11)
        );
    }

    #[test]
    fn default_weapon_prefers_combat_units_over_nearer_workers() {
        let candidates = [
            candidate(10, EntityKind::Worker, 400.0, false),
            candidate(11, EntityKind::Rifleman, 2_500.0, false),
        ];

        assert_eq!(
            choose_target(&context(EntityKind::MachineGunner), &candidates),
            Some(11)
        );
    }

    #[test]
    fn unit_attackers_prefer_workers_over_armored_buildings() {
        let candidates = [
            candidate(10, EntityKind::CityCentre, 400.0, false),
            candidate(11, EntityKind::Worker, 2_500.0, false),
        ];

        assert_eq!(
            choose_target(&context(EntityKind::Tank), &candidates),
            Some(11)
        );
    }

    #[test]
    fn worker_group_beats_weapon_fit_for_building_cleanup() {
        let candidates = [
            candidate(10, EntityKind::CityCentre, 400.0, false),
            candidate(11, EntityKind::Worker, 2_500.0, false),
        ];

        assert_eq!(
            choose_target(&context(EntityKind::AntiTankGun), &candidates),
            Some(11)
        );
    }

    #[test]
    fn cleanup_building_targets_still_use_weapon_fit() {
        let candidates = [
            candidate(10, EntityKind::CityCentre, 400.0, false),
            candidate(11, EntityKind::PumpJack, 2_500.0, false),
        ];

        assert_eq!(
            choose_target(&context(EntityKind::Rifleman), &candidates),
            Some(11)
        );
    }

    #[test]
    fn small_arms_prefers_soft_target_over_nearer_armored_target() {
        let candidates = [
            candidate(10, EntityKind::Tank, 400.0, false),
            candidate(11, EntityKind::MachineGunner, 2_500.0, false),
        ];

        assert_eq!(
            choose_target(&context(EntityKind::Rifleman), &candidates),
            Some(11)
        );
    }

    #[test]
    fn scout_car_prefers_soft_target_over_nearer_irrelevant_tank_trap() {
        let candidates = [
            candidate(10, EntityKind::TankTrap, 400.0, false),
            candidate(11, EntityKind::Worker, 2_500.0, false),
        ];

        assert_eq!(
            choose_target(&context(EntityKind::ScoutCar), &candidates),
            Some(11)
        );
    }

    #[test]
    fn vehicle_body_prefers_obstructing_tank_trap_over_soft_target() {
        let candidates = [
            obstructing_tank_trap(10, 900.0),
            candidate(11, EntityKind::Worker, 400.0, false),
        ];

        assert_eq!(
            choose_target(&context(EntityKind::ScoutCar), &candidates),
            Some(10)
        );
    }

    #[test]
    fn tank_keeps_anti_tank_gun_above_obstructing_tank_trap() {
        let candidates = [
            obstructing_tank_trap(10, 400.0),
            candidate(11, EntityKind::AntiTankGun, 2_500.0, false),
        ];

        assert_eq!(
            choose_target(&context(EntityKind::Tank), &candidates),
            Some(11)
        );
    }

    #[test]
    fn equal_rank_targets_use_distance_then_id_tie_breaks() {
        let farther = candidate(10, EntityKind::Worker, 2_500.0, false);
        let nearer = candidate(11, EntityKind::Worker, 400.0, false);
        assert_eq!(
            choose_target(&context(EntityKind::Rifleman), &[farther, nearer]),
            Some(11)
        );

        let first = candidate(10, EntityKind::Worker, 900.0, false);
        let second = candidate(11, EntityKind::Worker, 900.0, false);
        assert_eq!(
            choose_target(&context(EntityKind::Rifleman), &[first, second]),
            Some(10)
        );
    }

    #[test]
    fn coax_policy_prioritizes_infantry_priority_targets() {
        let candidates = [
            candidate(10, EntityKind::Tank, 400.0, false),
            candidate(11, EntityKind::Worker, 2_500.0, false),
        ];

        assert_eq!(choose_target(&coax_context(), &candidates), Some(11));
    }

    #[test]
    fn coax_policy_excludes_support_and_other_infantry_from_priority_bucket() {
        let candidates = [
            candidate(10, EntityKind::Golem, 100.0, false),
            candidate(11, EntityKind::MortarTeam, 200.0, false),
            candidate(12, EntityKind::Artillery, 300.0, false),
            candidate(13, EntityKind::AntiTankGun, 400.0, false),
            candidate(14, EntityKind::Ekat, 500.0, false),
            candidate(15, EntityKind::Rifleman, 2_500.0, false),
        ];

        assert_eq!(choose_target(&coax_context(), &candidates), Some(15));
    }

    #[test]
    fn coax_policy_falls_back_by_distance_across_legal_materials() {
        let candidates = [
            candidate(10, EntityKind::Tank, 2_500.0, false),
            candidate(11, EntityKind::Barracks, 900.0, false),
            obstructing_tank_trap(12, 400.0),
        ];

        assert_eq!(choose_target(&coax_context(), &candidates), Some(12));
    }

    #[test]
    fn coax_policy_uses_distance_then_id_inside_priority_bucket() {
        let farther = candidate(10, EntityKind::Worker, 2_500.0, false);
        let nearer = candidate(11, EntityKind::MachineGunner, 400.0, false);
        assert_eq!(choose_target(&coax_context(), &[farther, nearer]), Some(11));

        let first = candidate(10, EntityKind::Worker, 900.0, false);
        let second = candidate(11, EntityKind::Rifleman, 900.0, false);
        assert_eq!(choose_target(&coax_context(), &[second, first]), Some(10));
    }

    #[test]
    fn coax_policy_has_no_tank_cannon_threat_ordering() {
        let candidates = [
            candidate(10, EntityKind::AntiTankGun, 2_500.0, false),
            candidate(11, EntityKind::Tank, 400.0, false),
        ];

        assert_eq!(choose_target(&coax_context(), &candidates), Some(11));
    }

    #[test]
    fn coax_policy_ignores_resource_nodes() {
        let candidates = [
            candidate(10, EntityKind::Steel, 100.0, false),
            candidate(11, EntityKind::Tank, 400.0, false),
        ];

        assert_eq!(choose_target(&coax_context(), &candidates), Some(11));
    }
}
