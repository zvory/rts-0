//! Faction-aware catalog queries.
//!
//! Runtime and wire identity remain global: faction entries reference the same [`EntityKind`],
//! upgrade ids, ability ids, and Steel/Oil/Supply costs used by the current game. Reuse a global
//! id across factions only when its gameplay semantics are identical for every faction that can
//! use it; divergent behavior needs a distinct global id gated by catalog availability.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{balance, defs, economy::ResourceCost, EntityKind};

pub const DEFAULT_FACTION_ID: &str = "kriegsia";
pub const EKAT_FACTION_ID: &str = "ekat";
pub const EMPTY_FIXTURE_FACTION_ID: &str = "phase2_empty_fixture";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum UpgradeKind {
    Methamphetamines,
    Panzerfausts,
    Entrenchment,
    AntiTankGunUnlock,
    ArtilleryUnlock,
    BallisticTables,
    TankUnlock,
    MortarAutocast,
    SmokePlus,
}

impl UpgradeKind {
    pub const ALL: &'static [Self] = &[
        Self::Methamphetamines,
        Self::Panzerfausts,
        Self::Entrenchment,
        Self::AntiTankGunUnlock,
        Self::ArtilleryUnlock,
        Self::BallisticTables,
        Self::TankUnlock,
        Self::MortarAutocast,
        Self::SmokePlus,
    ];

    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::Methamphetamines => "methamphetamines",
            Self::Panzerfausts => "panzerfausts",
            Self::Entrenchment => "entrenchment",
            Self::AntiTankGunUnlock => "anti_tank_gun_unlock",
            Self::ArtilleryUnlock => "artillery_unlock",
            Self::BallisticTables => "ballistic_tables",
            Self::TankUnlock => "tank_unlock",
            Self::MortarAutocast => "mortar_autocast",
            Self::SmokePlus => "smoke_plus",
        }
    }

    pub const fn to_protocol_str(self) -> &'static str {
        self.stable_id()
    }
}

impl FromStr for UpgradeKind {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.stable_id() == value)
            .ok_or(())
    }
}

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
    pub const ALL: &'static [Self] = &[
        Self::Charge,
        Self::Smoke,
        Self::MortarFire,
        Self::PointFire,
        Self::BlanketFire,
        Self::Breakthrough,
        Self::ScoutPlane,
        Self::DismissScoutPlane,
        Self::EkatTeleport,
        Self::EkatLineShot,
        Self::EkatMagicAnchor,
        Self::EkatConsumeGolem,
    ];

    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::Charge => "charge",
            Self::Smoke => "smoke",
            Self::MortarFire => "mortarFire",
            Self::PointFire => "pointFire",
            Self::BlanketFire => "blanketFire",
            Self::Breakthrough => "breakthrough",
            Self::ScoutPlane => "scoutPlane",
            Self::DismissScoutPlane => "dismissScoutPlane",
            Self::EkatTeleport => "ekatTeleport",
            Self::EkatLineShot => "ekatLineShot",
            Self::EkatMagicAnchor => "ekatMagicAnchor",
            Self::EkatConsumeGolem => "ekatConsumeGolem",
        }
    }

    pub const fn to_protocol_str(self) -> &'static str {
        self.stable_id()
    }
}

impl FromStr for AbilityKind {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.stable_id() == value)
            .ok_or(())
    }
}

pub const METHAMPHETAMINES_UPGRADE: &str = UpgradeKind::Methamphetamines.stable_id();
pub const PANZERFAUSTS_UPGRADE: &str = UpgradeKind::Panzerfausts.stable_id();
pub const ENTRENCHMENT_UPGRADE: &str = UpgradeKind::Entrenchment.stable_id();
pub const ANTI_TANK_GUN_UNLOCK_UPGRADE: &str = UpgradeKind::AntiTankGunUnlock.stable_id();
pub const ARTILLERY_UNLOCK_UPGRADE: &str = UpgradeKind::ArtilleryUnlock.stable_id();
pub const BALLISTIC_TABLES_UPGRADE: &str = UpgradeKind::BallisticTables.stable_id();
pub const TANK_UNLOCK_UPGRADE: &str = UpgradeKind::TankUnlock.stable_id();
pub const MORTAR_AUTOCAST_UPGRADE: &str = UpgradeKind::MortarAutocast.stable_id();
pub const SMOKE_PLUS_UPGRADE: &str = UpgradeKind::SmokePlus.stable_id();

pub const SMOKE_ABILITY: &str = AbilityKind::Smoke.stable_id();
pub const MORTAR_FIRE_ABILITY: &str = AbilityKind::MortarFire.stable_id();
pub const POINT_FIRE_ABILITY: &str = AbilityKind::PointFire.stable_id();
pub const BLANKET_FIRE_ABILITY: &str = AbilityKind::BlanketFire.stable_id();
pub const BREAKTHROUGH_ABILITY: &str = AbilityKind::Breakthrough.stable_id();
pub const SCOUT_PLANE_ABILITY: &str = AbilityKind::ScoutPlane.stable_id();
pub const DISMISS_SCOUT_PLANE_ABILITY: &str = AbilityKind::DismissScoutPlane.stable_id();
pub const CHARGE_ABILITY: &str = AbilityKind::Charge.stable_id();
pub const EKAT_TELEPORT_ABILITY: &str = AbilityKind::EkatTeleport.stable_id();
pub const EKAT_LINE_SHOT_ABILITY: &str = AbilityKind::EkatLineShot.stable_id();
pub const EKAT_MAGIC_ANCHOR_ABILITY: &str = AbilityKind::EkatMagicAnchor.stable_id();
pub const EKAT_CONSUME_GOLEM_ABILITY: &str = AbilityKind::EkatConsumeGolem.stable_id();

