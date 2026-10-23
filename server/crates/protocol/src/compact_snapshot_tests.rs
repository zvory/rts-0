use super::*;
use crate::{kinds, states};

#[test]
fn compact_entity_trims_trailing_optional_nulls() {
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
        entities: vec![EntityView::new(
            1,
            1,
            kinds::WORKER,
            10.0,
            20.0,
            40,
            40,
            states::IDLE,
        )],
        resource_deltas: Vec::new(),
        smokes: Vec::new(),
        ability_objects: Vec::new(),
        trenches: Vec::new(),
        visible_tiles: Vec::new(),
        explored_tiles: Vec::new(),
        remembered_buildings: Vec::new(),
        remembered_anti_tank_guns: Vec::new(),
        events: Vec::new(),
        upgrades: Vec::new(),
        player_resources: Vec::new(),
        net_status: SnapshotNetStatus::default(),
    };

    let compact = serialize_compact_snapshot(&snapshot).unwrap();
    let value: serde_json::Value = serde_json::from_str(&compact).unwrap();
    let entity = value["e"][0].as_array().unwrap();
    assert_eq!(entity.len(), 8);
    assert!(value.get("r").is_none());
    assert!(value.get("ev").is_none());
}

#[test]
fn compact_json_snapshot_codec_remains_available_for_local_baselines() {
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
        events: Vec::new(),
        upgrades: Vec::new(),
        player_resources: Vec::new(),
        net_status: SnapshotNetStatus::default(),
    };
    let frame = crate::encode_snapshot_frame(&snapshot, crate::SnapshotCodec::CompactJson)
        .expect("compact JSON should encode");
    match frame {
        crate::SnapshotFrame::Text(text) => {
            let value: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(value["t"], "snapshot");
            assert_eq!(value["v"], COMPACT_SNAPSHOT_VERSION);
        }
        crate::SnapshotFrame::Binary(_) => panic!("compact JSON baseline codec must stay text"),
    }
}

#[test]
fn compact_snapshot_carries_bounded_ground_decal_delta() {
    let mut snapshot = Snapshot {
        tick: 1,
        ground_decal_revision: 31,
        ground_decal_delta: Some(rts_contract::GroundDecalDelta {
            after_revision: 24,
            decals: vec![GroundDecalView {
                id: 7,
                decal_class: "infantry".to_string(),
                source_kind: "rifleman".to_string(),
                x: 32.0,
                y: 64.0,
                owner: 1,
                seed: 9,
                facing: Some(0.5),
                weapon_facing: None,
                radius_tiles: None,
            }],
            tank_trails: vec![TankTrailView {
                id: 8,
                poses: vec![[128, 256, 0], [160, 256, 1024]],
            }],
        }),
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
        events: Vec::new(),
        upgrades: Vec::new(),
        player_resources: Vec::new(),
        net_status: SnapshotNetStatus::default(),
    };
    let value = compact_snapshot_value(&snapshot).unwrap();
    assert_eq!(value["gr"], 31);
    assert_eq!(value["gd"][0], 24);
    assert_eq!(value["gd"][1][0][0], 7);
    assert_eq!(value["gd"][1][0][1], "infantry");
    assert_eq!(value["gd"][1][0][2], kind_code("rifleman"));
    assert_eq!(value["gd"][2][0][0], 8);
    assert_eq!(value["gd"][2][0][1][1][0], 160);
    snapshot.ground_decal_revision = 0;
    snapshot.ground_decal_delta = None;
    let value = compact_snapshot_value(&snapshot).unwrap();
    assert!(value.get("gr").is_none());
    assert!(value.get("gd").is_none());
}
