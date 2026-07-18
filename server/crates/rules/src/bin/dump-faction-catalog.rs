use rts_rules::{balance, defs, economy, faction, EntityKind};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let catalogs: Vec<_> = if args.iter().any(|arg| arg == "--all") {
        faction::CATALOGS.to_vec()
    } else if let Some(index) = args.iter().position(|arg| arg == "--faction") {
        let Some(id) = args.get(index + 1) else {
            eprintln!("--faction requires an id");
            std::process::exit(2);
        };
        let Some(catalog) = faction::catalog_for(id) else {
            eprintln!("unknown faction catalog: {id}");
            std::process::exit(1);
        };
        vec![catalog]
    } else {
        vec![faction::CURRENT_CATALOG]
    };

    if catalogs.len() == 1 && !args.iter().any(|arg| arg == "--all") {
        print_catalog(catalogs[0], "");
        println!();
    } else {
        println!("{{");
        println!("  \"catalogs\": [");
        for (index, catalog) in catalogs.iter().enumerate() {
            print_catalog(*catalog, "    ");
            println!("{}", if index + 1 == catalogs.len() { "" } else { "," });
        }
        println!("  ]");
        println!("}}");
    }
}

fn print_catalog(catalog: faction::FactionCatalog, indent: &str) {
    println!("{indent}{{");
    println!("{indent}  \"id\": \"{}\",", catalog.id);
    println!("{indent}  \"loadoutId\": \"{}\",", catalog.loadout.id);
    print_kind_array("units", catalog.units, true, indent);
    print_kind_array("buildings", catalog.buildings, true, indent);
    println!("{indent}  \"buildables\": [");
    for (index, kind) in catalog.buildables.iter().enumerate() {
        let comma = if index + 1 == catalog.buildables.len() {
            ""
        } else {
            ","
        };
        print!(
            "{indent}    {{\"kind\":\"{}\",\"requires\":",
            kind.stable_id()
        );
        let requires = defs::building_def(*kind)
            .map(|def| def.build_requires)
            .unwrap_or(&[]);
        print_kind_array_inline(requires);
        println!("}}{comma}");
    }
    println!("{indent}  ],");
    println!("{indent}  \"trainables\": [");
    let producers: Vec<_> = catalog
        .buildings
        .iter()
        .copied()
        .filter(|kind| !catalog.trainable_units(*kind).is_empty())
        .collect();
    for (index, building) in producers.iter().enumerate() {
        let comma = if index + 1 == producers.len() {
            ""
        } else {
            ","
        };
        print!(
            "{indent}    {{\"building\":\"{}\",\"units\":",
            building.stable_id()
        );
        print_kind_vec_inline(&catalog.trainable_units(*building));
        println!("}}{comma}");
    }
    println!("{indent}  ],");
    println!("{indent}  \"research\": [");
    for (index, upgrade) in catalog.upgrades.iter().enumerate() {
        let comma = if index + 1 == catalog.upgrades.len() {
            ""
        } else {
            ","
        };
        println!(
            "{indent}    {{\"id\":\"{}\",\"researchedAt\":\"{}\"}}{comma}",
            upgrade.kind.stable_id(),
            upgrade.researched_at.stable_id()
        );
    }
    println!("{indent}  ],");
    println!("{indent}  \"abilities\": [");
    for (index, ability) in catalog.abilities.iter().enumerate() {
        let comma = if index + 1 == catalog.abilities.len() {
            ""
        } else {
            ","
        };
        print!(
            "{indent}    {{\"id\":\"{}\",\"label\":\"{}\",\"icon\":\"{}\",\"hotkey\":{},\"title\":\"{}\",\"carriers\":",
            ability.kind.stable_id(),
            ability.label,
            ability.icon,
            json_string_or_null(ability.hotkey),
            ability.title,
        );
        print_kind_array_inline(ability.carriers);
        print!(
            ",\"targetMode\":\"{}\",\"rangeTiles\":{},\"minRangeTiles\":{},\"cooldownTicks\":{},\"charges\":{},\"cost\":{{\"steel\":{},\"oil\":{}}},\"techRequirement\":{},\"mayQueue\":{},\"queuePolicy\":\"{}\",\"autocast\":{},\"commandCard\":{},\"protocolCode\":{},\"orderStageCode\":{}",
            ability.target_mode.stable_id(),
            json_u32_or_null(ability.range_tiles),
            json_u32_or_null(ability.min_range_tiles),
            ability.cooldown_ticks,
            json_u16_or_null(ability.charges),
            ability.cost.steel,
            ability.cost.oil,
            json_kind_or_null(ability.tech_requirement),
            ability.may_queue(),
            ability.queue_policy.stable_id(),
            ability.autocast,
            ability.command_card,
            ability.protocol_code,
            ability.order_stage_code,
        );
        println!("}}{comma}");
    }
    println!("{indent}  ],");
    print_kind_array("builders", catalog.builders, true, indent);
    print_kind_array("gatherers", catalog.gatherers, true, indent);
    print_kind_array(
        "productionAnchors",
        catalog.production_anchors,
        true,
        indent,
    );
    println!("{indent}  \"costs\": {{");
    let priced: Vec<_> = catalog
        .units
        .iter()
        .chain(catalog.buildings.iter())
        .copied()
        .collect();
    for (index, kind) in priced.iter().enumerate() {
        let comma = if index + 1 == priced.len() { "" } else { "," };
        let (steel, oil) = economy::cost(*kind);
        println!(
            "{indent}    \"{}\": {{\"steel\":{},\"oil\":{}}}{comma}",
            kind.stable_id(),
            steel,
            oil
        );
    }
    println!("{indent}  }},");
    let child_indent = format!("{indent}  ");
    print_client_config(&child_indent);
    println!();
    print!("{indent}}}");
}

