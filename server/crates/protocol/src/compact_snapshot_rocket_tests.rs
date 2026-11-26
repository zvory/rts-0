use super::*;

#[test]
fn rocket_mortar_impact_keeps_optional_placeholders_before_style_bit() {
    let snapshot = Snapshot {
        cosmic_energy: 0,
        tick: 1,
        ground_decal_revision: 0,
        ground_decal_delta: None,
        world_combat_position: None,
        steel: 0,
        oil: 0,
        supply_used: 0,
        supply_cap: 0,
        auto_build: None,
        entities: Vec::new(),
        resource_reset: false,
        resource_deltas: Vec::new(),
        smokes: Vec::new(),
        ability_objects: Vec::new(),
        trenches: Vec::new(),
        visible_tiles: Vec::new(),
        explored_tiles: Vec::new(),
        remembered_buildings: Vec::new(),
        remembered_anti_tank_guns: Vec::new(),
        events: vec![Event::MortarImpact {
            from: None,
            x: 320.0,
            y: 352.0,
            radius_tiles: 2.0,
            reveal: None,
            rocket: true,
        }],
        upgrades: Vec::new(),
        player_resources: Vec::new(),
        player_factions: Vec::new(),
        net_status: SnapshotNetStatus::default(),
    };

    let value = compact_snapshot_value(&snapshot).unwrap();
    let event = value["ev"][0].as_array().unwrap();
    assert_eq!(event.len(), 7);
    assert!(event[4].is_null());
    assert!(event[5].is_null());
    assert_eq!(event[6], true);
}

#[test]
fn rocket_rack_count_preserves_empty_and_loaded_values_in_slot_43() {
    for count in [0, 7, 16] {
        let mut entity = EntityView::new(1, 1, "rocket_launcher", 320.0, 320.0, 150, 150, "idle");
        entity.rocket_rack_count = Some(count);
        let value = serde_json::to_value(CompactEntity(&entity)).unwrap();
        assert_eq!(value.as_array().unwrap().len(), 44);
        assert_eq!(value[43], count);
        assert_eq!(
            serde_json::to_value(&entity).unwrap()["rocketRackCount"],
            count
        );
    }
}

#[test]
fn disciple_insight_and_dao_append_without_changing_existing_slots() {
    let mut unit = EntityView::new(1, 1, "warrior", 320.0, 320.0, 155, 155, "idle");
    unit.insight = Some(60);
    unit.dao = Some("war".into());
    let value = serde_json::to_value(CompactEntity(&unit)).unwrap();
    assert_eq!(value.as_array().unwrap().len(), 47);
    assert_eq!(value[45], 60);
    assert_eq!(value[46], "war");
    assert!(value[44].is_null());
    unit.insight = None;
    unit.dao_ready = Some(false);
    unit.dao_advance_ticks = Some(15);
    let public = serde_json::to_value(CompactEntity(&unit)).unwrap();
    assert_eq!(public.as_array().unwrap().len(), 49);
    assert!(public[45].is_null());
    assert_eq!(public[46], "war");
    assert_eq!(public[47], false);
    assert_eq!(public[48], 15);
}

#[test]
fn chosen_dao_appends_private_intent_with_null_placeholders() {
    let mut unit = EntityView::new(1, 1, "warrior", 320.0, 320.0, 135, 135, "idle");
    unit.chosen_dao = Some("magic".into());
    let value = serde_json::to_value(CompactEntity(&unit)).unwrap();
    assert_eq!(value.as_array().unwrap().len(), 50);
    assert!(value[48].is_null());
    assert_eq!(value[49], "magic");
    assert_eq!(serde_json::to_value(&unit).unwrap()["chosenDao"], "magic");
}

#[test]
fn dao_training_appends_private_plans_without_shifting_existing_slots() {
    let mut building = EntityView::new(1, 1, "proven_grounds", 320.0, 320.0, 120, 120, "idle");
    building.dao_training_queue = Some(vec!["dao_of_magic".into(), "dao_of_heaviness".into()]);
    let value = serde_json::to_value(CompactEntity(&building)).unwrap();
    assert_eq!(value.as_array().unwrap().len(), 52);
    assert_eq!(value[2], 33);
    assert!(value[50].is_null());
    assert_eq!(
        value[51],
        serde_json::json!(["dao_of_magic", "dao_of_heaviness"])
    );
}

#[test]
fn dao_master_rank_is_public_without_private_insight() {
    let mut unit = EntityView::new(1, 1, "warrior", 320.0, 320.0, 161, 161, "idle");
    unit.dao = Some("wind".into());
    unit.dao_master = Some(true);
    let value = serde_json::to_value(CompactEntity(&unit)).unwrap();
    assert_eq!(value.as_array().unwrap().len(), 53);
    assert!(value[45].is_null());
    assert_eq!(value[46], "wind");
    assert_eq!(value[52], true);
    assert_eq!(serde_json::to_value(&unit).unwrap()["daoMaster"], true);
}

#[test]
fn escape_treasure_flag_is_omitted_when_absent() {
    let mut unit = EntityView::new(1, 1, "warrior", 320.0, 320.0, 75, 150, "idle");
    assert!(serde_json::to_value(&unit)
        .unwrap()
        .get("escapeTreasure")
        .is_none());
    assert_eq!(
        serde_json::to_value(CompactEntity(&unit))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        8
    );
    unit.escape_treasure = true;
    let compact = serde_json::to_value(CompactEntity(&unit)).unwrap();
    assert_eq!(compact.as_array().unwrap().len(), 54);
    assert_eq!(compact[53], true);
    assert!(compact[52].is_null());
    assert_eq!(serde_json::to_value(&unit).unwrap()["escapeTreasure"], true);
}

#[test]
fn item_radiance_tier_appends_public_strength_including_explicit_zero() {
    let mut unit = EntityView::new(1, 1, "magic_adept", 320.0, 320.0, 75, 150, "idle");
    for tier in 0..=6 {
        unit.item_radiance_tier = Some(tier);
        let compact = serde_json::to_value(CompactEntity(&unit)).unwrap();
        assert_eq!(compact.as_array().unwrap().len(), 55);
        assert_eq!(compact[53], false);
        assert_eq!(compact[54], tier);
        assert_eq!(
            serde_json::to_value(&unit).unwrap()["itemRadianceTier"],
            tier
        );
        assert!(unit.abilities.is_empty());
    }
}

#[test]
fn charge_active_appends_public_flag_and_omits_inactive_state() {
    let mut unit = EntityView::new(1, 1, "war_adept", 320.0, 320.0, 176, 176, "attack");
    assert!(serde_json::to_value(&unit)
        .unwrap()
        .get("chargeActive")
        .is_none());
    unit.charge_active = true;
    let compact = serde_json::to_value(CompactEntity(&unit)).unwrap();
    assert_eq!(compact.as_array().unwrap().len(), 56);
    assert_eq!(compact[55], true);
    assert!(compact[54].is_null());
    assert_eq!(serde_json::to_value(&unit).unwrap()["chargeActive"], true);
}
