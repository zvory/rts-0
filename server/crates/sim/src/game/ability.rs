use std::str::FromStr;

use crate::config;
use crate::game::entity::EntityKind;
use crate::protocol;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbilityKind {
    Charge,
    Smoke,
    MortarFire,
}

impl AbilityKind {
    pub fn to_protocol_str(self) -> &'static str {
        match self {
            AbilityKind::Charge => protocol::abilities::CHARGE,
            AbilityKind::Smoke => protocol::abilities::SMOKE,
            AbilityKind::MortarFire => protocol::abilities::MORTAR_FIRE,
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
pub struct ResourceCost {
    pub steel: u32,
    pub oil: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbilityDefinition {
    pub kind: AbilityKind,
    pub carriers: &'static [EntityKind],
    pub target_mode: AbilityTargetMode,
    pub range_tiles: Option<u32>,
    pub cooldown_ticks: u16,
    pub cost: ResourceCost,
    pub tech_requirement: Option<EntityKind>,
    pub may_queue: bool,
}

const CHARGE_CARRIERS: &[EntityKind] = &[];
const SMOKE_CARRIERS: &[EntityKind] = &[EntityKind::ScoutCar];
const MORTAR_FIRE_CARRIERS: &[EntityKind] = &[EntityKind::MortarTeam];

pub fn definition(kind: AbilityKind) -> AbilityDefinition {
    match kind {
        AbilityKind::Charge => AbilityDefinition {
            kind,
            carriers: CHARGE_CARRIERS,
            target_mode: AbilityTargetMode::SelfTarget,
            range_tiles: None,
            cooldown_ticks: config::RIFLEMAN_CHARGE_COOLDOWN_TICKS,
            cost: ResourceCost { steel: 0, oil: 0 },
            tech_requirement: None,
            may_queue: false,
        },
        AbilityKind::Smoke => AbilityDefinition {
            kind,
            carriers: SMOKE_CARRIERS,
            target_mode: AbilityTargetMode::WorldPoint,
            range_tiles: Some(config::SMOKE_ABILITY_RANGE_TILES),
            cooldown_ticks: config::SMOKE_ABILITY_COOLDOWN_TICKS,
            cost: ResourceCost {
                steel: config::SMOKE_ABILITY_COST_STEEL,
                oil: config::SMOKE_ABILITY_COST_OIL,
            },
            tech_requirement: None,
            may_queue: true,
        },
        AbilityKind::MortarFire => AbilityDefinition {
            kind,
            carriers: MORTAR_FIRE_CARRIERS,
            target_mode: AbilityTargetMode::WorldPoint,
            range_tiles: Some(9),
            cooldown_ticks: 60,
            cost: ResourceCost { steel: 0, oil: 0 },
            tech_requirement: None,
            may_queue: false,
        },
    }
}

pub fn carried_by(kind: AbilityKind, entity_kind: EntityKind) -> bool {
    definition(kind).carriers.contains(&entity_kind)
}