fn print_client_config(indent: &str) {
    println!("{indent}\"clientConfig\": {{");
    print_client_constants(indent);
    println!("{indent}  \"unitStats\": {{");
    for (index, def) in defs::UNITS.iter().enumerate() {
        let comma = if index + 1 == defs::UNITS.len() {
            ""
        } else {
            ","
        };
        println!(
            "{indent}    \"{}\": {{\"size\":{},\"sight\":{},\"rangeTiles\":{},\"cost\":{{\"steel\":{},\"oil\":{}}},\"supply\":{},\"buildTicks\":{}}}{comma}",
            def.kind.stable_id(),
            json_f32(def.stats.radius),
            def.stats.sight_tiles,
            client_visible_range_tiles(def.kind, def.stats.range_tiles),
            def.stats.cost_steel,
            def.stats.cost_oil,
            def.stats.supply,
            def.stats.build_ticks,
        );
    }
    println!("{indent}  }},");
    println!("{indent}  \"buildingStats\": {{");
    for (index, def) in defs::BUILDINGS.iter().enumerate() {
        let comma = if index + 1 == defs::BUILDINGS.len() {
            ""
        } else {
            ","
        };
        println!(
            "{indent}    \"{}\": {{\"footW\":{},\"footH\":{},\"sight\":{},\"cost\":{{\"steel\":{},\"oil\":{}}},\"buildTicks\":{}}}{comma}",
            def.kind.stable_id(),
            def.stats.foot_w,
            def.stats.foot_h,
            def.stats.sight_tiles,
            def.stats.cost_steel,
            def.stats.cost_oil,
            def.stats.build_ticks,
        );
    }
    println!("{indent}  }},");
    println!("{indent}  \"resourceAmounts\": {{");
    for (index, def) in defs::NODES.iter().enumerate() {
        let comma = if index + 1 == defs::NODES.len() {
            ""
        } else {
            ","
        };
        println!(
            "{indent}    \"{}\": {}{comma}",
            def.kind.stable_id(),
            def.amount
        );
    }
    println!("{indent}  }},");
    println!("{indent}  \"bodies\": {{");
    println!(
        "{indent}    \"{}\": {{\"length\":{},\"width\":{},\"clearance\":{}}},",
        EntityKind::Tank.stable_id(),
        json_f32(balance::TANK_BODY_LENGTH_PX),
        json_f32(balance::TANK_BODY_WIDTH_PX),
        json_f32(balance::TANK_BODY_CLEARANCE_PX),
    );
    println!(
        "{indent}    \"{}\": {{\"length\":{},\"width\":{},\"clearance\":{}}},",
        EntityKind::AntiTankGun.stable_id(),
        json_f32(balance::ANTI_TANK_GUN_BODY_LENGTH_PX),
        json_f32(balance::ANTI_TANK_GUN_BODY_WIDTH_PX),
        json_f32(balance::ANTI_TANK_GUN_BODY_CLEARANCE_PX),
    );
    println!(
        "{indent}    \"{}\": {{\"length\":{},\"width\":{},\"clearance\":{}}},",
        EntityKind::Artillery.stable_id(),
        json_f32(balance::ARTILLERY_BODY_LENGTH_PX),
        json_f32(balance::ARTILLERY_BODY_WIDTH_PX),
        json_f32(balance::ARTILLERY_BODY_CLEARANCE_PX),
    );
    println!(
        "{indent}    \"{}\": {{\"length\":{},\"width\":{},\"clearance\":{}}},",
        EntityKind::ScoutCar.stable_id(),
        json_f32(balance::SCOUT_CAR_BODY_LENGTH_PX),
        json_f32(balance::SCOUT_CAR_BODY_WIDTH_PX),
        json_f32(balance::SCOUT_CAR_BODY_CLEARANCE_PX),
    );
    println!(
        "{indent}    \"{}\": {{\"length\":{},\"width\":{},\"clearance\":{}}},",
        EntityKind::ScoutPlane.stable_id(),
        json_f32(balance::SCOUT_PLANE_BODY_LENGTH_PX),
        json_f32(balance::SCOUT_PLANE_BODY_WIDTH_PX),
        json_f32(balance::SCOUT_PLANE_BODY_CLEARANCE_PX),
    );
    println!(
        "{indent}    \"{}\": {{\"length\":{},\"width\":{},\"clearance\":{}}}",
        EntityKind::CommandCar.stable_id(),
        json_f32(balance::COMMAND_CAR_BODY_LENGTH_PX),
        json_f32(balance::COMMAND_CAR_BODY_WIDTH_PX),
        json_f32(balance::COMMAND_CAR_BODY_CLEARANCE_PX),
    );
    println!("{indent}  }},");
    print_upgrades(indent);
    println!("{indent}  \"abilityEffects\": {{");
    print_ability_effects(indent);
    println!("{indent}  }}");
    print!("{indent}}}");
}