const CURRENT_STANDARD_START_ENTITIES: &[StartingEntityGroup] = &[
    StartingEntityGroup {
        kind: EntityKind::CityCentre,
        count: 1,
        formation: StartingFormation::Center,
        completed: true,
    },
    StartingEntityGroup {
        kind: EntityKind::Worker,
        count: crate::balance::STARTING_WORKERS,
        formation: StartingFormation::Ring {
            radius_tiles_x10: 25,
        },
        completed: true,
    },
];

const EMPTY_FIXTURE_START_ENTITIES: &[StartingEntityGroup] = &[
    StartingEntityGroup {
        kind: EntityKind::Depot,
        count: 1,
        formation: StartingFormation::Center,
        completed: true,
    },
    StartingEntityGroup {
        kind: EntityKind::ScoutCar,
        count: 1,
        formation: StartingFormation::Ring {
            radius_tiles_x10: 20,
        },
        completed: true,
    },
];

const EKAT_START_ENTITIES: &[StartingEntityGroup] = &[
    StartingEntityGroup {
        kind: EntityKind::Zamok,
        count: 1,
        formation: StartingFormation::Center,
        completed: true,
    },
    StartingEntityGroup {
        kind: EntityKind::Ekat,
        count: 1,
        formation: StartingFormation::Ring {
            radius_tiles_x10: 25,
        },
        completed: true,
    },
];

pub const CURRENT_STANDARD_LOADOUT: FactionLoadout = FactionLoadout {
    id: "kriegsia.standard",
    initial_steel: crate::balance::STARTING_STEEL,
    initial_oil: crate::balance::STARTING_OIL,
    starting_entities: CURRENT_STANDARD_START_ENTITIES,
    opening_upgrades: &[],
};

pub const EMPTY_FIXTURE_LOADOUT: FactionLoadout = FactionLoadout {
    id: "phase2_empty_fixture.scout_depot",
    initial_steel: 125,
    initial_oil: 25,
    starting_entities: EMPTY_FIXTURE_START_ENTITIES,
    opening_upgrades: &[],
};

pub const EKAT_LOADOUT: FactionLoadout = FactionLoadout {
    id: "ekat.standard",
    initial_steel: 0,
    initial_oil: 0,
    starting_entities: EKAT_START_ENTITIES,
    opening_upgrades: &[],
};

const DEFAULT_UNITS: &[EntityKind] = &[
    EntityKind::Worker,
    EntityKind::Rifleman,
    EntityKind::MachineGunner,
    EntityKind::AntiTankGun,
    EntityKind::MortarTeam,
    EntityKind::Artillery,
    EntityKind::Tank,
    EntityKind::ScoutCar,
    EntityKind::ScoutPlane,
    EntityKind::CommandCar,
];

const DEFAULT_BUILDINGS: &[EntityKind] = &[
    EntityKind::CityCentre,
    // Retained for replay and fixture compatibility, but unavailable in the current build catalog.
    EntityKind::Depot,
    EntityKind::Barracks,
    EntityKind::TrainingCentre,
    EntityKind::Factory,
    EntityKind::ResearchComplex,
    EntityKind::Steelworks,
    EntityKind::TankTrap,
    EntityKind::PumpJack,
];

const DEFAULT_WORKER_BUILDABLES: &[EntityKind] = &[
    EntityKind::CityCentre,
    EntityKind::PumpJack,
    EntityKind::Barracks,
    EntityKind::TrainingCentre,
    EntityKind::ResearchComplex,
    EntityKind::Factory,
    EntityKind::Steelworks,
    EntityKind::TankTrap,
];

const ARTILLERY_ABILITY_CARRIERS: &[EntityKind] = &[EntityKind::Artillery];

const DEFAULT_UPGRADES: [UpgradeCatalogEntry; 9] = [
    UpgradeCatalogEntry {
        kind: UpgradeKind::Methamphetamines,
        researched_at: EntityKind::TrainingCentre,
    },
    UpgradeCatalogEntry {
        kind: UpgradeKind::Panzerfausts,
        researched_at: EntityKind::TrainingCentre,
    },
    UpgradeCatalogEntry {
        kind: UpgradeKind::Entrenchment,
        researched_at: EntityKind::TrainingCentre,
    },
    UpgradeCatalogEntry {
        kind: UpgradeKind::AntiTankGunUnlock,
        researched_at: EntityKind::ResearchComplex,
    },
    UpgradeCatalogEntry {
        kind: UpgradeKind::BallisticTables,
        researched_at: EntityKind::ResearchComplex,
    },
    UpgradeCatalogEntry {
        kind: UpgradeKind::TankUnlock,
        researched_at: EntityKind::ResearchComplex,
    },
    UpgradeCatalogEntry {
        kind: UpgradeKind::MortarAutocast,
        researched_at: EntityKind::ResearchComplex,
    },
    UpgradeCatalogEntry {
        kind: UpgradeKind::SmokePlus,
        researched_at: EntityKind::ResearchComplex,
    },
    UpgradeCatalogEntry {
        kind: UpgradeKind::ArtilleryUnlock,
        researched_at: EntityKind::ResearchComplex,
    },
];

