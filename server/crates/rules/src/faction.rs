//! Faction-aware catalog queries.
//!
//! Factions share global kind, upgrade, ability, and resource identities. Reuse an id only for
//! identical gameplay semantics; divergent behavior needs a distinct, faction-gated id.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{balance, defs, economy::ResourceCost, EntityKind};

pub const DEFAULT_FACTION_ID: &str = "kriegsia";
pub const EKAT_FACTION_ID: &str = "ekat";
pub const CULTIVATORS_FACTION_ID: &str = "cultivators";
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
    TankPlus,
    SmokePlus,
    ScoutPlaneUnlock,
    Rockets,
    DaoWind,
    DaoOfMagic,
    DaoOfHeaviness,
    TechniqueCharge,
    TechniqueBlink,
    TechniqueShield,
    TechniqueThrowSword,
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
        Self::TankPlus,
        Self::SmokePlus,
        Self::ScoutPlaneUnlock,
        Self::Rockets,
        Self::DaoWind,
        Self::DaoOfMagic,
        Self::DaoOfHeaviness,
        Self::TechniqueCharge,
        Self::TechniqueBlink,
        Self::TechniqueShield,
        Self::TechniqueThrowSword,
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
            Self::TankPlus => "tank_plus",
            Self::SmokePlus => "smoke_plus",
            Self::ScoutPlaneUnlock => "scout_plane_unlock",
            Self::Rockets => "rockets",
            Self::DaoWind => "dao_wind",
            Self::DaoOfMagic => "dao_of_magic",
            Self::DaoOfHeaviness => "dao_of_heaviness",
            Self::TechniqueCharge => "technique_charge",
            Self::TechniqueBlink => "technique_blink",
            Self::TechniqueShield => "technique_shield",
            Self::TechniqueThrowSword => "technique_throw_sword",
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
    VectorField,
    EkatConsumeGolem,
    Barrage,
    EvolveWind,
    EvolveWar,
    EvolveMagic,
    DiscipleShield,
    ThrowSword,
    Regen,
    HealingSeal,
    EscapeTreasure,
    WindDash,
    Haste,
    DoubleDamage,
    ArmorTreasure,
    VitalityTreasure,
    DoubleXp,
    BreakthroughTreasure,
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
        Self::VectorField,
        Self::EkatConsumeGolem,
        Self::Barrage,
        Self::EvolveWind,
        Self::EvolveWar,
        Self::EvolveMagic,
        Self::DiscipleShield,
        Self::ThrowSword,
        Self::Regen,
        Self::HealingSeal,
        Self::EscapeTreasure,
        Self::WindDash,
        Self::Haste,
        Self::DoubleDamage,
        Self::ArmorTreasure,
        Self::VitalityTreasure,
        Self::DoubleXp,
        Self::BreakthroughTreasure,
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
            Self::VectorField => "vectorField",
            Self::EkatConsumeGolem => "ekatConsumeGolem",
            Self::Barrage => "barrage",
            Self::EvolveWind => "evolveWind",
            Self::EvolveWar => "evolveWar",
            Self::EvolveMagic => "evolveMagic",
            Self::DiscipleShield => "discipleShield",
            Self::ThrowSword => "throwSword",
            Self::Regen => "regen",
            Self::HealingSeal => "healingSeal",
            Self::EscapeTreasure => "escapeTreasure",
            Self::WindDash => "windDash",
            Self::Haste => "haste",
            Self::DoubleDamage => "doubleDamage",
            Self::ArmorTreasure => "armorTreasure",
            Self::VitalityTreasure => "vitalityTreasure",
            Self::DoubleXp => "doubleXp",
            Self::BreakthroughTreasure => "breakthroughTreasure",
        }
    }

    pub const fn is_dash(self) -> bool {
        matches!(self, Self::EkatTeleport | Self::WindDash)
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
pub const TANK_PLUS_UPGRADE: &str = UpgradeKind::TankPlus.stable_id();
pub const SMOKE_PLUS_UPGRADE: &str = UpgradeKind::SmokePlus.stable_id();
pub const SCOUT_PLANE_UNLOCK_UPGRADE: &str = UpgradeKind::ScoutPlaneUnlock.stable_id();
pub const ROCKETS_UPGRADE: &str = UpgradeKind::Rockets.stable_id();

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
pub const BARRAGE_ABILITY: &str = AbilityKind::Barrage.stable_id();

const CURRENT_STANDARD_START_ENTITIES: &[StartingEntityGroup] = &[
    StartingEntityGroup {
        kind: EntityKind::ResourceDepot,
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
    StartingEntityGroup {
        kind: EntityKind::Rifleman,
        count: crate::balance::STARTING_RIFLEMEN,
        formation: StartingFormation::DefensiveLine {
            distance_tiles_x10: 40,
            spacing_tiles_x10: 10,
        },
        completed: true,
    },
    StartingEntityGroup {
        kind: EntityKind::SteelMine,
        count: crate::balance::STARTING_STEEL_MINES,
        formation: StartingFormation::ResourcePatches,
        completed: true,
    },
    StartingEntityGroup {
        kind: EntityKind::PumpJack,
        count: crate::balance::STARTING_PUMP_JACKS,
        formation: StartingFormation::ResourcePatches,
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

// Reuse the standard economy with two starting Cultivator Novices.
const CULTIVATORS_START_ENTITIES: &[StartingEntityGroup] = &[
    CURRENT_STANDARD_START_ENTITIES[0],
    CURRENT_STANDARD_START_ENTITIES[1],
    StartingEntityGroup {
        kind: EntityKind::Warrior,
        count: 2,
        formation: StartingFormation::DefensiveLine {
            distance_tiles_x10: 50,
            spacing_tiles_x10: 15,
        },
        completed: true,
    },
    CURRENT_STANDARD_START_ENTITIES[3],
    CURRENT_STANDARD_START_ENTITIES[4],
];
pub const CULTIVATORS_LOADOUT: FactionLoadout = FactionLoadout {
    id: "cultivators.standard",
    initial_steel: balance::STARTING_STEEL,
    initial_oil: balance::STARTING_OIL,
    starting_entities: CULTIVATORS_START_ENTITIES,
    opening_upgrades: &[],
};

const DEFAULT_UNITS: &[EntityKind] = &[
    EntityKind::Worker,
    EntityKind::Rifleman,
    EntityKind::Panzerfaust,
    EntityKind::MachineGunner,
    EntityKind::AntiTankGun,
    EntityKind::MortarTeam,
    EntityKind::Artillery,
    EntityKind::RocketLauncher,
    EntityKind::Tank,
    EntityKind::ScoutCar,
    EntityKind::ScoutPlane,
    EntityKind::CommandCar,
];

const DEFAULT_BUILDINGS: &[EntityKind] = &[
    EntityKind::ResourceDepot,
    // Retained for replay and fixture compatibility, but unavailable in the current build catalog.
    EntityKind::Depot,
    EntityKind::Barracks,
    EntityKind::TrainingCentre,
    EntityKind::Factory,
    EntityKind::EngineeringComplex,
    EntityKind::Steelworks,
    EntityKind::TankTrap,
    EntityKind::BarbedWire,
    EntityKind::SteelMine,
    EntityKind::PumpJack,
];

const DEFAULT_WORKER_BUILDABLES: &[EntityKind] = &[
    EntityKind::ResourceDepot,
    EntityKind::PumpJack,
    EntityKind::Barracks,
    EntityKind::TrainingCentre,
    EntityKind::EngineeringComplex,
    EntityKind::Factory,
    EntityKind::Steelworks,
    EntityKind::TankTrap,
    EntityKind::BarbedWire,
];

const ARTILLERY_ABILITY_CARRIERS: &[EntityKind] = &[EntityKind::Artillery];
const ROCKET_LAUNCHER_ABILITY_CARRIERS: &[EntityKind] = &[EntityKind::RocketLauncher];
const SCOUT_PLANE_COMMAND_CAR_CARRIER: EntityKind = EntityKind::CommandCar;
const SCOUT_PLANE_ABILITY_CARRIERS: &[EntityKind] = &[SCOUT_PLANE_COMMAND_CAR_CARRIER];
const SCOUT_PLANE_CARRIER_COSTS: &[(EntityKind, ResourceCost)] = &[(
    SCOUT_PLANE_COMMAND_CAR_CARRIER,
    ResourceCost::new(
        balance::SCOUT_PLANE_COMMAND_CAR_COST_STEEL,
        balance::SCOUT_PLANE_COMMAND_CAR_COST_OIL,
    ),
)];

const LEGACY_ANTI_TANK_GUN_UNLOCK: UpgradeCatalogEntry = UpgradeCatalogEntry {
    kind: UpgradeKind::AntiTankGunUnlock,
    researched_at: EntityKind::EngineeringComplex,
};

const DEFAULT_UPGRADES: [UpgradeCatalogEntry; 10] = [
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
        kind: UpgradeKind::ArtilleryUnlock,
        researched_at: EntityKind::EngineeringComplex,
    },
    UpgradeCatalogEntry {
        kind: UpgradeKind::BallisticTables,
        researched_at: EntityKind::EngineeringComplex,
    },
    UpgradeCatalogEntry {
        kind: UpgradeKind::TankUnlock,
        researched_at: EntityKind::EngineeringComplex,
    },
    UpgradeCatalogEntry {
        kind: UpgradeKind::SmokePlus,
        researched_at: EntityKind::EngineeringComplex,
    },
    UpgradeCatalogEntry {
        kind: UpgradeKind::ScoutPlaneUnlock,
        researched_at: EntityKind::EngineeringComplex,
    },
    UpgradeCatalogEntry {
        kind: UpgradeKind::Rockets,
        researched_at: EntityKind::EngineeringComplex,
    },
    UpgradeCatalogEntry {
        kind: UpgradeKind::TankPlus,
        researched_at: EntityKind::EngineeringComplex,
    },
];

const DEFAULT_ABILITIES: [AbilityCatalogEntry; 9] = [
    AbilityCatalogEntry {
        kind: AbilityKind::Charge,
        label: "Charge",
        icon: "CHG",
        hotkey: None,
        title: "Charge at the enemy. Auto cast, 15-second cooldown.",
        carriers: &[EntityKind::Warrior, EntityKind::WarAdept],
        target_mode: AbilityTargetMode::SelfTarget,
        range_tiles: None,
        min_range_tiles: None,
        cooldown_ticks: balance::NOVICE_CHARGE_COOLDOWN_TICKS,
        charges: None,
        charge_recharge_ticks: None,
        cost: ResourceCost::new(0, 0),
        tech_requirement: None,
        upgrade_requirement: Some(UpgradeKind::TechniqueCharge),
        queue_policy: AbilityQueuePolicy::NotQueueable,
        autocast: false,
        command_card: true,
        protocol_code: 1,
        order_stage_code: 8,
    },
    AbilityCatalogEntry {
        kind: AbilityKind::Smoke,
        label: "Smoke",
        icon: "SMK",
        hotkey: Some("D"),
        title: "Hide and deny vision.",
        carriers: &[EntityKind::ScoutCar],
        target_mode: AbilityTargetMode::WorldPoint,
        range_tiles: Some(balance::SMOKE_ABILITY_RANGE_TILES as f32),
        min_range_tiles: None,
        cooldown_ticks: balance::SMOKE_ABILITY_COOLDOWN_TICKS,
        charges: Some(balance::SCOUT_CAR_SMOKE_CHARGES),
        charge_recharge_ticks: Some(balance::SCOUT_CAR_SMOKE_CHARGE_RECHARGE_TICKS),
        cost: ResourceCost::new(
            balance::SMOKE_ABILITY_COST_STEEL,
            balance::SMOKE_ABILITY_COST_OIL,
        ),
        tech_requirement: Some(EntityKind::EngineeringComplex),
        upgrade_requirement: None,
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
        title: "Blow them up.",
        carriers: &[EntityKind::MortarTeam],
        target_mode: AbilityTargetMode::WorldPoint,
        range_tiles: Some(balance::MORTAR_RANGE_TILES as f32),
        min_range_tiles: Some(balance::MORTAR_MIN_RANGE_TILES),
        cooldown_ticks: balance::MORTAR_FIRE_COOLDOWN_TICKS as u16,
        charges: None,
        charge_recharge_ticks: None,
        cost: ResourceCost::new(0, 0),
        tech_requirement: None,
        upgrade_requirement: None,
        queue_policy: AbilityQueuePolicy::QueueWaitUntilReady,
        autocast: false,
        command_card: true,
        protocol_code: 3,
        order_stage_code: 9,
    },
    AbilityCatalogEntry {
        kind: AbilityKind::PointFire,
        label: "Fire",
        icon: "FIR",
        hotkey: Some("X"),
        title: "Click a target, then click again to set the fire radius",
        carriers: ARTILLERY_ABILITY_CARRIERS,
        target_mode: AbilityTargetMode::WorldPoint,
        range_tiles: Some(balance::ARTILLERY_MAX_RANGE_TILES as f32),
        min_range_tiles: Some(balance::ARTILLERY_MIN_RANGE_TILES),
        cooldown_ticks: balance::ARTILLERY_RELOAD_TICKS as u16,
        charges: None,
        charge_recharge_ticks: None,
        cost: ResourceCost::new(balance::ARTILLERY_AMMO_COST_STEEL, 0),
        tech_requirement: None,
        upgrade_requirement: None,
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
        range_tiles: Some(balance::ARTILLERY_MAX_RANGE_TILES as f32),
        min_range_tiles: Some(balance::ARTILLERY_MIN_RANGE_TILES),
        cooldown_ticks: balance::ARTILLERY_RELOAD_TICKS as u16,
        charges: None,
        charge_recharge_ticks: None,
        cost: ResourceCost::new(balance::ARTILLERY_AMMO_COST_STEEL, 0),
        tech_requirement: None,
        upgrade_requirement: None,
        queue_policy: AbilityQueuePolicy::QueueSkipIfNotReady,
        autocast: false,
        command_card: false,
        protocol_code: 10,
        order_stage_code: 17,
    },
    AbilityCatalogEntry {
        kind: AbilityKind::Breakthrough,
        label: "Breakthrough!",
        icon: "BRK",
        hotkey: Some("E"),
        title: "Area of effect speed up. Combos with smoke.",
        carriers: &[EntityKind::CommandCar],
        target_mode: AbilityTargetMode::SelfTarget,
        range_tiles: None,
        min_range_tiles: None,
        cooldown_ticks: balance::BREAKTHROUGH_COOLDOWN_TICKS,
        charges: None,
        charge_recharge_ticks: None,
        cost: ResourceCost::new(0, 0),
        tech_requirement: None,
        upgrade_requirement: None,
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
        title: "Scout the enemy.",
        carriers: SCOUT_PLANE_ABILITY_CARRIERS,
        target_mode: AbilityTargetMode::WorldPoint,
        range_tiles: None,
        min_range_tiles: None,
        cooldown_ticks: balance::SCOUT_PLANE_ABILITY_COOLDOWN_TICKS,
        charges: None,
        charge_recharge_ticks: None,
        cost: ResourceCost::new(
            balance::SCOUT_PLANE_COST_STEEL,
            balance::SCOUT_PLANE_COST_OIL,
        ),
        tech_requirement: None,
        upgrade_requirement: Some(UpgradeKind::ScoutPlaneUnlock),
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
        charge_recharge_ticks: None,
        cost: ResourceCost::new(0, 0),
        tech_requirement: None,
        upgrade_requirement: None,
        queue_policy: AbilityQueuePolicy::NotQueueable,
        autocast: false,
        command_card: false,
        protocol_code: 11,
        order_stage_code: 18,
    },
    AbilityCatalogEntry {
        kind: AbilityKind::Barrage,
        label: "Barrage",
        icon: "RKT",
        hotkey: Some("X"),
        title: "Blow them up.",
        carriers: ROCKET_LAUNCHER_ABILITY_CARRIERS,
        target_mode: AbilityTargetMode::WorldPoint,
        range_tiles: Some(balance::ROCKET_BARRAGE_MAX_RANGE_TILES as f32),
        min_range_tiles: Some(balance::ROCKET_BARRAGE_MIN_RANGE_TILES),
        cooldown_ticks: balance::ROCKET_BARRAGE_RELOAD_TICKS,
        charges: Some(1),
        charge_recharge_ticks: None,
        cost: ResourceCost::new(0, balance::ROCKET_BARRAGE_COST_OIL),
        tech_requirement: None,
        upgrade_requirement: Some(UpgradeKind::Rockets),
        queue_policy: AbilityQueuePolicy::QueueWaitUntilReady,
        autocast: false,
        command_card: true,
        protocol_code: 13,
        order_stage_code: 21,
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
        range_tiles: Some(balance::EKAT_TELEPORT_RANGE_TILES as f32),
        min_range_tiles: None,
        cooldown_ticks: balance::EKAT_TELEPORT_COOLDOWN_TICKS,
        charges: None,
        charge_recharge_ticks: None,
        cost: ResourceCost::new(0, 0),
        tech_requirement: None,
        upgrade_requirement: None,
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
        range_tiles: Some(balance::EKAT_LINE_SHOT_RANGE_TILES as f32),
        min_range_tiles: None,
        cooldown_ticks: balance::EKAT_LINE_SHOT_COOLDOWN_TICKS,
        charges: None,
        charge_recharge_ticks: None,
        cost: ResourceCost::new(0, 0),
        tech_requirement: None,
        upgrade_requirement: None,
        queue_policy: AbilityQueuePolicy::QueueSkipIfNotReady,
        autocast: false,
        command_card: true,
        protocol_code: 7,
        order_stage_code: 13,
    },
    AbilityCatalogEntry {
        kind: AbilityKind::EkatMagicAnchor,
        label: "Anchor",
        icon: "ANC",
        hotkey: Some("C"),
        title: "Place a 10-second pull field",
        carriers: &[EntityKind::Ekat],
        target_mode: AbilityTargetMode::WorldPoint,
        range_tiles: Some(balance::EKAT_MAGIC_ANCHOR_RANGE_TILES),
        min_range_tiles: None,
        cooldown_ticks: 0,
        charges: None,
        charge_recharge_ticks: None,
        cost: ResourceCost::new(0, 0),
        tech_requirement: None,
        upgrade_requirement: None,
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
        range_tiles: Some(balance::EKAT_CONSUME_GOLEM_RANGE_TILES as f32),
        min_range_tiles: None,
        cooldown_ticks: 0,
        charges: None,
        charge_recharge_ticks: None,
        cost: ResourceCost::new(0, 0),
        tech_requirement: None,
        upgrade_requirement: None,
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
    gatherers: &[],
    production_anchors: &[
        EntityKind::ResourceDepot,
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

const CULTIVATORS_UPGRADES: [UpgradeCatalogEntry; 6] = [
    UpgradeCatalogEntry {
        kind: UpgradeKind::DaoWind,
        researched_at: EntityKind::ThreeDaoPavilion,
    },
    UpgradeCatalogEntry {
        kind: UpgradeKind::DaoOfMagic,
        researched_at: EntityKind::ThreeDaoPavilion,
    },
    UpgradeCatalogEntry {
        kind: UpgradeKind::DaoOfHeaviness,
        researched_at: EntityKind::ThreeDaoPavilion,
    },
    UpgradeCatalogEntry {
        kind: UpgradeKind::TechniqueCharge,
        researched_at: EntityKind::TechniqueLibrary,
    },
    UpgradeCatalogEntry {
        kind: UpgradeKind::TechniqueShield,
        researched_at: EntityKind::TechniqueLibrary,
    },
    UpgradeCatalogEntry {
        kind: UpgradeKind::TechniqueThrowSword,
        researched_at: EntityKind::TechniqueLibrary,
    },
];

mod ability_targeting;
mod dao;
pub use ability_targeting::{AbilityQueuePolicy, AbilityTargetMode};
use dao::CULTIVATOR_ABILITIES;

pub const CULTIVATORS_CATALOG: FactionCatalog = FactionCatalog {
    id: CULTIVATORS_FACTION_ID,
    loadout: CULTIVATORS_LOADOUT,
    units: &[
        EntityKind::Worker,
        EntityKind::Warrior,
        EntityKind::WindAdept,
        EntityKind::WarAdept,
        EntityKind::MagicAdept,
    ],
    buildings: &[
        EntityKind::ResourceDepot,
        EntityKind::Portal,
        EntityKind::ThreeDaoPavilion,
        EntityKind::TechniqueLibrary,
        EntityKind::TournamentGrounds,
        EntityKind::Shop,
        EntityKind::HealingSeal,
        EntityKind::SteelMine,
        EntityKind::PumpJack,
    ],
    buildables: &[
        EntityKind::ResourceDepot,
        EntityKind::PumpJack,
        EntityKind::Portal,
        EntityKind::ThreeDaoPavilion,
        EntityKind::TechniqueLibrary,
        EntityKind::TournamentGrounds,
        EntityKind::Shop,
    ],
    upgrades: &CULTIVATORS_UPGRADES,
    abilities: &CULTIVATOR_ABILITIES,
    builders: &[EntityKind::Worker],
    gatherers: &[],
    production_anchors: &[
        EntityKind::ResourceDepot,
        EntityKind::Portal,
        EntityKind::TournamentGrounds,
    ],
};

pub const CATALOGS: &[FactionCatalog] = &[
    CURRENT_CATALOG,
    EKAT_CATALOG,
    CULTIVATORS_CATALOG,
    EMPTY_FIXTURE_CATALOG,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpgradeCatalogEntry {
    pub kind: UpgradeKind,
    pub researched_at: EntityKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbilityCatalogEntry {
    pub kind: AbilityKind,
    pub label: &'static str,
    pub icon: &'static str,
    pub hotkey: Option<&'static str>,
    pub title: &'static str,
    pub carriers: &'static [EntityKind],
    pub target_mode: AbilityTargetMode,
    pub range_tiles: Option<f32>,
    pub min_range_tiles: Option<u32>,
    pub cooldown_ticks: u16,
    pub charges: Option<u16>,
    pub charge_recharge_ticks: Option<u16>,
    pub cost: ResourceCost,
    pub tech_requirement: Option<EntityKind>,
    pub upgrade_requirement: Option<UpgradeKind>,
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
    Ring {
        radius_tiles_x10: u32,
    },
    DefensiveLine {
        distance_tiles_x10: u32,
        spacing_tiles_x10: u32,
    },
    ResourcePatches,
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

#[derive(Debug, Clone, Copy, PartialEq)]
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
            .filter(|unit| self.allows_unit(*unit) || self.allows_building(*unit))
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
    let [charge, smoke, mortar_fire, point_fire, blanket_fire, breakthrough, scout_plane, dismiss_scout_plane, barrage] =
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
        AbilityKind::VectorField => CULTIVATOR_ABILITIES[4],
        AbilityKind::EkatConsumeGolem => ekat_consume_golem,
        AbilityKind::Barrage => barrage,
        AbilityKind::EvolveWind => CULTIVATOR_ABILITIES[0],
        AbilityKind::EvolveWar => CULTIVATOR_ABILITIES[1],
        AbilityKind::EvolveMagic => CULTIVATOR_ABILITIES[2],
        AbilityKind::DiscipleShield => CULTIVATOR_ABILITIES[5],
        AbilityKind::ThrowSword => CULTIVATOR_ABILITIES[6],
        AbilityKind::Regen => CULTIVATOR_ABILITIES[7],
        AbilityKind::HealingSeal => CULTIVATOR_ABILITIES[10],
        AbilityKind::EscapeTreasure => CULTIVATOR_ABILITIES[8],
        AbilityKind::WindDash => CULTIVATOR_ABILITIES[3],
        AbilityKind::Haste => CULTIVATOR_ABILITIES[11],
        AbilityKind::DoubleDamage => CULTIVATOR_ABILITIES[12],
        AbilityKind::ArmorTreasure => CULTIVATOR_ABILITIES[13],
        AbilityKind::VitalityTreasure => CULTIVATOR_ABILITIES[14],
        AbilityKind::DoubleXp => CULTIVATOR_ABILITIES[15],
        AbilityKind::BreakthroughTreasure => CULTIVATOR_ABILITIES[16],
    }
}

pub fn ability_range_for_carrier(kind: AbilityKind, carrier: EntityKind) -> Option<f32> {
    if kind == AbilityKind::WindDash && carrier == EntityKind::WindAdept {
        Some(balance::DAO_WIND_DASH_RANGE_TILES as f32)
    } else {
        ability_definition(kind).range_tiles
    }
}

pub fn ability_cost_for_carrier(kind: AbilityKind, carrier: EntityKind) -> ResourceCost {
    let definition = ability_definition(kind);
    let carrier_costs = match kind {
        AbilityKind::ScoutPlane => SCOUT_PLANE_CARRIER_COSTS,
        _ => &[],
    };
    carrier_costs
        .iter()
        .find_map(|(candidate, cost)| (*candidate == carrier).then_some(*cost))
        .unwrap_or(definition.cost)
}

pub fn upgrade_definition(kind: UpgradeKind) -> UpgradeCatalogEntry {
    let [methamphetamines, panzerfausts, entrenchment, artillery_unlock, ballistic_tables, tank_unlock, smoke_plus, scout_plane_unlock, rockets, tank_plus] =
        DEFAULT_UPGRADES;
    match kind {
        UpgradeKind::Methamphetamines => methamphetamines,
        UpgradeKind::Panzerfausts => panzerfausts,
        UpgradeKind::Entrenchment => entrenchment,
        UpgradeKind::AntiTankGunUnlock => LEGACY_ANTI_TANK_GUN_UNLOCK,
        UpgradeKind::BallisticTables => ballistic_tables,
        UpgradeKind::TankUnlock => tank_unlock,
        UpgradeKind::TankPlus => tank_plus,
        UpgradeKind::SmokePlus => smoke_plus,
        UpgradeKind::ArtilleryUnlock => artillery_unlock,
        UpgradeKind::ScoutPlaneUnlock => scout_plane_unlock,
        UpgradeKind::Rockets => rockets,
        UpgradeKind::TechniqueCharge => CULTIVATORS_UPGRADES[3],
        // Retained vocabulary for stored fixtures; unavailable in live catalogs.
        UpgradeKind::TechniqueBlink => UpgradeCatalogEntry {
            kind,
            researched_at: EntityKind::TechniqueLibrary,
        },
        UpgradeKind::TechniqueShield => CULTIVATORS_UPGRADES[4],
        UpgradeKind::TechniqueThrowSword => CULTIVATORS_UPGRADES[5],
        UpgradeKind::DaoWind => CULTIVATORS_UPGRADES[0],
        UpgradeKind::DaoOfMagic => CULTIVATORS_UPGRADES[1],
        UpgradeKind::DaoOfHeaviness => CULTIVATORS_UPGRADES[2],
    }
}

#[cfg(test)]
mod tests;