fn print_client_constants(indent: &str) {
    println!("{indent}  \"constants\": {{");
    println!("{indent}    \"tickHz\": {},", balance::TICK_HZ);
    println!(
        "{indent}    \"miningCcRangeTiles\": {},",
        json_f32(balance::MINING_CC_RANGE_TILES)
    );
    println!(
        "{indent}    \"antiTankGunDeployedRangeTiles\": {},",
        balance::ANTI_TANK_GUN_DEPLOYED_RANGE_TILES
    );
    println!(
        "{indent}    \"antiTankGunFieldOfFireRad\": {},",
        json_f32(balance::ANTI_TANK_GUN_FIELD_OF_FIRE_RAD)
    );
    println!(
        "{indent}    \"mortarMinRangeTiles\": {},",
        balance::MORTAR_MIN_RANGE_TILES
    );
    println!(
        "{indent}    \"mortarFieldOfFireRad\": {},",
        json_f32(balance::MORTAR_FIELD_OF_FIRE_RAD)
    );
    println!(
        "{indent}    \"mortarSetupTicks\": {},",
        balance::MORTAR_TEAM_SETUP_TICKS
    );
    println!(
        "{indent}    \"mortarTeardownTicks\": {},",
        balance::MORTAR_TEAM_TEARDOWN_TICKS
    );
    println!(
        "{indent}    \"artilleryMinRangeTiles\": {},",
        balance::ARTILLERY_MIN_RANGE_TILES
    );
    println!(
        "{indent}    \"artilleryMaxRangeTiles\": {},",
        balance::ARTILLERY_MAX_RANGE_TILES
    );
    println!(
        "{indent}    \"artilleryFieldOfFireRad\": {},",
        json_f32(balance::ARTILLERY_FIELD_OF_FIRE_RAD)
    );
    println!(
        "{indent}    \"artillerySetupTicks\": {},",
        balance::ARTILLERY_SETUP_TICKS
    );
    println!(
        "{indent}    \"artilleryShellDelayTicks\": {},",
        balance::ARTILLERY_SHELL_DELAY_TICKS
    );
    println!(
        "{indent}    \"artilleryOuterRadiusTiles\": {},",
        json_f32(balance::ARTILLERY_OUTER_RADIUS_TILES)
    );
    println!(
        "{indent}    \"artilleryBlanketRadiusTiles\": {},",
        json_f32(balance::ARTILLERY_BLANKET_RADIUS_TILES)
    );
    println!(
        "{indent}    \"artilleryAmmoCost\": {{\"steel\":{},\"oil\":0}},",
        balance::ARTILLERY_AMMO_COST_STEEL
    );
    println!(
        "{indent}    \"smokeAbilityRangeTiles\": {},",
        balance::SMOKE_ABILITY_RANGE_TILES
    );
    println!(
        "{indent}    \"smokeLaunchMaxDelayMs\": {},",
        ticks_to_ceil_ms(balance::SMOKE_LAUNCH_MAX_DELAY_TICKS)
    );
    println!(
        "{indent}    \"smokeCloudRadiusTiles\": {},",
        json_f32(balance::SMOKE_CLOUD_RADIUS_TILES)
    );
    println!(
        "{indent}    \"smokeCloudDurationTicks\": {},",
        balance::SMOKE_CLOUD_DURATION_TICKS
    );
    println!(
        "{indent}    \"smokeAbilityCooldownTicks\": {},",
        balance::SMOKE_ABILITY_COOLDOWN_TICKS
    );
    println!(
        "{indent}    \"scoutPlaneOrbitRadiusTiles\": {},",
        balance::SCOUT_PLANE_ORBIT_RADIUS_TILES
    );
    println!(
        "{indent}    \"scoutPlaneSpeedPxPerTick\": {},",
        json_f32(balance::SCOUT_PLANE_SPEED_PX_PER_TICK)
    );
    println!(
        "{indent}    \"scoutPlaneLifetimeTicks\": {},",
        balance::SCOUT_PLANE_LIFETIME_TICKS
    );
    println!(
        "{indent}    \"scoutPlaneAbilityCooldownTicks\": {},",
        balance::SCOUT_PLANE_ABILITY_COOLDOWN_TICKS
    );
    println!(
        "{indent}    \"smokeAbilityCost\": {{\"steel\":{},\"oil\":{}}},",
        balance::SMOKE_ABILITY_COST_STEEL,
        balance::SMOKE_ABILITY_COST_OIL
    );
    println!(
        "{indent}    \"mortarShellDelayTicks\": {},",
        balance::MORTAR_SHELL_DELAY_TICKS
    );
    println!(
        "{indent}    \"mortarOuterRadiusTiles\": {},",
        json_f32(balance::MORTAR_OUTER_RADIUS_TILES)
    );
    println!(
        "{indent}    \"mortarInnerRadiusTiles\": {},",
        json_f32(balance::MORTAR_INNER_RADIUS_TILES)
    );
    println!(
        "{indent}    \"mortarFireCooldownTicks\": {},",
        balance::TICK_HZ * 2
    );
    println!(
        "{indent}    \"panzerfaustRangeTiles\": {},",
        balance::PANZERFAUST_RANGE_TILES
    );
    println!(
        "{indent}    \"panzerfaustDamage\": {},",
        balance::PANZERFAUST_DAMAGE
    );
    println!(
        "{indent}    \"panzerfaustArmorPenetration\": {},",
        json_f32(balance::PANZERFAUST_ARMOR_PENETRATION)
    );
    println!(
        "{indent}    \"panzerfaustWindupTicks\": {},",
        balance::PANZERFAUST_WINDUP_TICKS
    );
    println!(
        "{indent}    \"panzerfaustTravelTicks\": {},",
        balance::PANZERFAUST_TRAVEL_TICKS
    );
    println!(
        "{indent}    \"methamphetaminesPanzerfaustWindupTicks\": {},",
        balance::METHAMPHETAMINES_PANZERFAUST_WINDUP_TICKS
    );
    println!(
        "{indent}    \"entrenchmentDigInTicks\": {},",
        balance::ENTRENCHMENT_DIG_IN_TICKS
    );
    println!(
        "{indent}    \"entrenchmentRangeBonusTiles\": {},",
        balance::ENTRENCHMENT_RANGE_BONUS_TILES
    );
    println!(
        "{indent}    \"entrenchmentDirectDamageReduction\": {},",
        json_f32(balance::ENTRENCHMENT_DIRECT_DAMAGE_REDUCTION)
    );
    println!(
        "{indent}    \"entrenchmentAreaDamageReduction\": {},",
        json_f32(balance::ENTRENCHMENT_AREA_DAMAGE_REDUCTION)
    );
    println!(
        "{indent}    \"entrenchmentTrenchRadiusTiles\": {},",
        json_f32(balance::ENTRENCHMENT_TRENCH_RADIUS_TILES)
    );
    println!(
        "{indent}    \"ekatConsumeGolemRangeTiles\": {},",
        balance::EKAT_CONSUME_GOLEM_RANGE_TILES
    );
    println!(
        "{indent}    \"ekatTeleportRangeTiles\": {},",
        balance::EKAT_TELEPORT_RANGE_TILES
    );
    println!(
        "{indent}    \"ekatTeleportCooldownTicks\": {},",
        balance::EKAT_TELEPORT_COOLDOWN_TICKS
    );
    println!(
        "{indent}    \"ekatLineShotRangeTiles\": {},",
        balance::EKAT_LINE_SHOT_RANGE_TILES
    );
    println!(
        "{indent}    \"ekatLineShotWidthTiles\": {},",
        json_f32(balance::EKAT_LINE_SHOT_WIDTH_TILES)
    );
    println!(
        "{indent}    \"ekatLineShotSpeedPxPerTick\": {},",
        json_f32(balance::EKAT_LINE_SHOT_SPEED_PX_PER_TICK)
    );
    println!(
        "{indent}    \"ekatLineShotDamage\": {},",
        balance::EKAT_LINE_SHOT_DAMAGE
    );
    println!(
        "{indent}    \"ekatLineShotCooldownTicks\": {},",
        balance::EKAT_LINE_SHOT_COOLDOWN_TICKS
    );
    println!(
        "{indent}    \"ekatMagicAnchorRangeTiles\": {},",
        balance::EKAT_MAGIC_ANCHOR_RANGE_TILES
    );
    println!(
        "{indent}    \"ekatMagicAnchorDurationTicks\": {},",
        balance::EKAT_MAGIC_ANCHOR_DURATION_TICKS
    );
    println!(
        "{indent}    \"ekatMagicAnchorRadiusTiles\": {},",
        json_f32(balance::EKAT_MAGIC_ANCHOR_RADIUS_TILES)
    );
    println!(
        "{indent}    \"ekatMagicAnchorPullAwayMultiplier\": {},",
        json_f32(balance::EKAT_MAGIC_ANCHOR_PULL_AWAY_MULTIPLIER)
    );
    println!(
        "{indent}    \"ekatMagicAnchorPullTowardMultiplier\": {},",
        json_f32(balance::EKAT_MAGIC_ANCHOR_PULL_TOWARD_MULTIPLIER)
    );
    println!(
        "{indent}    \"breakthroughRadiusTiles\": {},",
        json_f32(balance::BREAKTHROUGH_RADIUS_TILES)
    );
    println!(
        "{indent}    \"breakthroughDurationTicks\": {},",
        balance::BREAKTHROUGH_DURATION_TICKS
    );
    println!(
        "{indent}    \"breakthroughCooldownTicks\": {}",
        balance::BREAKTHROUGH_COOLDOWN_TICKS
    );
    println!("{indent}  }},");
}