const DEFAULT_ABILITIES: [AbilityCatalogEntry; 8] = [
    AbilityCatalogEntry {
        kind: AbilityKind::Charge,
        label: "Charge",
        icon: "CHG",
        hotkey: None,
        title: "Legacy Charge command compatibility",
        carriers: &[],
        target_mode: AbilityTargetMode::SelfTarget,
        range_tiles: None,
        min_range_tiles: None,
        cooldown_ticks: 0,
        charges: None,
        cost: ResourceCost::new(0, 0),
        tech_requirement: None,
        queue_policy: AbilityQueuePolicy::NotQueueable,
        autocast: false,
        command_card: false,
        protocol_code: 1,
        order_stage_code: 8,
    },
    AbilityCatalogEntry {
        kind: AbilityKind::Smoke,
        label: "Smoke",
        icon: "SMK",
        hotkey: Some("D"),
        title: "Target a smoke grenade location",
        carriers: &[EntityKind::ScoutCar],
        target_mode: AbilityTargetMode::WorldPoint,
        range_tiles: Some(balance::SMOKE_ABILITY_RANGE_TILES),
        min_range_tiles: None,
        cooldown_ticks: balance::SMOKE_ABILITY_COOLDOWN_TICKS,
        charges: None,
        cost: ResourceCost::new(
            balance::SMOKE_ABILITY_COST_STEEL,
            balance::SMOKE_ABILITY_COST_OIL,
        ),
        tech_requirement: Some(EntityKind::ResearchComplex),
        queue_policy: AbilityQueuePolicy::QueueSkipIfNotReady,
        autocast: false,
        command_card: true,
        protocol_code: 2,
        order_stage_code: 6,
    },
    AbilityCatalogEntry {
        kind: AbilityKind::MortarFire,
        label: "Fire",
        icon: "FIR",
        hotkey: Some("X"),
        title: "Target mortar fire",
        carriers: &[EntityKind::MortarTeam],
        target_mode: AbilityTargetMode::WorldPoint,
        range_tiles: Some(balance::MORTAR_RANGE_TILES),
        min_range_tiles: Some(balance::MORTAR_MIN_RANGE_TILES),
        cooldown_ticks: (balance::TICK_HZ as u16) * 2,
        charges: None,
        cost: ResourceCost::new(0, 0),
        tech_requirement: None,
        queue_policy: AbilityQueuePolicy::QueueWaitUntilReady,
        autocast: true,
        command_card: true,
        protocol_code: 3,
        order_stage_code: 9,
    },
    AbilityCatalogEntry {
        kind: AbilityKind::PointFire,
        label: "Point Fire",
        icon: "PF",
        hotkey: Some("X"),
        title: "Target artillery fire",
        carriers: ARTILLERY_ABILITY_CARRIERS,
        target_mode: AbilityTargetMode::WorldPoint,
        range_tiles: Some(balance::ARTILLERY_MAX_RANGE_TILES),
        min_range_tiles: Some(balance::ARTILLERY_MIN_RANGE_TILES),
        cooldown_ticks: balance::ARTILLERY_RELOAD_TICKS as u16,
        charges: None,
        cost: ResourceCost::new(balance::ARTILLERY_AMMO_COST_STEEL, 0),
        tech_requirement: None,
        queue_policy: AbilityQueuePolicy::QueueSkipIfNotReady,
        autocast: false,
        command_card: true,
        protocol_code: 4,
        order_stage_code: 10,
    },
    AbilityCatalogEntry {
        kind: AbilityKind::BlanketFire,
        label: "Blanket Fire",
        icon: "BF",
        hotkey: Some("C"),
        title: "Target blanket artillery fire",
        carriers: ARTILLERY_ABILITY_CARRIERS,
        target_mode: AbilityTargetMode::WorldPoint,
        range_tiles: Some(balance::ARTILLERY_MAX_RANGE_TILES),
        min_range_tiles: Some(balance::ARTILLERY_MIN_RANGE_TILES),
        cooldown_ticks: balance::ARTILLERY_RELOAD_TICKS as u16,
        charges: None,
        cost: ResourceCost::new(balance::ARTILLERY_AMMO_COST_STEEL, 0),
        tech_requirement: None,
        queue_policy: AbilityQueuePolicy::QueueSkipIfNotReady,
        autocast: false,
        command_card: true,
        protocol_code: 10,
        order_stage_code: 17,
    },
    AbilityCatalogEntry {
        kind: AbilityKind::Breakthrough,
        label: "Breakthrough!",
        icon: "BRK",
        hotkey: Some("E"),
        title: "Nearby owned units are always faster; activate full speed (stronger in smoke)",
        carriers: &[EntityKind::CommandCar],
        target_mode: AbilityTargetMode::SelfTarget,
        range_tiles: None,
        min_range_tiles: None,
        cooldown_ticks: balance::BREAKTHROUGH_COOLDOWN_TICKS,
        charges: None,
        cost: ResourceCost::new(0, 0),
        tech_requirement: None,
        queue_policy: AbilityQueuePolicy::QueueSkipIfNotReady,
        autocast: false,
        command_card: true,
        protocol_code: 5,
        order_stage_code: 11,
    },
    AbilityCatalogEntry {
        kind: AbilityKind::ScoutPlane,
        label: "Scout Plane",
        icon: "SP",
        hotkey: Some("C"),
        title: "Launch this Command Car's scout plane",
        carriers: &[EntityKind::CommandCar],
        target_mode: AbilityTargetMode::WorldPoint,
        range_tiles: None,
        min_range_tiles: None,
        cooldown_ticks: balance::SCOUT_PLANE_ABILITY_COOLDOWN_TICKS,
        charges: None,
        cost: ResourceCost::new(
            balance::SCOUT_PLANE_COST_STEEL,
            balance::SCOUT_PLANE_COST_OIL,
        ),
        tech_requirement: None,
        queue_policy: AbilityQueuePolicy::QueueSkipIfNotReady,
        autocast: false,
        command_card: true,
        protocol_code: 12,
        order_stage_code: 19,
    },
    AbilityCatalogEntry {
        kind: AbilityKind::DismissScoutPlane,
        label: "Dismiss",
        icon: "X",
        hotkey: Some("X"),
        title: "Dismiss the Scout Plane",
        carriers: &[],
        target_mode: AbilityTargetMode::SelfTarget,
        range_tiles: None,
        min_range_tiles: None,
        cooldown_ticks: 0,
        charges: None,
        cost: ResourceCost::new(0, 0),
        tech_requirement: None,
        queue_policy: AbilityQueuePolicy::NotQueueable,
        autocast: false,
        command_card: false,
        protocol_code: 11,
        order_stage_code: 18,
    },
];

