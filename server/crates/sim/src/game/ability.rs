use std::str::FromStr;

use crate::config;
use crate::game::entity::EntityKind;
use crate::protocol;
use crate::rules::economy::ResourceCost;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbilityKind {
    Charge,
    Smoke,
    MortarFire,
    PointFire,
    Breakthrough,
}

impl AbilityKind {
    pub fn to_protocol_str(self) -> &'static str {
        match self {
            AbilityKind::Charge => protocol::abilities::CHARGE,
            AbilityKind::Smoke => protocol::abilities::SMOKE,
            AbilityKind::MortarFire => protocol::abilities::MORTAR_FIRE,
            AbilityKind::PointFire => protocol::abilities::POINT_FIRE,
            AbilityKind::Breakthrough => protocol::abilities::BREAKTHROUGH,
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
            protocol::abilities::BREAKTHROUGH => Ok(AbilityKind::Breakthrough),
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
const POINT_FIRE_CARRIERS: &[EntityKind] = &[EntityKind::Artillery];
const BREAKTHROUGH_CARRIERS: &[EntityKind] = &[EntityKind::CommandCar];

pub fn definition(kind: AbilityKind) -> AbilityDefinition {
    match kind {
        AbilityKind::Charge => AbilityDefinition {
            kind,
            carriers: CHARGE_CARRIERS,
            target_mode: AbilityTargetMode::SelfTarget,
            range_tiles: None,
            cooldown_ticks: config::RIFLEMAN_CHARGE_COOLDOWN_TICKS,
            cost: ResourceCost::new(0, 0),
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
            cost: ResourceCost::new(0, 0),
            tech_requirement: None,
            may_queue: false,
        },
        AbilityKind::PointFire => AbilityDefinition {
            kind,
            carriers: POINT_FIRE_CARRIERS,
            target_mode: AbilityTargetMode::WorldPoint,
            range_tiles: Some(config::ARTILLERY_MAX_RANGE_TILES),
            cooldown_ticks: 0,
            cost: ResourceCost::new(0, 0),
            tech_requirement: None,
            may_queue: true,
        },
        AbilityKind::Breakthrough => AbilityDefinition {
            kind,
            carriers: BREAKTHROUGH_CARRIERS,
            target_mode: AbilityTargetMode::SelfTarget,
            range_tiles: None,
            cooldown_ticks: config::BREAKTHROUGH_COOLDOWN_TICKS,
            cost: ResourceCost::new(0, 0),
            tech_requirement: None,
            may_queue: true,
        },
    }
}

pub fn carried_by(kind: AbilityKind, entity_kind: EntityKind) -> bool {
    definition(kind).carriers.contains(&entity_kind)
}
