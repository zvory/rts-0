use crate::game::entity::EntityKind;
use crate::rules;
use crate::rules::economy::ResourceCost;
pub use crate::rules::faction::{AbilityKind, AbilityQueuePolicy, AbilityTargetMode};
use rts_rules::math::GameplayMath as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityEffectHook {
    LegacyNoop,
    ReservedNoop,
    OwnedAreaStatus,
    DelayedWorld,
    ArtilleryPointFire,
    DashReturn,
    Dash,
    LineProjectile,
    MagicAnchor,
    Shield,
    ConsumeGolem,
    ScoutPlane,
    Evolve,
    ShopItem,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbilityDefinition {
    pub kind: AbilityKind,
    pub carriers: &'static [EntityKind],
    pub target_mode: AbilityTargetMode,
    pub range_tiles: Option<f32>,
    pub min_range_tiles: Option<u32>,
    pub cooldown_ticks: u16,
    pub charges: Option<u16>,
    pub charge_recharge_ticks: Option<u16>,
    pub cost: ResourceCost,
    pub tech_requirement: Option<EntityKind>,
    pub upgrade_requirement: Option<crate::game::upgrade::UpgradeKind>,
    pub may_queue: bool,
    pub queue_policy: AbilityQueuePolicy,
    pub autocast: bool,
    pub command_card: bool,
    pub effect_hook: AbilityEffectHook,
}

pub fn definition(kind: AbilityKind) -> AbilityDefinition {
    let entry = rules::faction::ability_definition(kind);
    AbilityDefinition {
        kind,
        carriers: entry.carriers,
        target_mode: entry.target_mode,
        range_tiles: entry.range_tiles,
        min_range_tiles: entry.min_range_tiles,
        cooldown_ticks: entry.cooldown_ticks,
        charges: entry.charges,
        charge_recharge_ticks: entry.charge_recharge_ticks,
        cost: entry.cost,
        tech_requirement: entry.tech_requirement,
        upgrade_requirement: entry.upgrade_requirement,
        may_queue: entry.may_queue(),
        queue_policy: entry.queue_policy,
        autocast: entry.autocast,
        command_card: entry.command_card,
        effect_hook: effect_hook(kind),
    }
}

pub fn carried_by(kind: AbilityKind, entity_kind: EntityKind) -> bool {
    definition(kind).carriers.contains(&entity_kind)
}

pub fn cost_for_carrier(kind: AbilityKind, carrier: EntityKind) -> ResourceCost {
    rules::faction::ability_cost_for_carrier(kind, carrier)
}

pub(in crate::game) fn planner_code(kind: AbilityKind) -> u16 {
    match kind {
        AbilityKind::Charge => 0,
        AbilityKind::Smoke => 1,
        AbilityKind::MortarFire => 2,
        AbilityKind::PointFire => 3,
        AbilityKind::Breakthrough => 4,
        AbilityKind::EkatTeleport => 5,
        AbilityKind::EkatLineShot => 6,
        AbilityKind::EkatMagicAnchor => 7,
        AbilityKind::EkatConsumeGolem => 8,
        AbilityKind::BlanketFire => 9,
        AbilityKind::DismissScoutPlane => 10,
        AbilityKind::ScoutPlane => 11,
        AbilityKind::Barrage => 12,
        AbilityKind::EvolveWind => 13,
        AbilityKind::EvolveWar => 14,
        AbilityKind::EvolveMagic => 15,
        AbilityKind::DiscipleShield => 16,
        AbilityKind::ThrowSword => 17,
        AbilityKind::Regen => 18,
        AbilityKind::EscapeTreasure => 19,
        AbilityKind::WindDash => 20,
        AbilityKind::HealingSeal => 21,
        AbilityKind::Haste => 22,
        AbilityKind::DoubleDamage => 23,
        AbilityKind::ArmorTreasure => 24,
        AbilityKind::VitalityTreasure => 25,
        AbilityKind::VectorField => 26,
        AbilityKind::DoubleXp => 27,
        AbilityKind::BreakthroughTreasure => 28,
    }
}

pub(in crate::game) fn from_planner_code(code: u16) -> Option<AbilityKind> {
    match code {
        0 => Some(AbilityKind::Charge),
        1 => Some(AbilityKind::Smoke),
        2 => Some(AbilityKind::MortarFire),
        3 => Some(AbilityKind::PointFire),
        4 => Some(AbilityKind::Breakthrough),
        5 => Some(AbilityKind::EkatTeleport),
        6 => Some(AbilityKind::EkatLineShot),
        7 => Some(AbilityKind::EkatMagicAnchor),
        8 => Some(AbilityKind::EkatConsumeGolem),
        9 => Some(AbilityKind::BlanketFire),
        10 => Some(AbilityKind::DismissScoutPlane),
        11 => Some(AbilityKind::ScoutPlane),
        12 => Some(AbilityKind::Barrage),
        13 => Some(AbilityKind::EvolveWind),
        14 => Some(AbilityKind::EvolveWar),
        15 => Some(AbilityKind::EvolveMagic),
        16 => Some(AbilityKind::DiscipleShield),
        17 => Some(AbilityKind::ThrowSword),
        18 => Some(AbilityKind::Regen),
        19 => Some(AbilityKind::EscapeTreasure),
        20 => Some(AbilityKind::WindDash),
        21 => Some(AbilityKind::HealingSeal),
        22 => Some(AbilityKind::Haste),
        23 => Some(AbilityKind::DoubleDamage),
        24 => Some(AbilityKind::ArmorTreasure),
        25 => Some(AbilityKind::VitalityTreasure),
        26 => Some(AbilityKind::VectorField),
        27 => Some(AbilityKind::DoubleXp),
        28 => Some(AbilityKind::BreakthroughTreasure),

        _ => None,
    }
}

pub fn effect_hook(kind: AbilityKind) -> AbilityEffectHook {
    match kind {
        AbilityKind::EvolveWind | AbilityKind::EvolveWar | AbilityKind::EvolveMagic => {
            AbilityEffectHook::Evolve
        }
        AbilityKind::Regen
        | AbilityKind::EscapeTreasure
        | AbilityKind::HealingSeal
        | AbilityKind::Haste
        | AbilityKind::DoubleXp
        | AbilityKind::DoubleDamage
        | AbilityKind::ArmorTreasure
        | AbilityKind::VitalityTreasure
        | AbilityKind::BreakthroughTreasure => AbilityEffectHook::ShopItem,
        AbilityKind::Charge => AbilityEffectHook::LegacyNoop,
        AbilityKind::Smoke | AbilityKind::MortarFire | AbilityKind::Barrage => {
            AbilityEffectHook::DelayedWorld
        }
        AbilityKind::PointFire | AbilityKind::BlanketFire => AbilityEffectHook::ArtilleryPointFire,
        AbilityKind::Breakthrough => AbilityEffectHook::OwnedAreaStatus,
        AbilityKind::ScoutPlane => AbilityEffectHook::ScoutPlane,
        AbilityKind::DismissScoutPlane => AbilityEffectHook::ReservedNoop,
        AbilityKind::EkatTeleport => AbilityEffectHook::DashReturn,
        AbilityKind::WindDash => AbilityEffectHook::Dash,
        AbilityKind::EkatLineShot | AbilityKind::ThrowSword => AbilityEffectHook::LineProjectile,
        AbilityKind::EkatMagicAnchor | AbilityKind::VectorField => AbilityEffectHook::MagicAnchor,
        AbilityKind::DiscipleShield => AbilityEffectHook::Shield,
        AbilityKind::EkatConsumeGolem => AbilityEffectHook::ConsumeGolem,
    }
}

/// Rocket racks follow the hull; ten degrees of launch leeway on either side.
pub(crate) fn barrage_facing_ready(facing: f32, bearing: f32) -> bool {
    let error = (bearing - facing)
        .det_sin()
        .det_atan2((bearing - facing).det_cos());
    error.abs() <= 10.0_f32.to_radians()
}

#[cfg(test)]
mod tests {
    use super::{definition, AbilityEffectHook, AbilityKind};

    #[test]
    fn existing_abilities_are_classified_by_effect_hook() {
        assert_eq!(
            definition(AbilityKind::Charge).effect_hook,
            AbilityEffectHook::LegacyNoop
        );
        assert_eq!(
            definition(AbilityKind::Smoke).effect_hook,
            AbilityEffectHook::DelayedWorld
        );
        assert_eq!(
            definition(AbilityKind::MortarFire).effect_hook,
            AbilityEffectHook::DelayedWorld
        );
        assert_eq!(
            definition(AbilityKind::PointFire).effect_hook,
            AbilityEffectHook::ArtilleryPointFire
        );
        assert_eq!(
            definition(AbilityKind::BlanketFire).effect_hook,
            AbilityEffectHook::ArtilleryPointFire
        );
        assert_eq!(
            definition(AbilityKind::Breakthrough).effect_hook,
            AbilityEffectHook::OwnedAreaStatus
        );
        assert_eq!(
            definition(AbilityKind::ScoutPlane).effect_hook,
            AbilityEffectHook::ScoutPlane
        );
        assert_eq!(
            definition(AbilityKind::DismissScoutPlane).effect_hook,
            AbilityEffectHook::ReservedNoop
        );
        assert_eq!(
            definition(AbilityKind::EkatTeleport).effect_hook,
            AbilityEffectHook::DashReturn
        );
        assert_eq!(
            definition(AbilityKind::EkatLineShot).effect_hook,
            AbilityEffectHook::LineProjectile
        );
        assert_eq!(
            definition(AbilityKind::EkatMagicAnchor).effect_hook,
            AbilityEffectHook::MagicAnchor
        );
        assert_eq!(
            definition(AbilityKind::EkatConsumeGolem).effect_hook,
            AbilityEffectHook::ConsumeGolem
        );
    }
}