const EKAT_UNITS: &[EntityKind] = &[EntityKind::Ekat, EntityKind::Golem];
const EKAT_BUILDINGS: &[EntityKind] = &[EntityKind::Zamok];

const EKAT_ABILITIES: [AbilityCatalogEntry; 4] = [
    AbilityCatalogEntry {
        kind: AbilityKind::EkatTeleport,
        label: "Dash",
        icon: "DSH",
        hotkey: Some("D"),
        title: "Dash up to 5 tiles, then recast to return",
        carriers: &[EntityKind::Ekat],
        target_mode: AbilityTargetMode::WorldPoint,
        range_tiles: Some(balance::EKAT_TELEPORT_RANGE_TILES),
        min_range_tiles: None,
        cooldown_ticks: balance::EKAT_TELEPORT_COOLDOWN_TICKS,
        charges: None,
        cost: ResourceCost::new(0, 0),
        tech_requirement: None,
        queue_policy: AbilityQueuePolicy::QueueSkipIfNotReady,
        autocast: false,
        command_card: true,
        protocol_code: 6,
        order_stage_code: 12,
    },
    AbilityCatalogEntry {
        kind: AbilityKind::EkatLineShot,
        label: "Line Shot",
        icon: "LS",
        hotkey: Some("X"),
        title: "Send a line projectile out and back",
        carriers: &[EntityKind::Ekat],
        target_mode: AbilityTargetMode::WorldPoint,
        range_tiles: Some(balance::EKAT_LINE_SHOT_RANGE_TILES),
        min_range_tiles: None,
        cooldown_ticks: balance::EKAT_LINE_SHOT_COOLDOWN_TICKS,
        charges: None,
        cost: ResourceCost::new(0, 0),
        tech_requirement: None,
        queue_policy: AbilityQueuePolicy::QueueSkipIfNotReady,
        autocast: false,
        command_card: true,
        protocol_code: 7,
        order_stage_code: 13,
    },
    AbilityCatalogEntry {
        kind: AbilityKind::EkatMagicAnchor,
        label: "Magic Anchor",
        icon: "ANC",
        hotkey: Some("C"),
        title: "Place a 10-second pull field",
        carriers: &[EntityKind::Ekat],
        target_mode: AbilityTargetMode::WorldPoint,
        range_tiles: Some(balance::EKAT_MAGIC_ANCHOR_RANGE_TILES),
        min_range_tiles: None,
        cooldown_ticks: 0,
        charges: None,
        cost: ResourceCost::new(0, 0),
        tech_requirement: None,
        queue_policy: AbilityQueuePolicy::QueueSkipIfNotReady,
        autocast: false,
        command_card: true,
        protocol_code: 8,
        order_stage_code: 14,
    },
    AbilityCatalogEntry {
        kind: AbilityKind::EkatConsumeGolem,
        label: "Consume",
        icon: "CON",
        hotkey: Some("Z"),
        title: "Consume a nearby Golem to heal Ekat to full HP",
        carriers: &[EntityKind::Ekat],
        target_mode: AbilityTargetMode::SelfTarget,
        range_tiles: Some(balance::EKAT_CONSUME_GOLEM_RANGE_TILES),
        min_range_tiles: None,
        cooldown_ticks: 0,
        charges: None,
        cost: ResourceCost::new(0, 0),
        tech_requirement: None,
        queue_policy: AbilityQueuePolicy::NotQueueable,
        autocast: false,
        command_card: true,
        protocol_code: 9,
        order_stage_code: 16,
    },
];

pub const CURRENT_CATALOG: FactionCatalog = FactionCatalog {
    id: DEFAULT_FACTION_ID,
    loadout: CURRENT_STANDARD_LOADOUT,
    units: DEFAULT_UNITS,
    buildings: DEFAULT_BUILDINGS,
    buildables: DEFAULT_WORKER_BUILDABLES,
    upgrades: &DEFAULT_UPGRADES,
    abilities: &DEFAULT_ABILITIES,
    builders: &[EntityKind::Worker],
    gatherers: &[EntityKind::Worker],
    production_anchors: &[
        EntityKind::CityCentre,
        EntityKind::Barracks,
        EntityKind::Factory,
        EntityKind::Steelworks,
    ],
};

pub const EMPTY_FIXTURE_CATALOG: FactionCatalog = FactionCatalog {
    id: EMPTY_FIXTURE_FACTION_ID,
    loadout: EMPTY_FIXTURE_LOADOUT,
    units: &[EntityKind::ScoutCar],
    buildings: &[EntityKind::Depot],
    buildables: &[],
    upgrades: &[],
    abilities: &[],
    builders: &[],
    gatherers: &[],
    production_anchors: &[],
};

