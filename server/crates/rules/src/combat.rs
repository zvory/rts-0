//! Pure combat rules: classification predicates and damage formula.

use crate::defs::{self, ArmorClass, WeaponClass};
use crate::terrain::{self, TerrainKind};
use crate::{movement_body_class, EntityKind, MovementBodyClass};
use serde::{Deserialize, Serialize};

const FRONT_ARC_RAD: f32 = std::f32::consts::FRAC_PI_4;
const SIDE_ARC_RAD: f32 = std::f32::consts::PI * 3.0 / 4.0;
const FRONT_ARMOR_DAMAGE_MULTIPLIER: f32 = 1.0;
const SIDE_ARMOR_DAMAGE_MULTIPLIER: f32 = 1.5;
const REAR_ARMOR_DAMAGE_MULTIPLIER: f32 = 1.7;
const NO_ARMOR_PENETRATION: f32 = 0.0;
const FULL_ARMOR_PENETRATION: f32 = 1.0;
/// A tank keeps its hull-facing preference for this long after the latest qualifying direct-AP hit.
pub const TANK_ARMOR_REACTION_LOCK_TICKS: u32 = crate::balance::TICK_HZ * 3;

/// Attack profile for a combat-capable unit or building.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackProfile {
    pub range_tiles: u32,
    pub dmg: u32,
    pub cooldown: u32,
}

