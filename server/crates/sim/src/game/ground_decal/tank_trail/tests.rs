use super::*;
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};

use super::checkpoint::{decode_poses, encode_poses};

#[test]
fn pose_is_six_bytes_in_memory_and_five_bytes_in_checkpoints() {
    let pose = TankTrailPose((400, 404, -12));
    assert_eq!(std::mem::size_of::<TankTrailPose>(), 6);
    assert_eq!(serde_json::to_string(&pose).unwrap(), "[400,404,-12]");
    assert_eq!(
        STANDARD_NO_PAD.decode(encode_poses(&[pose])).unwrap().len(),
        5
    );
}

#[test]
fn sampling_is_sparse_but_preserves_pivots() {
    let origin = TankTrailPose((100, 100, 0));
    let short_travel = TankTrailPose((115, 100, 0));
    let full_travel = TankTrailPose((116, 100, 0));
    let small_turn = TankTrailPose((100, 100, (0.20 * HEADING_SCALE) as i8));
    let sampled_turn = TankTrailPose((100, 100, (0.30 * HEADING_SCALE) as i8));

    assert!(!sample_needed(origin, short_travel));
    assert!(sample_needed(origin, full_travel));
    assert!(!sample_needed(origin, small_turn));
    assert!(sample_needed(origin, sampled_turn));
}

#[test]
fn packed_heading_preserves_an_in_place_pivot() {
    let a = TankTrailPose((100, 100, 0));
    let b = TankTrailPose((
        100,
        100,
        (std::f32::consts::FRAC_PI_2 * HEADING_SCALE).round() as i8,
    ));
    assert!(contact_motion(a, b) > 40.0);
    assert_eq!(a.wire()[..2], b.wire()[..2]);
    assert_ne!(a.wire()[2], b.wire()[2]);
}

#[test]
fn settling_keeps_short_moves_and_the_unsampled_tail() {
    let origin = TankTrailPose((100, 100, 0));
    let sampled = TankTrailPose((116, 100, 0));
    let tail = TankTrailPose((121, 100, 0));
    let chunks = TankTrailStore::finish(ActiveTankTrail {
        owner: 1,
        poses: vec![origin, sampled],
        last_observed: tail,
        last_motion_tick: 1,
    });
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].poses, vec![origin, sampled, tail]);

    let short = TankTrailStore::finish(ActiveTankTrail {
        owner: 1,
        poses: vec![origin],
        last_observed: TankTrailPose((105, 100, 0)),
        last_motion_tick: 1,
    });
    assert_eq!(short.len(), 1);
    assert_eq!(short[0].poses.len(), 2);
}

#[test]
fn checkpoint_rejects_reserved_heading_byte() {
    let encoded = STANDARD_NO_PAD.encode([0, 0, 0, 0, i8::MIN as u8]);
    assert!(decode_poses::<serde::de::value::Error>(&encoded).is_err());
}

#[test]
fn finalized_chunk_bounds_include_the_oriented_track_footprint() {
    let poses = vec![TankTrailPose((25, 25, 0)), TankTrailPose((35, 25, 0))];
    let bounds = TrailBounds::from_poses(&poses).unwrap();
    assert!(bounds.min_x_quarter_px <= 300);
    assert!(bounds.max_x_quarter_px >= 660);
    assert!(bounds.min_y_quarter_px <= 336);
    assert!(bounds.max_y_quarter_px >= 464);
}

#[test]
fn checkpoint_accepts_an_active_chunk_at_the_runtime_pose_limit() {
    let map = Map::generate(1, 7);
    let pose = TankTrailPose((25, 25, 0));
    let mut store = TankTrailStore::new();
    store.active_by_tank.insert(
        7,
        ActiveTankTrail {
            owner: 1,
            poses: vec![pose; MAX_ACTIVE_POSES],
            last_observed: pose,
            last_motion_tick: 3,
        },
    );
    assert!(store.valid_checkpoint_state(&map, &BTreeSet::from([1]), 3));
}

#[test]
fn checkpoint_encoding_packs_poses_and_rebuilds_derived_bounds() {
    let map = Map::generate(1, 7);
    let poses = vec![TankTrailPose((25, 25, 0)), TankTrailPose((41, 25, 0))];
    let pending = TankTrailStore::pending(1, poses).unwrap();
    let mut store = TankTrailStore::new();
    assert!(store.commit(pending, 1, 7, &map));

    let json = serde_json::to_string(&store).unwrap();
    assert!(json.len() < 64, "compact checkpoint row was {json}");
    assert!(!json.contains("bounds"));
    assert!(!json.contains("nextId"));

    let restored: TankTrailStore = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.view(1), store.view(1));
    assert_eq!(restored.finalized[0].bounds, store.finalized[0].bounds);
}

#[test]
fn finalized_history_has_a_hard_memory_ceiling() {
    let map = Map::generate(1, 7);
    let poses = vec![TankTrailPose((25, 25, 0)), TankTrailPose((41, 25, 0))];
    let pending = TankTrailStore::pending(1, poses).unwrap();
    let mut store = TankTrailStore::new();
    assert!(store.commit(pending.clone(), 1, 1, &map));
    let prototype = store.finalized[0].clone();
    store.finalized = (1..=MAX_FINALIZED_TRAILS)
        .map(|id| FinalizedTankTrail {
            id: id as u32,
            created_revision: id as u32,
            ..prototype.clone()
        })
        .collect();
    store.next_id = MAX_FINALIZED_TRAILS as u32 + 1;

    assert!(store.next_id().is_none());
    assert!(!store.commit(
        pending,
        MAX_FINALIZED_TRAILS as u32 + 1,
        MAX_FINALIZED_TRAILS as u32 + 1,
        &map,
    ));
}
