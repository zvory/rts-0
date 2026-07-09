use std::str::FromStr;

use crate::game::entity::EntityKind;
use crate::protocol;
use crate::rules;
use crate::rules::economy::ResourceCost;
pub use crate::rules::faction::AbilityQueuePolicy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AbilityKind {
    Charge,
    Smoke,
    MortarFire,
    PointFire,
    BlanketFire,
    Breakthrough,
    ScoutPlane,
    DismissScoutPlane,
    EkatTeleport,
    EkatLineShot,
    EkatMagicAnchor,
    EkatConsumeGolem,
}

impl AbilityKind {
    pub fn to_protocol_str(self) -> &'static str {
        match self {
            AbilityKind::Charge => protocol::abilities::CHARGE,
            AbilityKind::Smoke => protocol::abilities::SMOKE,
            AbilityKind::MortarFire => protocol::abilities::MORTAR_FIRE,
            AbilityKind::PointFire => protocol::abilities::POINT_FIRE,
            AbilityKind::BlanketFire => protocol::abilities::BLANKET_FIRE,
            AbilityKind::Breakthrough => protocol::abilities::BREAKTHROUGH,
            AbilityKind::ScoutPlane => protocol::abilities::SCOUT_PLANE,
            AbilityKind::DismissScoutPlane => protocol::abilities::DISMISS_SCOUT_PLANE,
            AbilityKind::EkatTeleport => protocol::abilities::EKAT_TELEPORT,
            AbilityKind::EkatLineShot => protocol::abilities::EKAT_LINE_SHOT,
            AbilityKind::EkatMagicAnchor => protocol::abilities::EKAT_MAGIC_ANCHOR,
            AbilityKind::EkatConsumeGolem => protocol::abilities::EKAT_CONSUME_GOLEM,
        }
    }

    pub fn to_planner_code(self) -> u16 {
        match self {
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

    pub fn from_planner_code(code: u16) -> Option<Self> {
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
}

impl FromStr for AbilityKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            protocol::abilities::CHARGE => Ok(AbilityKind::Charge),
            protocol::abilities::SMOKE => Ok(AbilityKind::Smoke),
            protocol::abilities::MORTAR_FIRE => Ok(AbilityKind::MortarFire),
            protocol::abilities::POINT_FIRE => Ok(AbilityKind::PointFire),
            protocol::abilities::BLANKET_FIRE => Ok(AbilityKind::BlanketFire),
            protocol::abilities::BREAKTHROUGH => Ok(AbilityKind::Breakthrough),
            protocol::abilities::SCOUT_PLANE => Ok(AbilityKind::ScoutPlane),
            protocol::abilities::DISMISS_SCOUT_PLANE => Ok(AbilityKind::DismissScoutPlane),
            protocol::abilities::EKAT_TELEPORT => Ok(AbilityKind::EkatTeleport),
            protocol::abilities::EKAT_LINE_SHOT => Ok(AbilityKind::EkatLineShot),
            protocol::abilities::EKAT_MAGIC_ANCHOR => Ok(AbilityKind::EkatMagicAnchor),
            protocol::abilities::EKAT_CONSUME_GOLEM => Ok(AbilityKind::EkatConsumeGolem),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityTargetMode {
    SelfTarget,
    WorldPoint,
}

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
    pub cost: ResourceCost,
    pub tech_requirement: Option<EntityKind>,
    pub may_queue: bool,
    pub queue_policy: AbilityQueuePolicy,
    pub autocast: bool,
    pub command_card: bool,
    pub effect_hook: AbilityEffectHook,
}

pub fn definition(kind: AbilityKind) -> AbilityDefinition {
    let entry = rules::faction::ability_definition(kind.to_protocol_str())
        .unwrap_or_else(|| unreachable!("missing registry entry for {:?}", kind));
    AbilityDefinition {
        kind,
        carriers: entry.carriers,
        target_mode: match entry.target_mode {
            rules::faction::AbilityTargetMode::SelfTarget => AbilityTargetMode::SelfTarget,
            rules::faction::AbilityTargetMode::WorldPoint => AbilityTargetMode::WorldPoint,
        },
        range_tiles: entry.range_tiles,
        min_range_tiles: entry.min_range_tiles,
        cooldown_ticks: entry.cooldown_ticks,
        charges: entry.charges,
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
