use super::*;

#[test]
fn default_catalog_matches_defs_inventory() {
    assert_eq!(CURRENT_CATALOG.units, DEFAULT_UNITS);

    assert_eq!(CURRENT_CATALOG.buildings, DEFAULT_BUILDINGS);
}

#[test]
fn default_catalog_routes_current_tech_tree() {
    let catalog = CURRENT_CATALOG;
    let engineering_complex = EntityKind::EngineeringComplex;

    assert_eq!(
        catalog.trainable_units(EntityKind::ResourceDepot),
        vec![
            EntityKind::Worker,
            EntityKind::SteelMine,
            EntityKind::PumpJack
        ]
    );
    assert!(
        catalog.allows_unit(EntityKind::ScoutPlane),
        "Scout Plane remains in the catalog for ability-launched mission entities"
    );
    assert_eq!(
        catalog.trainable_units(EntityKind::Barracks),
        vec![
            EntityKind::Rifleman,
            EntityKind::MachineGunner,
            EntityKind::Panzerfaust
        ]
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
            EntityKind::Artillery,
            EntityKind::RocketLauncher
        ]
    );
    assert!(catalog.allows_research(UpgradeKind::Methamphetamines, EntityKind::TrainingCentre));
    assert!(catalog.allows_research(UpgradeKind::Entrenchment, EntityKind::TrainingCentre));
    assert!(!catalog.allows_research(UpgradeKind::AntiTankGunUnlock, engineering_complex));
    assert!(catalog.allows_research(UpgradeKind::BallisticTables, engineering_complex));
    assert!(catalog.allows_research(UpgradeKind::ArtilleryUnlock, engineering_complex));
    assert!(catalog.allows_research(UpgradeKind::TankUnlock, engineering_complex));
    assert!(catalog.allows_research(UpgradeKind::SmokePlus, engineering_complex));
    assert!(catalog.allows_research(UpgradeKind::Rockets, engineering_complex));
    assert!(!catalog.allows_research(UpgradeKind::TankUnlock, EntityKind::TrainingCentre));
    assert!(catalog.allows_building(EntityKind::TankTrap));
    assert!(catalog.can_build(EntityKind::Worker, EntityKind::TankTrap));
    assert!(catalog.allows_building(EntityKind::PumpJack));
    assert!(catalog.can_build(EntityKind::Worker, EntityKind::PumpJack));
    assert!(!catalog.can_build(EntityKind::Worker, EntityKind::SteelMine));
    assert!(!catalog.can_act_as_production_anchor(EntityKind::TankTrap));
    assert!(!catalog.can_act_as_production_anchor(EntityKind::PumpJack));
    assert!(!catalog.can_act_as_production_anchor(EntityKind::SteelMine));
    assert!(catalog.allows_ability(AbilityKind::Smoke, EntityKind::ScoutCar));
    assert!(catalog.allows_ability(AbilityKind::PointFire, ARTILLERY_ABILITY_CARRIERS[0]));
    assert!(catalog.allows_ability(AbilityKind::BlanketFire, ARTILLERY_ABILITY_CARRIERS[0]));
    for carrier in SCOUT_PLANE_ABILITY_CARRIERS {
        assert!(catalog.allows_ability(AbilityKind::ScoutPlane, *carrier));
    }
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
    assert!(!catalog.allows_building(EntityKind::ResourceDepot));
}

