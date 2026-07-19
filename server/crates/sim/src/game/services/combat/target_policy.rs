use crate::game::entity::EntityKind;
use crate::rules::combat as combat_rules;
use crate::rules::defs::WeaponClass;
use crate::rules::target::TargetFacts;

#[derive(Debug, Clone, Copy)]
pub(super) struct TargetPriorityPolicy {
    immediate_threats: ImmediateThreatPolicy,
    route_obstruction: RouteObstructionPolicy,
    target_group: TargetGroupPolicy,
    weapon_fit: WeaponFitPolicy,
    retention: RetentionPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImmediateThreatPolicy {
    None,
    TankCannon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RouteObstructionPolicy {
    None,
    VehicleTankTrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetGroupPolicy {
    CombatUnitsThenEconomyUnitsThenNonUnits,
    CoaxCombatInfantryThenEconomyUnitsThenFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WeaponFitPolicy {
    DefaultWeapon,
    TankCannonDefaultWeapon,
    TankCoaxMachineGun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RetentionPolicy {
    None,
    MovingFireEqualRank,
}

impl TargetPriorityPolicy {
    pub(super) fn for_id(id: combat_rules::TargetPriorityPolicyId) -> Self {
        match id {
            combat_rules::TargetPriorityPolicyId::DefaultWeapon => Self {
                immediate_threats: ImmediateThreatPolicy::None,
                route_obstruction: RouteObstructionPolicy::None,
                target_group: TargetGroupPolicy::CombatUnitsThenEconomyUnitsThenNonUnits,
                weapon_fit: WeaponFitPolicy::DefaultWeapon,
                retention: RetentionPolicy::MovingFireEqualRank,
            },
            combat_rules::TargetPriorityPolicyId::VehicleDefaultWeapon => Self {
                immediate_threats: ImmediateThreatPolicy::None,
                route_obstruction: RouteObstructionPolicy::VehicleTankTrap,
                target_group: TargetGroupPolicy::CombatUnitsThenEconomyUnitsThenNonUnits,
                weapon_fit: WeaponFitPolicy::DefaultWeapon,
                retention: RetentionPolicy::MovingFireEqualRank,
            },
            combat_rules::TargetPriorityPolicyId::TankCannon => Self {
                immediate_threats: ImmediateThreatPolicy::TankCannon,
                route_obstruction: RouteObstructionPolicy::None,
                target_group: TargetGroupPolicy::CombatUnitsThenEconomyUnitsThenNonUnits,
                weapon_fit: WeaponFitPolicy::TankCannonDefaultWeapon,
                retention: RetentionPolicy::MovingFireEqualRank,
            },
            combat_rules::TargetPriorityPolicyId::TankCoaxMachineGun => Self {
                immediate_threats: ImmediateThreatPolicy::None,
                route_obstruction: RouteObstructionPolicy::None,
                target_group: TargetGroupPolicy::CoaxCombatInfantryThenEconomyUnitsThenFallback,
                weapon_fit: WeaponFitPolicy::TankCoaxMachineGun,
                retention: RetentionPolicy::None,
            },
        }
    }

    pub(super) fn allows_candidate(self, facts: TargetFacts) -> bool {
        self.weapon_fit != WeaponFitPolicy::TankCoaxMachineGun || !facts.is_resource_node
    }

    pub(super) fn immediate_threat_order(
        self,
        facts: TargetFacts,
        in_weapon_range: bool,
        tank_trap_obstructs_vehicle_route: bool,
    ) -> Option<u8> {
        if self.immediate_threats != ImmediateThreatPolicy::TankCannon || !in_weapon_range {
            return None;
        }
        if facts.kind == EntityKind::AntiTankGun {
            Some(0)
        } else {
            match facts.threat_role {
                combat_rules::TargetThreatRole::AntiArmorThreat => Some(1),
                combat_rules::TargetThreatRole::FieldObstacle
                    if facts.kind == EntityKind::TankTrap && tank_trap_obstructs_vehicle_route =>
                {
                    Some(2)
                }
                combat_rules::TargetThreatRole::SupportWeapon => Some(3),
                combat_rules::TargetThreatRole::FieldObstacle
                | combat_rules::TargetThreatRole::Ordinary => None,
            }
        }
    }

    pub(super) fn vehicle_route_obstruction_order(
        self,
        facts: TargetFacts,
        tank_trap_obstructs_vehicle_route: bool,
    ) -> Option<u8> {
        if self.route_obstruction == RouteObstructionPolicy::VehicleTankTrap
            && facts.kind == EntityKind::TankTrap
            && tank_trap_obstructs_vehicle_route
        {
            Some(0)
        } else {
            None
        }
    }

    pub(super) fn target_group_order(self, attacker_is_unit: bool, facts: TargetFacts) -> u8 {
        match self.target_group {
            TargetGroupPolicy::CombatUnitsThenEconomyUnitsThenNonUnits if attacker_is_unit => {
                if facts.is_unit && !facts.is_economy_unit {
                    0
                } else if facts.is_economy_unit {
                    1
                } else {
                    2
                }
            }
            TargetGroupPolicy::CombatUnitsThenEconomyUnitsThenNonUnits => 0,
            TargetGroupPolicy::CoaxCombatInfantryThenEconomyUnitsThenFallback
                if attacker_is_unit =>
            {
                if facts.is_coax_infantry_priority && !facts.is_economy_unit {
                    0
                } else if facts.is_economy_unit {
                    1
                } else {
                    2
                }
            }
            TargetGroupPolicy::CoaxCombatInfantryThenEconomyUnitsThenFallback => 0,
        }
    }

    pub(super) fn weapon_fit_order(
        self,
        attacker_weapon_class: WeaponClass,
        facts: TargetFacts,
        in_weapon_range: bool,
    ) -> u8 {
        match self.weapon_fit {
            WeaponFitPolicy::TankCoaxMachineGun => {
                if facts.is_coax_infantry_priority {
                    0
                } else {
                    1
                }
            }
            WeaponFitPolicy::TankCannonDefaultWeapon if !in_weapon_range => 3,
            WeaponFitPolicy::DefaultWeapon | WeaponFitPolicy::TankCannonDefaultWeapon => {
                default_weapon_fit_order(attacker_weapon_class, facts)
            }
        }
    }

    pub(super) fn retention_order(self, can_retain: bool, retained_target: bool) -> u8 {
        match self.retention {
            RetentionPolicy::MovingFireEqualRank if can_retain && retained_target => 0,
            RetentionPolicy::MovingFireEqualRank => 1,
            RetentionPolicy::None => 0,
        }
    }
}

fn default_weapon_fit_order(attacker_weapon_class: WeaponClass, facts: TargetFacts) -> u8 {
    match combat_rules::default_weapon_target_fit(
        attacker_weapon_class,
        facts.armor_class,
        facts.threat_role,
    ) {
        combat_rules::WeaponTargetFit::PreferredThreat => {
            if facts.kind == EntityKind::AntiTankGun {
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