fn print_upgrades(indent: &str) {
    println!("{indent}  \"upgrades\": {{");
    print_upgrade(
        indent,
        faction::METHAMPHETAMINES_UPGRADE,
        balance::METHAMPHETAMINES_COST_STEEL,
        balance::METHAMPHETAMINES_COST_OIL,
        balance::METHAMPHETAMINES_RESEARCH_TICKS,
        None,
        true,
    );
    print_upgrade(
        indent,
        faction::ENTRENCHMENT_UPGRADE,
        balance::ENTRENCHMENT_COST_STEEL,
        balance::ENTRENCHMENT_COST_OIL,
        balance::ENTRENCHMENT_RESEARCH_TICKS,
        None,
        true,
    );
    print_upgrade(
        indent,
        faction::ANTI_TANK_GUN_UNLOCK_UPGRADE,
        balance::ANTI_TANK_GUN_UNLOCK_COST_STEEL,
        balance::ANTI_TANK_GUN_UNLOCK_COST_OIL,
        balance::ANTI_TANK_GUN_UNLOCK_RESEARCH_TICKS,
        None,
        true,
    );
    print_upgrade(
        indent,
        faction::BALLISTIC_TABLES_UPGRADE,
        balance::BALLISTIC_TABLES_COST_STEEL,
        balance::BALLISTIC_TABLES_COST_OIL,
        balance::BALLISTIC_TABLES_RESEARCH_TICKS,
        Some(faction::ARTILLERY_UNLOCK_UPGRADE),
        true,
    );
    print_upgrade(
        indent,
        faction::ARTILLERY_UNLOCK_UPGRADE,
        balance::ARTILLERY_UNLOCK_COST_STEEL,
        balance::ARTILLERY_UNLOCK_COST_OIL,
        balance::ARTILLERY_UNLOCK_RESEARCH_TICKS,
        Some(faction::ANTI_TANK_GUN_UNLOCK_UPGRADE),
        true,
    );
    print_upgrade(
        indent,
        faction::TANK_UNLOCK_UPGRADE,
        balance::TANK_UNLOCK_COST_STEEL,
        balance::TANK_UNLOCK_COST_OIL,
        balance::TANK_UNLOCK_RESEARCH_TICKS,
        None,
        true,
    );
    print_upgrade(
        indent,
        faction::MORTAR_AUTOCAST_UPGRADE,
        balance::MORTAR_AUTOCAST_COST_STEEL,
        balance::MORTAR_AUTOCAST_COST_OIL,
        balance::MORTAR_AUTOCAST_RESEARCH_TICKS,
        None,
        true,
    );
    print_upgrade(
        indent,
        faction::PANZERFAUSTS_UPGRADE,
        balance::PANZERFAUSTS_COST_STEEL,
        balance::PANZERFAUSTS_COST_OIL,
        balance::PANZERFAUSTS_RESEARCH_TICKS,
        None,
        true,
    );
    print_upgrade(
        indent,
        faction::SMOKE_PLUS_UPGRADE,
        balance::SMOKE_PLUS_COST_STEEL,
        balance::SMOKE_PLUS_COST_OIL,
        balance::SMOKE_PLUS_RESEARCH_TICKS,
        None,
        false,
    );
    println!("{indent}  }},");
}