#[test]
fn default_ability_registry_preserves_current_metadata() {
    let smoke = CURRENT_CATALOG.ability(AbilityKind::Smoke).unwrap();
    assert_eq!(smoke.label, "Smoke");
    assert_eq!(smoke.carriers, &[EntityKind::ScoutCar]);
    assert_eq!(smoke.target_mode, AbilityTargetMode::WorldPoint);
    assert_eq!(
        smoke.range_tiles,
        Some(balance::SMOKE_ABILITY_RANGE_TILES as f32)
    );
    assert_eq!(smoke.charges, Some(balance::SCOUT_CAR_SMOKE_CHARGES));
    assert_eq!(
        smoke.charge_recharge_ticks,
        Some(balance::SCOUT_CAR_SMOKE_CHARGE_RECHARGE_TICKS)
    );
    assert_eq!(
        smoke.tech_requirement,
        Some(EntityKind::EngineeringComplex),
        "Smoke should require a completed Engineering Complex"
    );
    assert_eq!(
        smoke.cost,
        ResourceCost::new(
            balance::SMOKE_ABILITY_COST_STEEL,
            balance::SMOKE_ABILITY_COST_OIL,
        )
    );
    assert!(smoke.command_card);

    let scout_plane = CURRENT_CATALOG.ability(AbilityKind::ScoutPlane).unwrap();
    assert_eq!(scout_plane.carriers, SCOUT_PLANE_ABILITY_CARRIERS);
    assert_eq!(
        scout_plane.upgrade_requirement,
        Some(UpgradeKind::ScoutPlaneUnlock),
        "Scout Plane should require its Engineering Complex upgrade"
    );
    assert!(!CURRENT_CATALOG.allows_ability(AbilityKind::ScoutPlane, EntityKind::ResourceDepot));
    assert_eq!(
        ability_cost_for_carrier(AbilityKind::ScoutPlane, SCOUT_PLANE_COMMAND_CAR_CARRIER),
        ResourceCost::new(0, 0)
    );

    let point_fire = CURRENT_CATALOG.ability(AbilityKind::PointFire).unwrap();
    assert_eq!(point_fire.carriers, ARTILLERY_ABILITY_CARRIERS);
    assert_eq!(
        point_fire.min_range_tiles,
        Some(balance::ARTILLERY_MIN_RANGE_TILES)
    );
    assert_eq!(
        point_fire.range_tiles,
        Some(balance::ARTILLERY_MAX_RANGE_TILES as f32)
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
        Some(balance::ARTILLERY_MAX_RANGE_TILES as f32)
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
    assert!(!blanket_fire.command_card);
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
    assert!(charge.command_card);
    let charge_carriers = &[EntityKind::Warrior, EntityKind::WarAdept];
    assert_eq!(charge.carriers, charge_carriers);
    assert_eq!(charge.cooldown_ticks, balance::NOVICE_CHARGE_COOLDOWN_TICKS);

    let teleport = EKAT_CATALOG.ability(AbilityKind::EkatTeleport).unwrap();
    assert_eq!(teleport.carriers, &[EntityKind::Ekat]);
    assert_eq!(
        teleport.range_tiles,
        Some(balance::EKAT_TELEPORT_RANGE_TILES as f32)
    );

    let line_shot = EKAT_CATALOG.ability(AbilityKind::EkatLineShot).unwrap();
    assert_eq!(line_shot.carriers, &[EntityKind::Ekat]);
    assert_eq!(
        line_shot.range_tiles,
        Some(balance::EKAT_LINE_SHOT_RANGE_TILES as f32)
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
        Some(balance::EKAT_CONSUME_GOLEM_RANGE_TILES as f32)
    );
    assert_eq!(consume.protocol_code, 9);
    assert_eq!(consume.order_stage_code, 16);
}

#[test]
fn fixture_catalog_rejects_global_current_kinds() {
    let catalog = EMPTY_FIXTURE_CATALOG;

    assert!(!catalog.allows_unit(EntityKind::Worker));
    assert!(!catalog.allows_building(EntityKind::ResourceDepot));
    assert!(!catalog.can_build(EntityKind::Worker, EntityKind::Depot));
    assert!(catalog.allows_unit(EntityKind::ScoutCar));
    assert!(catalog.allows_building(EntityKind::Depot));
    assert!(catalog
        .trainable_units(EntityKind::ResourceDepot)
        .is_empty());
    assert!(!catalog.allows_research(UpgradeKind::Methamphetamines, EntityKind::TrainingCentre));
    assert!(!catalog.allows_research(UpgradeKind::Entrenchment, EntityKind::TrainingCentre));
    assert!(!catalog.allows_ability(AbilityKind::Smoke, EntityKind::ScoutCar));
}

#[test]
fn cultivators_catalog_trains_warriors_from_portals() {
    let catalog = catalog_for(CULTIVATORS_FACTION_ID).unwrap();
    assert_eq!(catalog.loadout.id, "cultivators.standard");
    assert!(!catalog.can_build(EntityKind::Worker, EntityKind::ProvenGrounds));
    assert!(!catalog.buildings.contains(&EntityKind::ProvenGrounds));
    assert!(!catalog
        .production_anchors
        .contains(&EntityKind::ProvenGrounds));
    assert_eq!(catalog.loadout.initial_steel, balance::STARTING_STEEL);
    assert_eq!(catalog.loadout.initial_oil, balance::STARTING_OIL);
    assert_eq!(
        catalog.units,
        &[
            EntityKind::Worker,
            EntityKind::Warrior,
            EntityKind::WindAdept,
            EntityKind::WarAdept,
            EntityKind::MagicAdept
        ]
    );
    assert_eq!(
        catalog.buildables,
        &[
            EntityKind::ResourceDepot,
            EntityKind::PumpJack,
            EntityKind::Portal,
            EntityKind::ThreeDaoPavilion,
            EntityKind::TechniqueLibrary,
            EntityKind::TournamentGrounds,
            EntityKind::Shop
        ]
    );
    assert_eq!(
        catalog.trainable_units(EntityKind::ResourceDepot),
        vec![
            EntityKind::Worker,
            EntityKind::SteelMine,
            EntityKind::PumpJack
        ]
    );
    assert!(catalog.trainable_units(EntityKind::Barracks).is_empty());
    assert_eq!(
        catalog.trainable_units(EntityKind::Portal),
        catalog.units[1..].to_vec()
    );
    assert!(catalog
        .loadout
        .starting_entities
        .iter()
        .all(|group| catalog.allows_unit(group.kind) || catalog.allows_building(group.kind)));
    assert_eq!(catalog.upgrades, &CULTIVATORS_UPGRADES);
    assert_eq!(catalog.abilities, &CULTIVATOR_ABILITIES);
    for ability in [
        AbilityKind::EvolveWind,
        AbilityKind::EvolveWar,
        AbilityKind::EvolveMagic,
    ] {
        assert!(catalog.allows_ability(ability, EntityKind::Warrior));
    }
    assert!(catalog.allows_ability(AbilityKind::DiscipleShield, EntityKind::MagicAdept));
    assert!(catalog.allows_ability(AbilityKind::VectorField, EntityKind::MagicAdept));
    assert!(catalog.allows_ability(AbilityKind::WindDash, EntityKind::WindAdept));
    assert!(catalog.allows_ability(AbilityKind::ThrowSword, EntityKind::WarAdept));
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
    assert!(catalog_loadout_for(CULTIVATORS_FACTION_ID, "cultivators.standard").is_some());
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

#[test]
fn technique_library_matches_pavilion_body_and_separates_techniques_from_dao() {
    let library = defs::building_def(EntityKind::TechniqueLibrary).unwrap();
    let pavilion = defs::building_def(EntityKind::ThreeDaoPavilion).unwrap();
    let body = |s: balance::BuildingStats| {
        (
            s.hp,
            s.sight_tiles,
            s.cost_steel,
            s.cost_oil,
            s.foot_w,
            s.foot_h,
            s.build_ticks,
        )
    };
    assert_eq!(body(library.stats), body(pavilion.stats));
    assert_eq!(library.stats.dmg, 0);
    assert_eq!(library.build_requires, pavilion.build_requires);
    assert!(CULTIVATORS_CATALOG
        .buildables
        .contains(&EntityKind::TechniqueLibrary));
    assert!(!CURRENT_CATALOG
        .buildables
        .contains(&EntityKind::TechniqueLibrary));
    let researches = CULTIVATORS_CATALOG
        .researchable_upgrade_kinds(EntityKind::TechniqueLibrary)
        .collect::<Vec<_>>();
    assert_eq!(
        researches,
        vec![
            UpgradeKind::TechniqueCharge,
            UpgradeKind::TechniqueBlink,
            UpgradeKind::TechniqueShield,
            UpgradeKind::TechniqueThrowSword
        ]
    );
    assert_eq!(
        ability_definition(AbilityKind::VectorField).upgrade_requirement,
        None
    );
}
