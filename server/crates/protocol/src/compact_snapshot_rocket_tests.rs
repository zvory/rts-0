use super::*;

#[test]
fn rocket_mortar_impact_keeps_optional_placeholders_before_style_bit() {
    let snapshot = Snapshot {
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