pub const EKAT_CATALOG: FactionCatalog = FactionCatalog {
    id: EKAT_FACTION_ID,
    loadout: EKAT_LOADOUT,
    units: EKAT_UNITS,
    buildings: EKAT_BUILDINGS,
    buildables: &[],
    upgrades: &[],
    abilities: &EKAT_ABILITIES,
    builders: &[],
    gatherers: &[EntityKind::Golem],
    production_anchors: &[EntityKind::Zamok],
};

pub const CATALOGS: &[FactionCatalog] = &[CURRENT_CATALOG, EKAT_CATALOG, EMPTY_FIXTURE_CATALOG];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpgradeCatalogEntry {
    pub kind: UpgradeKind,
    pub researched_at: EntityKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityTargetMode {
    SelfTarget,
    WorldPoint,
}

impl AbilityTargetMode {
    pub fn stable_id(self) -> &'static str {
        match self {
            AbilityTargetMode::SelfTarget => "self",
            AbilityTargetMode::WorldPoint => "worldPoint",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityQueuePolicy {
    NotQueueable,
    QueueSkipIfNotReady,
    QueueWaitUntilReady,
}

impl AbilityQueuePolicy {
    pub fn stable_id(self) -> &'static str {
        match self {
            AbilityQueuePolicy::NotQueueable => "notQueueable",
            AbilityQueuePolicy::QueueSkipIfNotReady => "skipIfNotReady",
            AbilityQueuePolicy::QueueWaitUntilReady => "waitUntilReady",
        }
    }

    pub fn may_queue(self) -> bool {
        !matches!(self, AbilityQueuePolicy::NotQueueable)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbilityCatalogEntry {
    pub kind: AbilityKind,
    pub label: &'static str,
    pub icon: &'static str,
    pub hotkey: Option<&'static str>,
    pub title: &'static str,
    pub carriers: &'static [EntityKind],
    pub target_mode: AbilityTargetMode,
    pub range_tiles: Option<u32>,
    pub min_range_tiles: Option<u32>,
    pub cooldown_ticks: u16,
    pub charges: Option<u16>,
    pub cost: ResourceCost,
    pub tech_requirement: Option<EntityKind>,
    pub queue_policy: AbilityQueuePolicy,
    pub autocast: bool,
    pub command_card: bool,
    pub protocol_code: u8,
    pub order_stage_code: u8,
}

impl AbilityCatalogEntry {
    pub fn may_queue(self) -> bool {
        self.queue_policy.may_queue()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartingFormation {
    Center,
    Ring { radius_tiles_x10: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartingEntityGroup {
    pub kind: EntityKind,
    pub count: u32,
    pub formation: StartingFormation,
    pub completed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FactionLoadout {
    pub id: &'static str,
    pub initial_steel: u32,
    pub initial_oil: u32,
    pub starting_entities: &'static [StartingEntityGroup],
    pub opening_upgrades: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FactionCatalog {
    pub id: &'static str,
    pub loadout: FactionLoadout,
    pub units: &'static [EntityKind],
    pub buildings: &'static [EntityKind],
    pub buildables: &'static [EntityKind],
    pub upgrades: &'static [UpgradeCatalogEntry],
    pub abilities: &'static [AbilityCatalogEntry],
    pub builders: &'static [EntityKind],
    pub gatherers: &'static [EntityKind],
    pub production_anchors: &'static [EntityKind],
}

impl FactionCatalog {
    pub fn allows_unit(self, kind: EntityKind) -> bool {
        self.units.contains(&kind)
    }

    pub fn allows_building(self, kind: EntityKind) -> bool {
        self.buildings.contains(&kind)
    }

    pub fn can_build(self, builder: EntityKind, building: EntityKind) -> bool {
        self.builders.contains(&builder) && self.buildables.contains(&building)
    }

    pub fn can_gather(self, unit: EntityKind) -> bool {
        self.gatherers.contains(&unit)
    }

    pub fn can_act_as_production_anchor(self, building: EntityKind) -> bool {
        self.production_anchors.contains(&building)
    }

    pub fn trainable_units(self, building_kind: EntityKind) -> Vec<EntityKind> {
        if !self.can_act_as_production_anchor(building_kind) {
            return Vec::new();
        }
        defs::building_def(building_kind)
            .map(|d| d.trains)
            .unwrap_or(&[])
            .iter()
            .copied()
            .filter(|unit| self.allows_unit(*unit))
            .collect::<Vec<_>>()
    }

    pub fn researchable_upgrade_kinds(
        self,
        building_kind: EntityKind,
    ) -> impl Iterator<Item = UpgradeKind> {
        self.upgrades
            .iter()
            .filter(move |entry| entry.researched_at == building_kind)
            .map(|entry| entry.kind)
    }

    pub fn researchable_upgrades(self, building_kind: EntityKind) -> Vec<&'static str> {
        self.researchable_upgrade_kinds(building_kind)
            .map(UpgradeKind::stable_id)
            .collect()
    }

    pub fn allows_research(self, upgrade: UpgradeKind, building_kind: EntityKind) -> bool {
        self.upgrades
            .iter()
            .any(|entry| entry.kind == upgrade && entry.researched_at == building_kind)
    }

    pub fn allows_ability(self, ability: AbilityKind, carrier: EntityKind) -> bool {
        self.abilities
            .iter()
            .any(|entry| entry.kind == ability && entry.carriers.contains(&carrier))
    }

    pub fn ability(self, ability: AbilityKind) -> Option<AbilityCatalogEntry> {
        self.abilities
            .iter()
            .copied()
            .find(|entry| entry.kind == ability)
    }

    pub fn abilities_for_carrier(
        self,
        carrier: EntityKind,
    ) -> impl Iterator<Item = AbilityCatalogEntry> {
        self.abilities
            .iter()
            .copied()
            .filter(move |entry| entry.carriers.contains(&carrier))
    }
}

pub fn catalog_for(faction_id: &str) -> Option<FactionCatalog> {
    CATALOGS
        .iter()
        .copied()
        .find(|catalog| catalog.id == faction_id)
}

pub fn catalog_for_or_default_empty(faction_id: &str) -> Option<FactionCatalog> {
    if faction_id.trim().is_empty() {
        Some(CURRENT_CATALOG)
    } else {
        catalog_for(faction_id)
    }
}

pub fn catalog_loadout_for(faction_id: &str, loadout_id: &str) -> Option<FactionLoadout> {
    let catalog = catalog_for(faction_id)?;
    (catalog.loadout.id == loadout_id).then_some(catalog.loadout)
}

pub fn ability_definition(kind: AbilityKind) -> AbilityCatalogEntry {
    let [charge, smoke, mortar_fire, point_fire, blanket_fire, breakthrough, scout_plane, dismiss_scout_plane] =
        DEFAULT_ABILITIES;
    let [ekat_teleport, ekat_line_shot, ekat_magic_anchor, ekat_consume_golem] = EKAT_ABILITIES;
    match kind {
        AbilityKind::Charge => charge,
        AbilityKind::Smoke => smoke,
        AbilityKind::MortarFire => mortar_fire,
        AbilityKind::PointFire => point_fire,
        AbilityKind::BlanketFire => blanket_fire,
        AbilityKind::Breakthrough => breakthrough,
        AbilityKind::ScoutPlane => scout_plane,
        AbilityKind::DismissScoutPlane => dismiss_scout_plane,
        AbilityKind::EkatTeleport => ekat_teleport,
        AbilityKind::EkatLineShot => ekat_line_shot,
        AbilityKind::EkatMagicAnchor => ekat_magic_anchor,
        AbilityKind::EkatConsumeGolem => ekat_consume_golem,
    }
}

pub fn upgrade_definition(kind: UpgradeKind) -> UpgradeCatalogEntry {
    let [methamphetamines, panzerfausts, entrenchment, anti_tank_gun_unlock, ballistic_tables, tank_unlock, mortar_autocast, smoke_plus, artillery_unlock] =
        DEFAULT_UPGRADES;
    match kind {
        UpgradeKind::Methamphetamines => methamphetamines,
        UpgradeKind::Panzerfausts => panzerfausts,
        UpgradeKind::Entrenchment => entrenchment,
        UpgradeKind::AntiTankGunUnlock => anti_tank_gun_unlock,
        UpgradeKind::BallisticTables => ballistic_tables,
        UpgradeKind::TankUnlock => tank_unlock,
        UpgradeKind::MortarAutocast => mortar_autocast,
        UpgradeKind::SmokePlus => smoke_plus,
        UpgradeKind::ArtilleryUnlock => artillery_unlock,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_catalog_matches_defs_inventory() {
        assert_eq!(CURRENT_CATALOG.units, DEFAULT_UNITS);

        assert_eq!(CURRENT_CATALOG.buildings, DEFAULT_BUILDINGS);
    }

    #[test]
    fn default_catalog_routes_current_tech_tree() {
        let catalog = CURRENT_CATALOG;
        let research_complex = EntityKind::ResearchComplex;

        assert_eq!(
            catalog.trainable_units(EntityKind::CityCentre),
            vec![EntityKind::Worker]
        );
        assert!(
            catalog.allows_unit(EntityKind::ScoutPlane),
            "Scout Plane remains in the catalog for ability-launched mission entities"
        );
        assert_eq!(
            catalog.trainable_units(EntityKind::Barracks),
            vec![EntityKind::Rifleman, EntityKind::MachineGunner]
        );
        assert_eq!(
            catalog.trainable_units(EntityKind::Factory),
            vec![
                EntityKind::ScoutCar,
                EntityKind::Tank,
                EntityKind::CommandCar
            ]
        );
        assert_eq!(
            catalog.trainable_units(EntityKind::Steelworks),
            vec![
                EntityKind::MortarTeam,
                EntityKind::AntiTankGun,
                EntityKind::Artillery
            ]
        );
        assert!(catalog.allows_research(UpgradeKind::Methamphetamines, EntityKind::TrainingCentre));
        assert!(catalog.allows_research(UpgradeKind::Entrenchment, EntityKind::TrainingCentre));
        assert!(catalog.allows_research(UpgradeKind::AntiTankGunUnlock, research_complex));
        assert!(catalog.allows_research(UpgradeKind::BallisticTables, research_complex));
        assert!(catalog.allows_research(UpgradeKind::ArtilleryUnlock, research_complex));
        assert!(catalog.allows_research(UpgradeKind::TankUnlock, research_complex));
        assert!(catalog.allows_research(UpgradeKind::MortarAutocast, research_complex));
        assert!(catalog.allows_research(UpgradeKind::SmokePlus, research_complex));
        assert!(!catalog.allows_research(UpgradeKind::TankUnlock, EntityKind::TrainingCentre));
        assert!(catalog.allows_building(EntityKind::TankTrap));
        assert!(catalog.can_build(EntityKind::Worker, EntityKind::TankTrap));
        assert!(catalog.allows_building(EntityKind::PumpJack));
        assert!(
            catalog.can_build(EntityKind::Worker, EntityKind::PumpJack),
            "Pump Jacks are worker-buildable while placement remains restricted to oil nodes"
        );
        assert!(!catalog.can_act_as_production_anchor(EntityKind::TankTrap));
        assert!(!catalog.can_act_as_production_anchor(EntityKind::PumpJack));
        assert!(catalog.allows_ability(AbilityKind::Smoke, EntityKind::ScoutCar));
        assert!(catalog.allows_ability(AbilityKind::PointFire, ARTILLERY_ABILITY_CARRIERS[0]));
        assert!(catalog.allows_ability(AbilityKind::BlanketFire, ARTILLERY_ABILITY_CARRIERS[0]));
        assert!(catalog.allows_ability(AbilityKind::ScoutPlane, EntityKind::CommandCar));
        assert!(!catalog.allows_ability(AbilityKind::Charge, EntityKind::Rifleman));
        assert!(!catalog.allows_ability(AbilityKind::DismissScoutPlane, EntityKind::ScoutPlane));
        assert!(!catalog.allows_ability(AbilityKind::Smoke, EntityKind::Worker));
    }

    #[test]
    fn ekat_catalog_exposes_hero_golem_and_zamok() {
        let catalog = EKAT_CATALOG;

        assert_eq!(catalog.units, &[EntityKind::Ekat, EntityKind::Golem]);
        assert_eq!(catalog.buildings, &[EntityKind::Zamok]);
        assert_eq!(
            catalog.trainable_units(EntityKind::Zamok),
            vec![EntityKind::Golem]
        );
        assert_eq!(catalog.loadout.id, "ekat.standard");
        assert_eq!(catalog.loadout.starting_entities, EKAT_START_ENTITIES);
        assert!(catalog.can_gather(EntityKind::Golem));
        assert!(catalog.can_act_as_production_anchor(EntityKind::Zamok));
        assert!(catalog.allows_ability(AbilityKind::EkatTeleport, EntityKind::Ekat));
        assert!(catalog.allows_ability(AbilityKind::EkatLineShot, EntityKind::Ekat));
        assert!(catalog.allows_ability(AbilityKind::EkatMagicAnchor, EntityKind::Ekat));
        assert!(catalog.allows_ability(AbilityKind::EkatConsumeGolem, EntityKind::Ekat));
        assert!(!catalog.allows_unit(EntityKind::Rifleman));
        assert!(!catalog.allows_building(EntityKind::CityCentre));
    }

    #[test]
    fn default_ability_registry_preserves_current_metadata() {
        let smoke = CURRENT_CATALOG.ability(AbilityKind::Smoke).unwrap();
        assert_eq!(smoke.label, "Smoke");
        assert_eq!(smoke.carriers, &[EntityKind::ScoutCar]);
        assert_eq!(smoke.target_mode, AbilityTargetMode::WorldPoint);
        assert_eq!(smoke.range_tiles, Some(balance::SMOKE_ABILITY_RANGE_TILES));
        assert_eq!(smoke.charges, None);
        assert_eq!(
            smoke.tech_requirement,
            Some(EntityKind::ResearchComplex),
            "Smoke should require a completed R&D Complex"
        );
        assert_eq!(
            smoke.cost,
            ResourceCost::new(
                balance::SMOKE_ABILITY_COST_STEEL,
                balance::SMOKE_ABILITY_COST_OIL,
            )
        );
        assert!(smoke.command_card);

        let point_fire = CURRENT_CATALOG.ability(AbilityKind::PointFire).unwrap();
        assert_eq!(point_fire.carriers, ARTILLERY_ABILITY_CARRIERS);
        assert_eq!(
            point_fire.min_range_tiles,
            Some(balance::ARTILLERY_MIN_RANGE_TILES)
        );
        assert_eq!(
            point_fire.range_tiles,
            Some(balance::ARTILLERY_MAX_RANGE_TILES)
        );
        assert_eq!(
            point_fire.cost,
            ResourceCost::new(balance::ARTILLERY_AMMO_COST_STEEL, 0)
        );
        assert_eq!(
            point_fire.cooldown_ticks,
            balance::ARTILLERY_RELOAD_TICKS as u16
        );

        let blanket_fire = CURRENT_CATALOG.ability(AbilityKind::BlanketFire).unwrap();
        assert_eq!(blanket_fire.carriers, ARTILLERY_ABILITY_CARRIERS);
        assert_eq!(blanket_fire.target_mode, AbilityTargetMode::WorldPoint);
        assert_eq!(
            blanket_fire.range_tiles,
            Some(balance::ARTILLERY_MAX_RANGE_TILES)
        );
        assert_eq!(
            blanket_fire.min_range_tiles,
            Some(balance::ARTILLERY_MIN_RANGE_TILES)
        );
        assert_eq!(
            blanket_fire.cost,
            ResourceCost::new(balance::ARTILLERY_AMMO_COST_STEEL, 0)
        );
        assert_eq!(
            blanket_fire.cooldown_ticks,
            balance::ARTILLERY_RELOAD_TICKS as u16
        );
        assert!(blanket_fire.command_card);
        assert_eq!(blanket_fire.protocol_code, 10);
        assert_eq!(blanket_fire.order_stage_code, 17);

        let breakthrough = CURRENT_CATALOG.ability(AbilityKind::Breakthrough).unwrap();
        assert_eq!(breakthrough.target_mode, AbilityTargetMode::SelfTarget);
        assert_eq!(
            breakthrough.cooldown_ticks,
            balance::BREAKTHROUGH_COOLDOWN_TICKS
        );

        let dismiss = ability_definition(AbilityKind::DismissScoutPlane);
        assert!(dismiss.carriers.is_empty());
        assert_eq!(dismiss.target_mode, AbilityTargetMode::SelfTarget);
        assert!(!dismiss.command_card);
        assert_eq!(dismiss.protocol_code, 11);
        assert_eq!(dismiss.order_stage_code, 18);

        let charge = ability_definition(AbilityKind::Charge);
        assert!(!charge.command_card);
        assert!(charge.carriers.is_empty());
        assert_eq!(charge.cooldown_ticks, 0);

        let teleport = EKAT_CATALOG.ability(AbilityKind::EkatTeleport).unwrap();
        assert_eq!(teleport.carriers, &[EntityKind::Ekat]);
        assert_eq!(
            teleport.range_tiles,
            Some(balance::EKAT_TELEPORT_RANGE_TILES)
        );

        let line_shot = EKAT_CATALOG.ability(AbilityKind::EkatLineShot).unwrap();
        assert_eq!(line_shot.carriers, &[EntityKind::Ekat]);
        assert_eq!(
            line_shot.range_tiles,
            Some(balance::EKAT_LINE_SHOT_RANGE_TILES)
        );

        let anchor = EKAT_CATALOG.ability(AbilityKind::EkatMagicAnchor).unwrap();
        assert_eq!(anchor.carriers, &[EntityKind::Ekat]);
        assert_eq!(
            anchor.range_tiles,
            Some(balance::EKAT_MAGIC_ANCHOR_RANGE_TILES)
        );

        let consume = EKAT_CATALOG.ability(AbilityKind::EkatConsumeGolem).unwrap();
        assert_eq!(consume.carriers, &[EntityKind::Ekat]);
        assert_eq!(consume.target_mode, AbilityTargetMode::SelfTarget);
        assert_eq!(
            consume.range_tiles,
            Some(balance::EKAT_CONSUME_GOLEM_RANGE_TILES)
        );
        assert_eq!(consume.protocol_code, 9);
        assert_eq!(consume.order_stage_code, 16);
    }

    #[test]
    fn fixture_catalog_rejects_global_current_kinds() {
        let catalog = EMPTY_FIXTURE_CATALOG;

        assert!(!catalog.allows_unit(EntityKind::Worker));
        assert!(!catalog.allows_building(EntityKind::CityCentre));
        assert!(!catalog.can_build(EntityKind::Worker, EntityKind::Depot));
        assert!(catalog.allows_unit(EntityKind::ScoutCar));
        assert!(catalog.allows_building(EntityKind::Depot));
        assert!(catalog.trainable_units(EntityKind::CityCentre).is_empty());
        assert!(!catalog.allows_research(UpgradeKind::Methamphetamines, EntityKind::TrainingCentre));
        assert!(!catalog.allows_research(UpgradeKind::Entrenchment, EntityKind::TrainingCentre));
        assert!(!catalog.allows_ability(AbilityKind::Smoke, EntityKind::ScoutCar));
    }

    #[test]
    fn unknown_non_empty_catalog_ids_fail_closed() {
        assert!(catalog_for("unknown_faction").is_none());
        assert!(catalog_for_or_default_empty("unknown_faction").is_none());
        assert_eq!(
            catalog_for_or_default_empty("").unwrap().id,
            DEFAULT_FACTION_ID
        );
        assert!(catalog_loadout_for("unknown_faction", "kriegsia.standard").is_none());
        assert!(catalog_loadout_for(DEFAULT_FACTION_ID, "missing.loadout").is_none());
        assert!(catalog_loadout_for(DEFAULT_FACTION_ID, "kriegsia.standard").is_some());
        assert!(catalog_loadout_for(EKAT_FACTION_ID, "ekat.standard").is_some());
    }

    #[test]
    fn typed_ability_and_upgrade_ids_are_unique_and_round_trip() {
        for (index, kind) in AbilityKind::ALL.iter().copied().enumerate() {
            let id = kind.stable_id();
            assert_eq!(id.parse::<AbilityKind>(), Ok(kind));
            assert!(!AbilityKind::ALL[..index]
                .iter()
                .any(|prior| prior.stable_id() == id));
            assert_eq!(ability_definition(kind).kind, kind);
        }
        assert!("unknownAbility".parse::<AbilityKind>().is_err());

        for (index, kind) in UpgradeKind::ALL.iter().copied().enumerate() {
            let id = kind.stable_id();
            assert_eq!(id.parse::<UpgradeKind>(), Ok(kind));
            assert!(!UpgradeKind::ALL[..index]
                .iter()
                .any(|prior| prior.stable_id() == id));
            assert_eq!(upgrade_definition(kind).kind, kind);
        }
        assert!("unknown_upgrade".parse::<UpgradeKind>().is_err());
    }

    #[test]
    fn faction_catalogs_define_starting_loadouts() {
        assert_eq!(CURRENT_CATALOG.loadout.id, "kriegsia.standard");
        assert_eq!(
            CURRENT_CATALOG.loadout.initial_steel,
            crate::balance::STARTING_STEEL
        );
        assert_eq!(
            CURRENT_CATALOG.loadout.initial_oil,
            crate::balance::STARTING_OIL
        );
        assert_eq!(
            CURRENT_CATALOG.loadout.starting_entities,
            CURRENT_STANDARD_START_ENTITIES
        );

        assert_eq!(EMPTY_FIXTURE_CATALOG.loadout.initial_steel, 125);
        assert_eq!(EMPTY_FIXTURE_CATALOG.loadout.initial_oil, 25);
        assert_eq!(
            EMPTY_FIXTURE_CATALOG.loadout.starting_entities,
            EMPTY_FIXTURE_START_ENTITIES
        );

        assert_eq!(EKAT_CATALOG.loadout.initial_steel, 0);
        assert_eq!(EKAT_CATALOG.loadout.initial_oil, 0);
        assert_eq!(EKAT_CATALOG.loadout.starting_entities, EKAT_START_ENTITIES);
    }
}