fn print_upgrade(
    indent: &str,
    id: &str,
    steel: u32,
    oil: u32,
    research_ticks: u32,
    requires_upgrade: Option<&str>,
    comma: bool,
) {
    println!(
        "{indent}    \"{id}\": {{\"cost\":{{\"steel\":{steel},\"oil\":{oil}}},\"researchTicks\":{research_ticks},\"requiresUpgrade\":{}}}{}",
        json_string_or_null(requires_upgrade),
        if comma { "," } else { "" },
    );
}

fn print_ability_effects(indent: &str) {
    println!(
        "{indent}    \"{}\": {{\"radiusTiles\":{},\"durationTicks\":{},\"upgradedRadiusTiles\":{},\"upgradedDurationTicks\":{}}},",
        faction::SMOKE_ABILITY,
        json_f32(balance::SMOKE_CLOUD_RADIUS_TILES),
        balance::SMOKE_CLOUD_DURATION_TICKS,
        json_f32(balance::SMOKE_PLUS_CLOUD_RADIUS_TILES),
        balance::SMOKE_PLUS_CLOUD_DURATION_TICKS,
    );
    println!(
        "{indent}    \"{}\": {{\"radiusTiles\":{}}},",
        faction::MORTAR_FIRE_ABILITY,
        json_f32(balance::MORTAR_OUTER_RADIUS_TILES),
    );
    println!(
        "{indent}    \"{}\": {{\"radiusTiles\":{},\"delayTicks\":{}}},",
        faction::POINT_FIRE_ABILITY,
        json_f32(balance::ARTILLERY_OUTER_RADIUS_TILES),
        balance::ARTILLERY_SHELL_DELAY_TICKS,
    );
    println!(
        "{indent}    \"{}\": {{\"radiusTiles\":{}}},",
        faction::BLANKET_FIRE_ABILITY,
        json_f32(balance::ARTILLERY_BLANKET_RADIUS_TILES),
    );
    println!(
        "{indent}    \"{}\": {{\"radiusTiles\":{},\"durationTicks\":{}}},",
        faction::BREAKTHROUGH_ABILITY,
        json_f32(balance::BREAKTHROUGH_RADIUS_TILES),
        balance::BREAKTHROUGH_DURATION_TICKS,
    );
    println!("{indent}    \"{}\": {{}},", faction::EKAT_TELEPORT_ABILITY,);
    println!(
        "{indent}    \"{}\": {{\"radiusTiles\":{},\"speedPxPerTick\":{},\"damage\":{}}},",
        faction::EKAT_LINE_SHOT_ABILITY,
        json_f32(balance::EKAT_LINE_SHOT_WIDTH_TILES * 0.5),
        json_f32(balance::EKAT_LINE_SHOT_SPEED_PX_PER_TICK),
        balance::EKAT_LINE_SHOT_DAMAGE,
    );
    println!(
        "{indent}    \"{}\": {{\"radiusTiles\":{},\"durationTicks\":{},\"pullAwayMultiplier\":{},\"pullTowardMultiplier\":{}}},",
        faction::EKAT_MAGIC_ANCHOR_ABILITY,
        json_f32(balance::EKAT_MAGIC_ANCHOR_RADIUS_TILES),
        balance::EKAT_MAGIC_ANCHOR_DURATION_TICKS,
        json_f32(balance::EKAT_MAGIC_ANCHOR_PULL_AWAY_MULTIPLIER),
        json_f32(balance::EKAT_MAGIC_ANCHOR_PULL_TOWARD_MULTIPLIER),
    );
    println!(
        "{indent}    \"{}\": {{\"radiusTiles\":{}}}",
        faction::EKAT_CONSUME_GOLEM_ABILITY,
        json_f32(balance::EKAT_CONSUME_GOLEM_RANGE_TILES as f32),
    );
}