impl AttackProfile {
    pub const NONE: AttackProfile = AttackProfile {
        range_tiles: 0,
        dmg: 0,
        cooldown: 0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum WeaponKind {
    WorkerTools,
    GolemFists,
    RiflemanRifle,
    MachineGunnerMg,
    ScoutCarMg,
    AntiTankGun,
    PanzerfaustLoadedShot,
    MortarTeamMortar,
    ArtilleryGun,
    TankCannon,
    /// Tank coaxial machine gun. Tanks fire this as a secondary weapon.
    TankCoax,
}

impl WeaponKind {
    pub const ALL: [WeaponKind; 11] = [
        WeaponKind::WorkerTools,
        WeaponKind::GolemFists,
        WeaponKind::RiflemanRifle,
        WeaponKind::MachineGunnerMg,
        WeaponKind::ScoutCarMg,
        WeaponKind::AntiTankGun,
        WeaponKind::PanzerfaustLoadedShot,
        WeaponKind::MortarTeamMortar,
        WeaponKind::ArtilleryGun,
        WeaponKind::TankCannon,
        WeaponKind::TankCoax,
    ];

    pub fn stable_id(self) -> &'static str {
        match self {
            WeaponKind::WorkerTools => "worker_tools",
            WeaponKind::GolemFists => "golem_fists",
            WeaponKind::RiflemanRifle => "rifleman_rifle",
            WeaponKind::MachineGunnerMg => "machine_gunner_mg",
            WeaponKind::ScoutCarMg => "scout_car_mg",
            WeaponKind::AntiTankGun => "anti_tank_gun",
            WeaponKind::PanzerfaustLoadedShot => "panzerfaust_loaded_shot",
            WeaponKind::MortarTeamMortar => "mortar_team_mortar",
            WeaponKind::ArtilleryGun => "artillery_gun",
            WeaponKind::TankCannon => "tank_cannon",
            WeaponKind::TankCoax => "tank_coax",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissPolicy {
    None,
    AntiTankGunVsInfantrySized,
    TankCannonVsInfantry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FacingDamagePolicy {
    None,
    TankArmorFacing,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverpenetrationPolicy {
    None,
    DirectFire { range_factor: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponProfile {
    pub id: WeaponKind,
    pub range_tiles: u32,
    pub dmg: u32,
    pub cooldown: u32,
    pub weapon_class: WeaponClass,
    pub armor_penetration: f32,
    pub miss_policy: MissPolicy,
    pub facing_damage_policy: FacingDamagePolicy,
    pub overpenetration: OverpenetrationPolicy,
}

impl WeaponProfile {
    pub fn attack_profile(&self) -> AttackProfile {
        AttackProfile {
            range_tiles: self.range_tiles,
            dmg: self.dmg,
            cooldown: self.cooldown,
        }
    }
}

pub const WEAPON_PROFILES: &[WeaponProfile] = &[
    WeaponProfile {
        id: WeaponKind::WorkerTools,
        range_tiles: 1,
        dmg: 4,
        cooldown: 24,
        weapon_class: WeaponClass::SmallArms,
        armor_penetration: NO_ARMOR_PENETRATION,
        miss_policy: MissPolicy::None,
        facing_damage_policy: FacingDamagePolicy::None,
        overpenetration: OverpenetrationPolicy::DirectFire { range_factor: 0.25 },
    },
    WeaponProfile {
        id: WeaponKind::GolemFists,
        range_tiles: 1,
        dmg: 16,
        cooldown: 24,
        weapon_class: WeaponClass::SmallArms,
        armor_penetration: NO_ARMOR_PENETRATION,
        miss_policy: MissPolicy::None,
        facing_damage_policy: FacingDamagePolicy::None,
        overpenetration: OverpenetrationPolicy::DirectFire { range_factor: 0.25 },
    },
    WeaponProfile {
        id: WeaponKind::RiflemanRifle,
        range_tiles: 5,
        dmg: 5,
        cooldown: 16,
        weapon_class: WeaponClass::SmallArms,
        armor_penetration: NO_ARMOR_PENETRATION,
        miss_policy: MissPolicy::None,
        facing_damage_policy: FacingDamagePolicy::None,
        overpenetration: OverpenetrationPolicy::DirectFire { range_factor: 0.25 },
    },
    WeaponProfile {
        id: WeaponKind::MachineGunnerMg,
        range_tiles: 6,
        dmg: 4,
        cooldown: 6,
        weapon_class: WeaponClass::SmallArms,
        armor_penetration: NO_ARMOR_PENETRATION,
        miss_policy: MissPolicy::None,
        facing_damage_policy: FacingDamagePolicy::None,
        overpenetration: OverpenetrationPolicy::DirectFire { range_factor: 0.25 },
    },
    WeaponProfile {
        id: WeaponKind::ScoutCarMg,
        range_tiles: 7,
        dmg: 6,
        cooldown: 6,
        weapon_class: WeaponClass::SmallArms,
        armor_penetration: NO_ARMOR_PENETRATION,
        miss_policy: MissPolicy::None,
        facing_damage_policy: FacingDamagePolicy::None,
        overpenetration: OverpenetrationPolicy::DirectFire { range_factor: 0.25 },
    },
    WeaponProfile {
        id: WeaponKind::AntiTankGun,
        range_tiles: 5,
        dmg: 100,
        cooldown: 72,
        weapon_class: WeaponClass::AntiTank,
        armor_penetration: FULL_ARMOR_PENETRATION,
        miss_policy: MissPolicy::AntiTankGunVsInfantrySized,
        facing_damage_policy: FacingDamagePolicy::TankArmorFacing,
        overpenetration: OverpenetrationPolicy::DirectFire { range_factor: 0.50 },
    },
    WeaponProfile {
        id: WeaponKind::PanzerfaustLoadedShot,
        range_tiles: crate::balance::PANZERFAUST_RANGE_TILES,
        dmg: crate::balance::PANZERFAUST_DAMAGE,
        cooldown: 0,
        weapon_class: WeaponClass::AntiTank,
        armor_penetration: crate::balance::PANZERFAUST_ARMOR_PENETRATION,
        miss_policy: MissPolicy::None,
        facing_damage_policy: FacingDamagePolicy::None,
        overpenetration: OverpenetrationPolicy::None,
    },
    WeaponProfile {
        id: WeaponKind::MortarTeamMortar,
        range_tiles: crate::balance::MORTAR_RANGE_TILES,
        dmg: crate::balance::MORTAR_OUTER_DAMAGE,
        cooldown: 60,
        weapon_class: WeaponClass::SmallArms,
        armor_penetration: NO_ARMOR_PENETRATION,
        miss_policy: MissPolicy::None,
        facing_damage_policy: FacingDamagePolicy::None,
        overpenetration: OverpenetrationPolicy::None,
    },
    WeaponProfile {
        id: WeaponKind::ArtilleryGun,
        range_tiles: crate::balance::ARTILLERY_MAX_RANGE_TILES,
        dmg: 0,
        cooldown: crate::balance::ARTILLERY_RELOAD_TICKS,
        weapon_class: WeaponClass::None,
        armor_penetration: NO_ARMOR_PENETRATION,
        miss_policy: MissPolicy::None,
        facing_damage_policy: FacingDamagePolicy::None,
        overpenetration: OverpenetrationPolicy::None,
    },
    WeaponProfile {
        id: WeaponKind::TankCannon,
        range_tiles: 5,
        dmg: 60,
        cooldown: 72,
        weapon_class: WeaponClass::AntiTank,
        armor_penetration: FULL_ARMOR_PENETRATION,
        miss_policy: MissPolicy::TankCannonVsInfantry,
        facing_damage_policy: FacingDamagePolicy::TankArmorFacing,
        overpenetration: OverpenetrationPolicy::DirectFire { range_factor: 0.25 },
    },
    WeaponProfile {
        id: WeaponKind::TankCoax,
        range_tiles: 6,
        dmg: 4,
        cooldown: 6,
        weapon_class: WeaponClass::SmallArms,
        armor_penetration: NO_ARMOR_PENETRATION,
        miss_policy: MissPolicy::None,
        facing_damage_policy: FacingDamagePolicy::None,
        overpenetration: OverpenetrationPolicy::DirectFire { range_factor: 0.25 },
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmorFacing {
    Front,
    Side,
    Rear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetThreatRole {
    Ordinary,
    AntiArmorThreat,
    FieldObstacle,
    SupportWeapon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponTargetFit {
    PreferredThreat,
    PreferredArmor,
    PreferredSoft,
    Fallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TargetPriorityPolicyId {
    DefaultWeapon,
    VehicleDefaultWeapon,
    TankCannon,
    TankCoaxMachineGun,
}

impl TargetPriorityPolicyId {
    pub fn stable_id(self) -> &'static str {
        match self {
            TargetPriorityPolicyId::DefaultWeapon => "default_weapon",
            TargetPriorityPolicyId::VehicleDefaultWeapon => "vehicle_default_weapon",
            TargetPriorityPolicyId::TankCannon => "tank_cannon",
            TargetPriorityPolicyId::TankCoaxMachineGun => "tank_coax_machine_gun",
        }
    }
}

pub fn weapon_profile(kind: WeaponKind) -> Option<&'static WeaponProfile> {
    WEAPON_PROFILES.iter().find(|profile| profile.id == kind)
}

pub fn default_weapon_kind(kind: EntityKind) -> Option<WeaponKind> {
    match kind {
        EntityKind::Worker => Some(WeaponKind::WorkerTools),
        EntityKind::Golem => Some(WeaponKind::GolemFists),
        EntityKind::Rifleman | EntityKind::Panzerfaust => Some(WeaponKind::RiflemanRifle),
        EntityKind::MachineGunner => Some(WeaponKind::MachineGunnerMg),
        EntityKind::AntiTankGun => Some(WeaponKind::AntiTankGun),
        EntityKind::MortarTeam => Some(WeaponKind::MortarTeamMortar),
        EntityKind::Artillery => Some(WeaponKind::ArtilleryGun),
        EntityKind::ScoutCar => Some(WeaponKind::ScoutCarMg),
        EntityKind::Tank => Some(WeaponKind::TankCannon),
        EntityKind::ScoutPlane
        | EntityKind::CommandCar
        | EntityKind::Ekat
        | EntityKind::CityCentre
        | EntityKind::Zamok
        | EntityKind::Depot
        | EntityKind::Barracks
        | EntityKind::TrainingCentre
        | EntityKind::ResearchComplex
        | EntityKind::Factory
        | EntityKind::Steelworks
        | EntityKind::TankTrap
        | EntityKind::PumpJack
        | EntityKind::Steel
        | EntityKind::Oil => None,
    }
}

pub fn default_weapon_profile(kind: EntityKind) -> Option<&'static WeaponProfile> {
    default_weapon_kind(kind).and_then(weapon_profile)
}

pub fn default_target_priority_policy(kind: EntityKind) -> TargetPriorityPolicyId {
    if kind == EntityKind::Tank {
        TargetPriorityPolicyId::TankCannon
    } else if movement_body_class(kind) == MovementBodyClass::VehicleBody {
        TargetPriorityPolicyId::VehicleDefaultWeapon
    } else {
        TargetPriorityPolicyId::DefaultWeapon
    }
}

/// Returns the attack profile for the given kind, or zeroes if non-combatant.
pub fn attack_profile(kind: EntityKind) -> AttackProfile {
    default_weapon_profile(kind)
        .map(WeaponProfile::attack_profile)
        .unwrap_or(AttackProfile::NONE)
}

/// Armored targets take 75% damage reduction from non-AP weapons.
pub fn is_armored(kind: EntityKind) -> bool {
    armor_class(kind) == Some(ArmorClass::Armored)
}

/// Weapons with non-zero armor penetration count as AP threats for target ranking.
pub fn is_ap(kind: EntityKind) -> bool {
    default_weapon_profile(kind).is_some_and(weapon_is_ap)
}

pub fn weapon_is_ap(profile: &WeaponProfile) -> bool {
    profile.armor_penetration > 0.0
}

/// The current direct-fire AP weapons that make a Tank commit its hull to their source.
pub fn weapon_triggers_tank_armor_reaction(profile: &WeaponProfile) -> bool {
    weapon_is_ap(profile)
        && matches!(
            profile.id,
            WeaponKind::AntiTankGun | WeaponKind::PanzerfaustLoadedShot | WeaponKind::TankCannon
        )
}

/// Whether this unit uses the autonomous first-hit armor-reaction lock.
pub fn unit_uses_tank_armor_reaction(kind: EntityKind) -> bool {
    kind == EntityKind::Tank
}

/// Rules-owned armor classification for target ranking and damage policy.
pub fn armor_class(kind: EntityKind) -> Option<ArmorClass> {
    defs::unit_def(kind)
        .map(|d| d.armor_class)
        .or_else(|| defs::building_def(kind).map(|d| d.armor_class))
}

/// Rules-owned weapon classification for target ranking and threat policy.
pub fn weapon_class(kind: EntityKind) -> WeaponClass {
    default_weapon_profile(kind)
        .map(|profile| profile.weapon_class)
        .unwrap_or(WeaponClass::None)
}

/// Rules-owned threat role used by sim-local target ranking.
pub fn target_threat_role(kind: EntityKind) -> TargetThreatRole {
    if is_ap(kind) {
        TargetThreatRole::AntiArmorThreat
    } else if kind == EntityKind::TankTrap {
        TargetThreatRole::FieldObstacle
    } else if matches!(kind, EntityKind::MortarTeam | EntityKind::Artillery) {
        TargetThreatRole::SupportWeapon
    } else {
        TargetThreatRole::Ordinary
    }
}

/// Loaded Panzerfaust target filter. The runtime consumes this separately from the normal rifle
/// profile so the one disposable anti-armor shot keeps its own target priority.
pub fn is_panzerfaust_loaded_shot_target(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::ScoutCar | EntityKind::Tank | EntityKind::CommandCar
    )
}

/// Pure default-weapon fit vocabulary for target ranking.
pub fn default_weapon_target_fit(
    attacker_weapon: WeaponClass,
    target_armor: Option<ArmorClass>,
    target_role: TargetThreatRole,
) -> WeaponTargetFit {
    match attacker_weapon {
        WeaponClass::SmallArms => {
            if target_armor == Some(ArmorClass::Small) {
                WeaponTargetFit::PreferredSoft
            } else {
                WeaponTargetFit::Fallback
            }
        }
        WeaponClass::AntiTank => {
            if target_role == TargetThreatRole::AntiArmorThreat {
                WeaponTargetFit::PreferredThreat
            } else if target_armor == Some(ArmorClass::Armored) {
                WeaponTargetFit::PreferredArmor
            } else {
                WeaponTargetFit::Fallback
            }
        }
        WeaponClass::None => WeaponTargetFit::Fallback,
    }
}

/// Miss probability [0.0, 1.0) for an attack. Anti-Tank Gun shells have a 90% miss rate against
/// infantry-sized targets, while Tank cannon shells have a 50% miss rate against humanoid infantry.
/// A miss flies straight through without finding anyone; hits that connect deal full damage.
pub fn miss_chance(attacker_kind: EntityKind, victim_kind: EntityKind) -> f32 {
    default_weapon_profile(attacker_kind)
        .map(|profile| miss_chance_for_weapon(profile, victim_kind))
        .unwrap_or(0.0)
}

pub fn miss_chance_for_weapon(profile: &WeaponProfile, victim_kind: EntityKind) -> f32 {
    match profile.miss_policy {
        MissPolicy::AntiTankGunVsInfantrySized if anti_tank_gun_miss_target(victim_kind) => 0.90,
        MissPolicy::TankCannonVsInfantry if tank_cannon_miss_target(victim_kind) => 0.50,
        _ => 0.0,
    }
}

/// Applies the shared direct-damage reduction for actively entrenched eligible infantry.
pub fn direct_damage_after_entrenchment(
    victim_kind: EntityKind,
    damage: u32,
    victim_actively_entrenched: bool,
) -> u32 {
    if !victim_actively_entrenched
        || !crate::balance::is_entrenchment_eligible_infantry(victim_kind)
    {
        return damage;
    }
    let multiplier = (1.0 - crate::balance::ENTRENCHMENT_DIRECT_DAMAGE_REDUCTION).clamp(0.0, 1.0);
    ((damage as f32) * multiplier).round().max(0.0) as u32
}

/// Applies the shared area-damage reduction for actively entrenched eligible infantry.
pub fn area_damage_after_entrenchment(
    victim_kind: EntityKind,
    damage: u32,
    victim_actively_entrenched: bool,
) -> u32 {
    if !victim_actively_entrenched
        || !crate::balance::is_entrenchment_eligible_infantry(victim_kind)
    {
        return damage;
    }
    let multiplier = (1.0 - crate::balance::ENTRENCHMENT_AREA_DAMAGE_REDUCTION).clamp(0.0, 1.0);
    ((damage as f32) * multiplier).round().max(0.0) as u32
}

fn anti_tank_gun_miss_target(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Worker
            | EntityKind::Golem
            | EntityKind::Rifleman
            | EntityKind::Panzerfaust
            | EntityKind::MachineGunner
    )
}

fn tank_cannon_miss_target(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Worker
            | EntityKind::Rifleman
            | EntityKind::Panzerfaust
            | EntityKind::MachineGunner
    )
}

/// Applies the AP/armor damage formula. The miss_chance roll is handled at the call site.
pub fn effective_damage(
    attacker_kind: EntityKind,
    victim_kind: EntityKind,
    base_dmg: u32,
    victim_terrain: Option<TerrainKind>,
) -> u32 {
    default_weapon_profile(attacker_kind)
        .map(|profile| effective_damage_for_weapon(profile, victim_kind, base_dmg, victim_terrain))
        .unwrap_or_else(|| {
            effective_damage_for_weapon_class(
                WeaponClass::None,
                victim_kind,
                base_dmg,
                victim_terrain,
            )
        })
}

pub fn panzerfaust_loaded_shot_damage(
    victim_kind: EntityKind,
    victim_terrain: Option<TerrainKind>,
) -> u32 {
    weapon_profile(WeaponKind::PanzerfaustLoadedShot)
        .map(|profile| {
            effective_damage_for_weapon(
                profile,
                victim_kind,
                crate::balance::PANZERFAUST_DAMAGE,
                victim_terrain,
            )
        })
        .unwrap_or(0)
}

pub fn effective_damage_for_weapon(
    profile: &WeaponProfile,
    victim_kind: EntityKind,
    base_dmg: u32,
    victim_terrain: Option<TerrainKind>,
) -> u32 {
    effective_damage_for_armor_penetration(
        profile.armor_penetration,
        victim_kind,
        base_dmg,
        victim_terrain,
    )
}

fn effective_damage_for_weapon_class(
    attacker_weapon_class: WeaponClass,
    victim_kind: EntityKind,
    base_dmg: u32,
    victim_terrain: Option<TerrainKind>,
) -> u32 {
    effective_damage_for_armor_penetration(
        if attacker_weapon_class == WeaponClass::AntiTank {
            FULL_ARMOR_PENETRATION
        } else {
            NO_ARMOR_PENETRATION
        },
        victim_kind,
        base_dmg,
        victim_terrain,
    )
}

fn effective_damage_for_armor_penetration(
    armor_penetration: f32,
    victim_kind: EntityKind,
    base_dmg: u32,
    victim_terrain: Option<TerrainKind>,
) -> u32 {
    let armor_class = armor_class(victim_kind);
    let penetration = armor_penetration.clamp(0.0, 1.0);
    let armor_adjusted = match armor_class {
        Some(ArmorClass::Armored) if penetration <= 0.0 => base_dmg / 4,
        Some(ArmorClass::Armored) => {
            let multiplier = 0.25 + (0.75 * penetration);
            ((base_dmg as f32) * multiplier).round() as u32
        }
        _ => base_dmg,
    };
    let terrain = victim_terrain.unwrap_or(TerrainKind::Open);
    (armor_adjusted as f32 * terrain::cover_modifier(victim_kind, terrain)).round() as u32
}

pub fn classify_armor_facing(
    victim_facing: f32,
    victim_pos: (f32, f32),
    attacker_pos: (f32, f32),
) -> ArmorFacing {
    let attacker_angle = (attacker_pos.1 - victim_pos.1).atan2(attacker_pos.0 - victim_pos.0);
    let angle_error = normalized_angle_delta(attacker_angle, victim_facing).abs();
    if angle_error <= FRONT_ARC_RAD {
        ArmorFacing::Front
    } else if angle_error <= SIDE_ARC_RAD {
        ArmorFacing::Side
    } else {
        ArmorFacing::Rear
    }
}

pub fn facing_damage_multiplier(
    attacker_kind: EntityKind,
    victim_kind: EntityKind,
    facing: ArmorFacing,
) -> f32 {
    default_weapon_profile(attacker_kind)
        .map(|profile| facing_damage_multiplier_for_weapon(profile, victim_kind, facing))
        .unwrap_or(1.0)
}

pub fn facing_damage_multiplier_for_weapon(
    profile: &WeaponProfile,
    victim_kind: EntityKind,
    facing: ArmorFacing,
) -> f32 {
    if victim_kind != EntityKind::Tank {
        return 1.0;
    }
    if profile.facing_damage_policy != FacingDamagePolicy::TankArmorFacing {
        return 1.0;
    }
    match facing {
        ArmorFacing::Front => FRONT_ARMOR_DAMAGE_MULTIPLIER,
        ArmorFacing::Side => SIDE_ARMOR_DAMAGE_MULTIPLIER,
        ArmorFacing::Rear => REAR_ARMOR_DAMAGE_MULTIPLIER,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn effective_damage_with_facing(
    attacker_kind: EntityKind,
    victim_kind: EntityKind,
    base_dmg: u32,
    victim_terrain: Option<TerrainKind>,
    victim_facing: Option<f32>,
    victim_pos: (f32, f32),
    attacker_pos: (f32, f32),
) -> u32 {
    default_weapon_profile(attacker_kind)
        .map(|profile| {
            effective_damage_with_facing_for_weapon(
                profile,
                victim_kind,
                base_dmg,
                victim_terrain,
                victim_facing,
                victim_pos,
                attacker_pos,
            )
        })
        .unwrap_or_else(|| {
            let damage = effective_damage_for_weapon_class(
                WeaponClass::None,
                victim_kind,
                base_dmg,
                victim_terrain,
            );
            apply_facing_damage_multiplier(
                damage,
                None,
                victim_kind,
                victim_facing,
                victim_pos,
                attacker_pos,
            )
        })
}

#[allow(clippy::too_many_arguments)]
pub fn effective_damage_with_facing_for_weapon(
    profile: &WeaponProfile,
    victim_kind: EntityKind,
    base_dmg: u32,
    victim_terrain: Option<TerrainKind>,
    victim_facing: Option<f32>,
    victim_pos: (f32, f32),
    attacker_pos: (f32, f32),
) -> u32 {
    let damage = effective_damage_for_weapon(profile, victim_kind, base_dmg, victim_terrain);
    apply_facing_damage_multiplier(
        damage,
        Some(profile),
        victim_kind,
        victim_facing,
        victim_pos,
        attacker_pos,
    )
}

fn apply_facing_damage_multiplier(
    damage: u32,
    profile: Option<&WeaponProfile>,
    victim_kind: EntityKind,
    victim_facing: Option<f32>,
    victim_pos: (f32, f32),
    attacker_pos: (f32, f32),
) -> u32 {
    let Some(victim_facing) = victim_facing.filter(|facing| facing.is_finite()) else {
        return damage;
    };
    let facing = classify_armor_facing(victim_facing, victim_pos, attacker_pos);
    let multiplier = profile
        .map(|profile| facing_damage_multiplier_for_weapon(profile, victim_kind, facing))
        .unwrap_or(1.0);
    ((damage as f32) * multiplier).round().max(0.0) as u32
}

fn normalized_angle_delta(from: f32, to: f32) -> f32 {
    let two_pi = std::f32::consts::TAU;
    (from - to + std::f32::consts::PI).rem_euclid(two_pi) - std::f32::consts::PI
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tank_armor_reaction_policy_has_one_unit_and_three_ap_weapons() {
        let reacting_units = EntityKind::ALL
            .into_iter()
            .filter(|kind| unit_uses_tank_armor_reaction(*kind))
            .collect::<Vec<_>>();
        assert_eq!(reacting_units, vec![EntityKind::Tank]);

        let triggering_weapons = WEAPON_PROFILES
            .iter()
            .filter(|profile| weapon_triggers_tank_armor_reaction(profile))
            .map(|profile| profile.id)
            .collect::<Vec<_>>();
        assert_eq!(
            triggering_weapons,
            vec![
                WeaponKind::AntiTankGun,
                WeaponKind::PanzerfaustLoadedShot,
                WeaponKind::TankCannon,
            ]
        );
        assert!(WEAPON_PROFILES
            .iter()
            .filter(|profile| weapon_triggers_tank_armor_reaction(profile))
            .all(weapon_is_ap));
    }

    fn defs_attack_profile_and_class(kind: EntityKind) -> (AttackProfile, WeaponClass) {
        if let Some(def) = defs::unit_def(kind) {
            (
                AttackProfile {
                    range_tiles: def.stats.range_tiles,
                    dmg: def.stats.dmg,
                    cooldown: def.stats.cooldown,
                },
                def.weapon,
            )
        } else if let Some(def) = defs::building_def(kind) {
            (
                AttackProfile {
                    range_tiles: def.stats.range_tiles,
                    dmg: def.stats.dmg,
                    cooldown: def.stats.cooldown,
                },
                def.weapon,
            )
        } else {
            (AttackProfile::NONE, WeaponClass::None)
        }
    }

    #[test]
    fn weapon_kind_stable_ids_cover_current_profiles_and_reserved_coax() {
        let expected = [
            (WeaponKind::WorkerTools, "worker_tools"),
            (WeaponKind::GolemFists, "golem_fists"),
            (WeaponKind::RiflemanRifle, "rifleman_rifle"),
            (WeaponKind::MachineGunnerMg, "machine_gunner_mg"),
            (WeaponKind::ScoutCarMg, "scout_car_mg"),
            (WeaponKind::AntiTankGun, "anti_tank_gun"),
            (WeaponKind::PanzerfaustLoadedShot, "panzerfaust_loaded_shot"),
            (WeaponKind::MortarTeamMortar, "mortar_team_mortar"),
            (WeaponKind::ArtilleryGun, "artillery_gun"),
            (WeaponKind::TankCannon, "tank_cannon"),
            (WeaponKind::TankCoax, "tank_coax"),
        ];

        assert_eq!(WeaponKind::ALL.len(), expected.len());
        for (kind, stable_id) in expected {
            assert!(WeaponKind::ALL.contains(&kind), "{kind:?} missing from ALL");
            assert_eq!(kind.stable_id(), stable_id);
        }
        for (index, kind) in WeaponKind::ALL.iter().enumerate() {
            for other in &WeaponKind::ALL[index + 1..] {
                assert_ne!(
                    kind.stable_id(),
                    other.stable_id(),
                    "weapon stable ids must be unique"
                );
            }
        }
    }

    #[test]
    fn default_weapon_profile_ids_match_current_combat_entities() {
        let expected = [
            (EntityKind::Worker, Some(WeaponKind::WorkerTools)),
            (EntityKind::Golem, Some(WeaponKind::GolemFists)),
            (EntityKind::Rifleman, Some(WeaponKind::RiflemanRifle)),
            (EntityKind::Panzerfaust, Some(WeaponKind::RiflemanRifle)),
            (EntityKind::MachineGunner, Some(WeaponKind::MachineGunnerMg)),
            (EntityKind::AntiTankGun, Some(WeaponKind::AntiTankGun)),
            (EntityKind::MortarTeam, Some(WeaponKind::MortarTeamMortar)),
            (EntityKind::Artillery, Some(WeaponKind::ArtilleryGun)),
            (EntityKind::ScoutCar, Some(WeaponKind::ScoutCarMg)),
            (EntityKind::ScoutPlane, None),
            (EntityKind::Tank, Some(WeaponKind::TankCannon)),
            (EntityKind::CommandCar, None),
            (EntityKind::Ekat, None),
            (EntityKind::CityCentre, None),
            (EntityKind::Zamok, None),
            (EntityKind::Depot, None),
            (EntityKind::Barracks, None),
            (EntityKind::TrainingCentre, None),
            (EntityKind::ResearchComplex, None),
            (EntityKind::Factory, None),
            (EntityKind::Steelworks, None),
            (EntityKind::TankTrap, None),
            (EntityKind::PumpJack, None),
            (EntityKind::Steel, None),
            (EntityKind::Oil, None),
        ];

        assert_eq!(EntityKind::ALL.len(), expected.len());
        for (kind, weapon_kind) in expected {
            assert_eq!(
                default_weapon_kind(kind),
                weapon_kind,
                "{kind} default weapon"
            );
            assert_eq!(
                default_weapon_profile(kind).map(|profile| profile.id),
                weapon_kind,
                "{kind} default weapon profile"
            );
        }
    }

    #[test]
    fn default_weapon_profiles_match_legacy_attack_values_and_classes() {
        for kind in EntityKind::ALL {
            let (expected_profile, expected_class) = defs_attack_profile_and_class(kind);

            assert_eq!(
                attack_profile(kind),
                expected_profile,
                "{kind} attack profile"
            );
            assert_eq!(weapon_class(kind), expected_class, "{kind} weapon class");

            if let Some(profile) = default_weapon_profile(kind) {
                assert_eq!(
                    profile.attack_profile(),
                    expected_profile,
                    "{kind} profile attack values"
                );
                assert_eq!(
                    profile.weapon_class, expected_class,
                    "{kind} profile weapon class"
                );
            } else {
                assert_eq!(
                    expected_profile,
                    AttackProfile::NONE,
                    "{kind} without a profile must keep zero attack"
                );
                assert_eq!(
                    expected_class,
                    WeaponClass::None,
                    "{kind} without a profile must keep no weapon class"
                );
            }
        }
    }

    #[test]
    fn weapon_profile_metadata_preserves_current_special_damage_policies() {
        let anti_tank_gun = weapon_profile(WeaponKind::AntiTankGun).expect("AT gun profile");
        assert_eq!(anti_tank_gun.armor_penetration, FULL_ARMOR_PENETRATION);
        assert_eq!(
            anti_tank_gun.miss_policy,
            MissPolicy::AntiTankGunVsInfantrySized
        );
        assert_eq!(
            anti_tank_gun.facing_damage_policy,
            FacingDamagePolicy::TankArmorFacing
        );
        assert_eq!(
            anti_tank_gun.overpenetration,
            OverpenetrationPolicy::DirectFire { range_factor: 0.50 }
        );

        let tank_cannon = weapon_profile(WeaponKind::TankCannon).expect("tank cannon profile");
        assert_eq!(tank_cannon.armor_penetration, FULL_ARMOR_PENETRATION);
        assert_eq!(tank_cannon.miss_policy, MissPolicy::TankCannonVsInfantry);
        assert_eq!(
            tank_cannon.facing_damage_policy,
            FacingDamagePolicy::TankArmorFacing
        );
        assert_eq!(
            tank_cannon.overpenetration,
            OverpenetrationPolicy::DirectFire { range_factor: 0.25 }
        );

        let machine_gunner = weapon_profile(WeaponKind::MachineGunnerMg).expect("MG profile");
        assert_eq!(machine_gunner.weapon_class, WeaponClass::SmallArms);
        assert_eq!(machine_gunner.armor_penetration, NO_ARMOR_PENETRATION);
        assert_eq!(machine_gunner.range_tiles, 6);
        assert_eq!(machine_gunner.dmg, 4);
        assert_eq!(machine_gunner.cooldown, 6);

        let scout_car = weapon_profile(WeaponKind::ScoutCarMg).expect("Scout Car MG profile");
        assert_eq!(scout_car.weapon_class, WeaponClass::SmallArms);
        assert_eq!(scout_car.range_tiles, 7);
        assert_eq!(scout_car.dmg, 6);
        assert_eq!(scout_car.cooldown, 6);

        let tank_coax = weapon_profile(WeaponKind::TankCoax).expect("Tank coax profile");
        assert_eq!(tank_coax.weapon_class, WeaponClass::SmallArms);
        assert_eq!(tank_coax.armor_penetration, NO_ARMOR_PENETRATION);
        assert_eq!(tank_coax.range_tiles, 6);
        assert_eq!(tank_coax.dmg, 4);
        assert_eq!(tank_coax.cooldown, 6);
        assert_eq!(tank_coax.miss_policy, MissPolicy::None);
        assert_eq!(tank_coax.facing_damage_policy, FacingDamagePolicy::None);
        assert_eq!(
            tank_coax.overpenetration,
            OverpenetrationPolicy::DirectFire { range_factor: 0.25 }
        );
        assert_eq!(
            default_weapon_profile(EntityKind::Tank)
                .expect("Tank default profile")
                .id,
            WeaponKind::TankCannon,
            "Tank coax must not replace the default Tank cannon profile"
        );

        let panzerfaust =
            weapon_profile(WeaponKind::PanzerfaustLoadedShot).expect("Panzerfaust profile");
        assert_eq!(panzerfaust.weapon_class, WeaponClass::AntiTank);
        assert_eq!(
            panzerfaust.armor_penetration,
            crate::balance::PANZERFAUST_ARMOR_PENETRATION
        );
        assert_eq!(panzerfaust.facing_damage_policy, FacingDamagePolicy::None);
        assert_eq!(panzerfaust.overpenetration, OverpenetrationPolicy::None);
        assert_eq!(
            panzerfaust_loaded_shot_damage(EntityKind::Tank, None),
            effective_damage_for_weapon(
                panzerfaust,
                EntityKind::Tank,
                crate::balance::PANZERFAUST_DAMAGE,
                None,
            )
        );
    }

    #[test]
    fn weapon_policy_helpers_use_profile_metadata() {
        let tank_cannon = weapon_profile(WeaponKind::TankCannon).expect("tank cannon profile");
        let machine_gunner = weapon_profile(WeaponKind::MachineGunnerMg).expect("MG profile");
        let anti_tank_gun = weapon_profile(WeaponKind::AntiTankGun).expect("AT gun profile");

        assert_eq!(
            effective_damage_for_weapon(tank_cannon, EntityKind::Tank, 40, None),
            40
        );
        assert_eq!(
            effective_damage_for_weapon(machine_gunner, EntityKind::Tank, 40, None),
            10
        );
        assert_eq!(
            facing_damage_multiplier_for_weapon(tank_cannon, EntityKind::Tank, ArmorFacing::Rear,),
            REAR_ARMOR_DAMAGE_MULTIPLIER
        );
        assert_eq!(
            facing_damage_multiplier_for_weapon(
                machine_gunner,
                EntityKind::Tank,
                ArmorFacing::Rear,
            ),
            1.0
        );
        assert_eq!(
            miss_chance_for_weapon(anti_tank_gun, EntityKind::Rifleman),
            0.90
        );
        assert_eq!(
            miss_chance_for_weapon(tank_cannon, EntityKind::Rifleman),
            0.50
        );
    }

    #[test]
    fn ap_vs_armored_full_damage() {
        assert_eq!(
            effective_damage(EntityKind::AntiTankGun, EntityKind::Tank, 40, None),
            40
        );
    }

    #[test]
    fn non_ap_vs_armored_reduced() {
        assert_eq!(
            effective_damage(EntityKind::Rifleman, EntityKind::Tank, 40, None),
            10
        );
        assert_eq!(
            effective_damage(EntityKind::ScoutCar, EntityKind::Tank, 6, None),
            1
        );
    }

    #[test]
    fn ap_vs_small_full_damage_on_hit() {
        assert_eq!(
            effective_damage(EntityKind::AntiTankGun, EntityKind::Rifleman, 20, None),
            20
        );
    }

    #[test]
    fn non_ap_vs_unarmored_full_damage() {
        assert_eq!(
            effective_damage(EntityKind::Rifleman, EntityKind::Rifleman, 20, None),
            20
        );
    }

    #[test]
    fn artillery_uses_soft_target_damage_and_targeting_policy() {
        assert!(!is_armored(EntityKind::Artillery));
        assert_eq!(
            effective_damage(EntityKind::Rifleman, EntityKind::Artillery, 40, None),
            40
        );
        assert_eq!(
            default_weapon_target_fit(
                WeaponClass::SmallArms,
                armor_class(EntityKind::Artillery),
                target_threat_role(EntityKind::Artillery),
            ),
            WeaponTargetFit::PreferredSoft
        );
    }

    #[test]
    fn entrenched_direct_damage_reduces_only_eligible_infantry() {
        assert_eq!(
            direct_damage_after_entrenchment(EntityKind::Rifleman, 100, true),
            50
        );
        assert_eq!(
            direct_damage_after_entrenchment(EntityKind::MachineGunner, 41, true),
            21
        );
        assert_eq!(
            direct_damage_after_entrenchment(EntityKind::MortarTeam, 100, true),
            100
        );
        assert_eq!(
            direct_damage_after_entrenchment(EntityKind::Worker, 100, true),
            100
        );
        assert_eq!(
            direct_damage_after_entrenchment(EntityKind::Rifleman, 100, false),
            100
        );
    }

    #[test]
    fn entrenched_area_damage_reduces_only_eligible_infantry() {
        assert_eq!(
            area_damage_after_entrenchment(EntityKind::Rifleman, 100, true),
            75
        );
        assert_eq!(
            area_damage_after_entrenchment(EntityKind::MachineGunner, 40, true),
            30
        );
        assert_eq!(
            area_damage_after_entrenchment(EntityKind::MortarTeam, 100, true),
            100
        );
        assert_eq!(
            area_damage_after_entrenchment(EntityKind::Worker, 100, true),
            100
        );
        assert_eq!(
            area_damage_after_entrenchment(EntityKind::Rifleman, 100, false),
            100
        );
    }

    #[test]
    fn panzerfaust_loaded_shot_targets_only_real_vehicles_with_half_penetration() {
        for kind in EntityKind::ALL {
            assert_eq!(
                is_panzerfaust_loaded_shot_target(kind),
                matches!(
                    kind,
                    EntityKind::ScoutCar | EntityKind::Tank | EntityKind::CommandCar
                ),
                "{kind:?}"
            );
        }
        assert_eq!(panzerfaust_loaded_shot_damage(EntityKind::Tank, None), 63);
        assert_eq!(
            panzerfaust_loaded_shot_damage(EntityKind::ScoutCar, None),
            100
        );
    }

    #[test]
    fn tank_ap_vs_building_full_damage() {
        assert_eq!(
            effective_damage(EntityKind::Tank, EntityKind::Barracks, 50, None),
            50
        );
    }

    #[test]
    fn tank_two_hits_destroy_tank_trap() {
        let tank_trap_hp = defs::building_def(EntityKind::TankTrap)
            .expect("tank trap def")
            .stats
            .hp;
        let tank_shot = effective_damage(
            EntityKind::Tank,
            EntityKind::TankTrap,
            attack_profile(EntityKind::Tank).dmg,
            None,
        );

        assert_eq!(tank_shot, 60);
        assert!(tank_trap_hp > tank_shot, "one Tank shot should not kill");
        assert!(tank_trap_hp <= tank_shot * 2, "two Tank shots should kill");
    }

    #[test]
    fn infantry_vs_building_reduced() {
        assert_eq!(
            effective_damage(EntityKind::MachineGunner, EntityKind::Depot, 40, None),
            10
        );
    }

    #[test]
    fn infantry_vs_tank_trap_reduced_by_armor() {
        assert_eq!(
            effective_damage(EntityKind::Rifleman, EntityKind::TankTrap, 5, None),
            1
        );
    }

    #[test]
    fn open_terrain_keeps_current_damage_values() {
        assert_eq!(
            effective_damage(
                EntityKind::Rifleman,
                EntityKind::Rifleman,
                20,
                Some(TerrainKind::Open)
            ),
            20
        );
        assert_eq!(
            effective_damage(
                EntityKind::Rifleman,
                EntityKind::Tank,
                40,
                Some(TerrainKind::Open)
            ),
            10
        );
    }

    #[test]
    fn combat_classification_matches_default_weapon_policy_table() {
        let expected = [
            (EntityKind::Worker, false, false, TargetThreatRole::Ordinary),
            (EntityKind::Golem, false, false, TargetThreatRole::Ordinary),
            (
                EntityKind::Rifleman,
                false,
                false,
                TargetThreatRole::Ordinary,
            ),
            (
                EntityKind::MachineGunner,
                false,
                false,
                TargetThreatRole::Ordinary,
            ),
            (
                EntityKind::AntiTankGun,
                false,
                true,
                TargetThreatRole::AntiArmorThreat,
            ),
            (
                EntityKind::MortarTeam,
                false,
                false,
                TargetThreatRole::SupportWeapon,
            ),
            (
                EntityKind::ScoutCar,
                false,
                false,
                TargetThreatRole::Ordinary,
            ),
            (
                EntityKind::ScoutPlane,
                false,
                false,
                TargetThreatRole::Ordinary,
            ),
            (
                EntityKind::Tank,
                true,
                true,
                TargetThreatRole::AntiArmorThreat,
            ),
            (
                EntityKind::CityCentre,
                true,
                false,
                TargetThreatRole::Ordinary,
            ),
            (EntityKind::Depot, true, false, TargetThreatRole::Ordinary),
            (
                EntityKind::Barracks,
                true,
                false,
                TargetThreatRole::Ordinary,
            ),
            (
                EntityKind::TrainingCentre,
                true,
                false,
                TargetThreatRole::Ordinary,
            ),
            (EntityKind::Factory, true, false, TargetThreatRole::Ordinary),
            (
                EntityKind::Steelworks,
                true,
                false,
                TargetThreatRole::Ordinary,
            ),
            (
                EntityKind::TankTrap,
                true,
                false,
                TargetThreatRole::FieldObstacle,
            ),
            (EntityKind::Steel, false, false, TargetThreatRole::Ordinary),
            (EntityKind::Oil, false, false, TargetThreatRole::Ordinary),
        ];

        for (kind, armored, ap, threat_role) in expected {
            assert_eq!(is_armored(kind), armored, "{kind} armor classification");
            assert_eq!(is_ap(kind), ap, "{kind} AP classification");
            assert_eq!(
                target_threat_role(kind),
                threat_role,
                "{kind} target threat role"
            );
        }
    }

    #[test]
    fn default_weapon_fit_prefers_soft_or_anti_armor_targets() {
        assert_eq!(
            default_weapon_target_fit(
                WeaponClass::SmallArms,
                Some(ArmorClass::Small),
                TargetThreatRole::Ordinary,
            ),
            WeaponTargetFit::PreferredSoft
        );
        assert_eq!(
            default_weapon_target_fit(
                WeaponClass::SmallArms,
                Some(ArmorClass::Armored),
                TargetThreatRole::Ordinary,
            ),
            WeaponTargetFit::Fallback
        );
        assert_eq!(
            default_weapon_target_fit(
                WeaponClass::AntiTank,
                Some(ArmorClass::Small),
                TargetThreatRole::AntiArmorThreat,
            ),
            WeaponTargetFit::PreferredThreat
        );
        assert_eq!(
            default_weapon_target_fit(
                WeaponClass::AntiTank,
                Some(ArmorClass::Armored),
                TargetThreatRole::Ordinary,
            ),
            WeaponTargetFit::PreferredArmor
        );
    }

    #[test]
    fn anti_tank_gun_misses_infantry_sized_targets_nine_times_out_of_ten() {
        for victim in [
            EntityKind::Worker,
            EntityKind::Golem,
            EntityKind::Rifleman,
            EntityKind::Panzerfaust,
            EntityKind::MachineGunner,
        ] {
            assert_eq!(miss_chance(EntityKind::AntiTankGun, victim), 0.90);
        }

        for victim in [
            EntityKind::ScoutCar,
            EntityKind::AntiTankGun,
            EntityKind::Tank,
        ] {
            assert_eq!(miss_chance(EntityKind::AntiTankGun, victim), 0.0);
        }
    }

    #[test]
    fn tank_cannon_misses_humanoid_infantry_half_the_time() {
        for victim in [
            EntityKind::Worker,
            EntityKind::Rifleman,
            EntityKind::Panzerfaust,
            EntityKind::MachineGunner,
        ] {
            assert_eq!(miss_chance(EntityKind::Tank, victim), 0.50);
        }

        for victim in [
            EntityKind::Golem,
            EntityKind::ScoutCar,
            EntityKind::AntiTankGun,
            EntityKind::Tank,
        ] {
            assert_eq!(miss_chance(EntityKind::Tank, victim), 0.0);
        }
    }

    #[test]
    fn tank_front_hit_uses_normal_at_damage() {
        assert_eq!(
            effective_damage_with_facing(
                EntityKind::AntiTankGun,
                EntityKind::Tank,
                48,
                None,
                Some(0.0),
                (100.0, 100.0),
                (140.0, 100.0),
            ),
            48
        );
    }

    #[test]
    fn tank_side_hit_boosts_at_damage() {
        assert_eq!(
            effective_damage_with_facing(
                EntityKind::AntiTankGun,
                EntityKind::Tank,
                48,
                None,
                Some(0.0),
                (100.0, 100.0),
                (100.0, 140.0),
            ),
            72
        );
    }

    #[test]
    fn tank_rear_hit_boosts_at_damage() {
        assert_eq!(
            effective_damage_with_facing(
                EntityKind::AntiTankGun,
                EntityKind::Tank,
                48,
                None,
                Some(0.0),
                (100.0, 100.0),
                (60.0, 100.0),
            ),
            82
        );
    }

    #[test]
    fn tank_shell_uses_same_facing_modifiers_against_tank() {
        assert_eq!(
            effective_damage_with_facing(
                EntityKind::Tank,
                EntityKind::Tank,
                60,
                None,
                Some(0.0),
                (100.0, 100.0),
                (140.0, 100.0),
            ),
            60
        );
        assert_eq!(
            effective_damage_with_facing(
                EntityKind::Tank,
                EntityKind::Tank,
                60,
                None,
                Some(0.0),
                (100.0, 100.0),
                (100.0, 140.0),
            ),
            90
        );
        assert_eq!(
            effective_damage_with_facing(
                EntityKind::Tank,
                EntityKind::Tank,
                60,
                None,
                Some(0.0),
                (100.0, 100.0),
                (60.0, 100.0),
            ),
            102
        );
    }

    #[test]
    fn rifleman_vs_rifleman_ignores_facing() {
        assert_eq!(
            effective_damage_with_facing(
                EntityKind::Rifleman,
                EntityKind::Rifleman,
                5,
                None,
                Some(0.0),
                (100.0, 100.0),
                (60.0, 100.0),
            ),
            5
        );
    }

    #[test]
    fn tank_vs_building_ignores_facing() {
        assert_eq!(
            effective_damage_with_facing(
                EntityKind::Tank,
                EntityKind::Barracks,
                60,
                None,
                Some(0.0),
                (100.0, 100.0),
                (60.0, 100.0),
            ),
            60
        );
    }

    #[test]
    fn facing_classification_wraps_around_pi() {
        assert_eq!(
            classify_armor_facing(std::f32::consts::PI - 0.05, (100.0, 100.0), (60.0, 98.0),),
            ArmorFacing::Front
        );
        assert_eq!(
            classify_armor_facing(-std::f32::consts::PI + 0.05, (100.0, 100.0), (60.0, 102.0),),
            ArmorFacing::Front
        );
    }
}
