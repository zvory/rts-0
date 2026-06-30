use std::cmp::Ordering;

use crate::game::entity::EntityKind;
use crate::rules::combat as combat_rules;
use crate::rules::defs::{ArmorClass, WeaponClass};

#[derive(Debug, Clone, Copy)]
pub(super) struct AttackPriorityContext {
    pub attacker_kind: EntityKind,
    pub attacker_is_unit: bool,
    /// The ranking policy applies to the current default attack only. Future
    /// grenades, satchels, or melee demolition profiles should build their own
    /// activation/ranking context instead of changing this default profile.
    pub attacker_weapon_class: WeaponClass,
    /// Moving-fire units may keep a still-legal target inside the same material
    /// rank, but higher-rank default-weapon threats are allowed to steal focus.
    pub can_retain_moving_target: bool,
    pub attacker_is_vehicle_body: bool,
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
    pub threat_role: combat_rules::TargetThreatRole,
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
        .then_with(|| left_rank.target_group_order.cmp(&right_rank.target_group_order))
        .then_with(|| left_rank.weapon_fit_order.cmp(&right_rank.weapon_fit_order))
        .then_with(|| left_rank.retention_order.cmp(&right_rank.retention_order))
        .then_with(|| left_rank.distance_sq.total_cmp(&right_rank.distance_sq))
        .then_with(|| left_rank.id.cmp(&right_rank.id))
}

fn rank_candidate(context: &AttackPriorityContext, candidate: &TargetCandidate) -> TargetRank {
    let (priority_bucket, target_group_order, weapon_fit_order) =
        if let Some(order) = tank_immediate_threat_order(context, candidate) {
            (0, order, 0)
        } else if let Some(order) = vehicle_route_obstruction_order(context, candidate) {
            (0, order, 0)
        } else {
            (
                1,
                default_target_group_order(context, candidate),
                default_weapon_fit_order(context, candidate),
            )
        };

    TargetRank {
        priority_bucket,
        target_group_order,
        weapon_fit_order,
        retention_order: retention_order(context, candidate),
        distance_sq: candidate.distance_sq,
        id: candidate.id,
    }
}

fn retention_order(context: &AttackPriorityContext, candidate: &TargetCandidate) -> u8 {
    if context.can_retain_moving_target && candidate.retained_target {
        0
    } else {
        1
    }
}

fn tank_immediate_threat_order(
    context: &AttackPriorityContext,
    candidate: &TargetCandidate,
) -> Option<u8> {
    if context.attacker_kind != EntityKind::Tank || !candidate.in_weapon_range {
        return None;
    }
    if candidate.kind == EntityKind::AntiTankGun {
        Some(0)
    } else {
        match candidate.threat_role {
            combat_rules::TargetThreatRole::AntiArmorThreat => Some(1),
            combat_rules::TargetThreatRole::FieldObstacle
                if candidate.kind == EntityKind::TankTrap
                    && candidate.tank_trap_obstructs_vehicle_route =>
            {
                Some(2)
            }
            combat_rules::TargetThreatRole::SupportWeapon => Some(3),
            combat_rules::TargetThreatRole::FieldObstacle
            | combat_rules::TargetThreatRole::Ordinary => None,
        }
    }
}

fn vehicle_route_obstruction_order(
    context: &AttackPriorityContext,
    candidate: &TargetCandidate,
) -> Option<u8> {
    if !context.attacker_is_vehicle_body
        || candidate.kind != EntityKind::TankTrap
        || !candidate.tank_trap_obstructs_vehicle_route
    {
        return None;
    }

    if context.attacker_kind == EntityKind::Tank {
        None
    } else {
        Some(0)
    }
}

fn default_weapon_fit_order(context: &AttackPriorityContext, candidate: &TargetCandidate) -> u8 {
    if context.attacker_kind == EntityKind::Tank && !candidate.in_weapon_range {
        return 3;
    }
    match combat_rules::default_weapon_target_fit(
        context.attacker_weapon_class,
        candidate.armor_class,
        candidate.threat_role,
    ) {
        combat_rules::WeaponTargetFit::PreferredThreat => {
            if candidate.kind == EntityKind::AntiTankGun {
                0
            } else {
                1
            }
        }
        combat_rules::WeaponTargetFit::PreferredArmor => 2,
        combat_rules::WeaponTargetFit::PreferredSoft => 0,
        combat_rules::WeaponTargetFit::Fallback => 3,
    }
}

fn default_target_group_order(context: &AttackPriorityContext, candidate: &TargetCandidate) -> u8 {
    if context.attacker_is_unit && candidate.is_unit {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(attacker_kind: EntityKind) -> AttackPriorityContext {
        AttackPriorityContext {
            attacker_kind,
            attacker_is_unit: true,
            attacker_is_vehicle_body: matches!(
                attacker_kind,
                EntityKind::Tank
                    | EntityKind::ScoutCar
                    | EntityKind::AntiTankGun
                    | EntityKind::Artillery
                    | EntityKind::CommandCar
                    | EntityKind::MortarTeam
            ),
            attacker_weapon_class: combat_rules::weapon_class(attacker_kind),
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
            threat_role: combat_rules::target_threat_role(kind),
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
    fn unit_attackers_prefer_units_over_armored_buildings() {
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
}