fn print_kind_array(name: &str, kinds: &[EntityKind], comma: bool, indent: &str) {
    print!("{indent}  \"{name}\": ");
    print_kind_array_inline(kinds);
    println!("{}", if comma { "," } else { "" });
}

fn print_kind_array_inline(kinds: &[EntityKind]) {
    print!("[");
    for (index, kind) in kinds.iter().enumerate() {
        if index > 0 {
            print!(",");
        }
        print!("\"{}\"", kind.stable_id());
    }
    print!("]");
}

fn print_kind_vec_inline(kinds: &[EntityKind]) {
    print_kind_array_inline(kinds);
}

fn json_string_or_null(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{value}\""))
        .unwrap_or_else(|| "null".to_string())
}

fn json_u32_or_null(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn json_u16_or_null(value: Option<u16>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn json_f32(value: f32) -> String {
    if value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

fn ticks_to_ceil_ms(ticks: u32) -> u32 {
    (ticks * 1000_u32).div_ceil(balance::TICK_HZ)
}

fn client_visible_range_tiles(kind: EntityKind, default_range_tiles: u32) -> u32 {
    match kind {
        EntityKind::AntiTankGun => balance::ANTI_TANK_GUN_DEPLOYED_RANGE_TILES,
        _ => default_range_tiles,
    }
}

fn json_kind_or_null(value: Option<EntityKind>) -> String {
    value
        .map(|kind| format!("\"{}\"", kind.stable_id()))
        .unwrap_or_else(|| "null".to_string())
}
