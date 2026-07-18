use crate::game::entity::EntityKind;
use crate::rules;
use crate::rules::economy::ResourceCost;
pub use crate::rules::faction::{AbilityKind, AbilityQueuePolicy, AbilityTargetMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityEffectHook {
    LegacyNoop,
    ReservedNoop,
    OwnedAreaStatus,
    DelayedWorld,
    ArtilleryPointFire,
    DashReturn,
    LineProjectile,
    MagicAnchor,
    ConsumeGolem,
    ScoutPlane,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbilityDefinition {
    pub kind: AbilityKind,
    pub carriers: &'static [EntityKind],
    pub target_mode: AbilityTargetMode,
    pub range_tiles: Option<u32>,
    pub min_range_tiles: Option<u32>,
    pub cooldown_ticks: u16,
    pub charges: Option<u16>,
    pub charge_recharge_ticks: Option<u16>,
    pub cost: ResourceCost,
    pub tech_requirement: Option<EntityKind>,
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
        _ => None,
    }
}

pub fn effect_hook(kind: AbilityKind) -> AbilityEffectHook {
    match kind {
        AbilityKind::Charge => AbilityEffectHook::LegacyNoop,
        AbilityKind::Smoke | AbilityKind::MortarFire => AbilityEffectHook::DelayedWorld,
        AbilityKind::PointFire | AbilityKind::BlanketFire => AbilityEffectHook::ArtilleryPointFire,
        AbilityKind::Breakthrough => AbilityEffectHook::OwnedAreaStatus,
        AbilityKind::ScoutPlane => AbilityEffectHook::ScoutPlane,
        AbilityKind::DismissScoutPlane => AbilityEffectHook::ReservedNoop,
        AbilityKind::EkatTeleport => AbilityEffectHook::DashReturn,
        AbilityKind::EkatLineShot => AbilityEffectHook::LineProjectile,
        AbilityKind::EkatMagicAnchor => AbilityEffectHook::MagicAnchor,
        AbilityKind::EkatConsumeGolem => AbilityEffectHook::ConsumeGolem,
    }
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
