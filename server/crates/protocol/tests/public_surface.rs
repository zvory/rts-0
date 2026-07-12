use rts_protocol::{
    abilities, ability_object_kinds, default_snapshot_codec, encode_snapshot_frame, kinds,
    lab_replay_artifact_from_slice, protocol_contract, serialize_compact_snapshot,
    serialize_messagepack_compact_snapshot, states, supported_snapshot_codec, terrain, upgrades,
    validate_lab_replay_artifact, AbilityCooldownView, AbilityObjectOwnerStateView,
    AbilityObjectView, ActionCapabilities, AttackReveal, AvailableMap, BranchStagingOccupant,
    BranchStagingSeat, ClientMessage, ClientNetReport, Command, CommandCapabilities,
    CompactSlotSchemas, DebugPathPoint, DebugPathView, DiagnosticCapabilities, EntityView, Event,
    InitialCamera, LabCheckpointScenarioMap, LabCheckpointScenarioMapData,
    LabCheckpointScenarioMetadata, LabCheckpointScenarioSource, LabCheckpointScenarioV1,
    LabClientOp, LabReplayArtifactV1, LabReplayAuthoringMetadata, LabReplayOperation,
    LabReplayOperationEntry, LabReplayTimelineMetadata, LabReplayValidationError, LabResult,
    LabScenarioEntityIdRemap, LabScenarioLabMetadata, LabScenarioPayload, LabScenarioTile,
    LabSpawnEntitySpec, LabStartMetadata, LabStartRole, LabState, LabUpdateSpec, LabVisionMode,
    LivePauseState, LobbyPlayer, MapInfo, MatchControlCapabilities, MovementPathDiagnosticScope,
    NoticeSeverity, ObserverAnalysisAiDiagnostics, ObserverAnalysisKindCount,
    ObserverAnalysisPayload, ObserverAnalysisPlayer, ObserverAnalysisProduction,
    ObserverAnalysisResourcesLost, OrderPlanMarker, PlayerResourceSnapshot, PlayerScore,
    PlayerStart, ProtocolCompactCodes, ProtocolContract, ProtocolMessageTags, ProtocolVocabularies,
    RememberedBuildingView, ReplayBranchSeat, ReplayStartMetadata, ResourceDelta, ResourceNode,
    RoomCapabilities, RoomTimeCapabilities, RoomTimeState, ScoutPlaneStateView, ServerMessage,
    SlotField, SmokeCloudView, Snapshot, SnapshotCodec, SnapshotCodecContract, SnapshotEncodeError,
    SnapshotFrame, SnapshotNetStatus, StartPayload, TeamId, TrenchView, VisibilityCapabilities,
    VisionSelectionRequest, COMPACT_SNAPSHOT_VERSION, COMPACT_UNKNOWN_CODE, DEFAULT_FACTION_ID,
    LAB_REPLAY_ARTIFACT_KIND, LAB_REPLAY_ARTIFACT_SCHEMA, LAB_REPLAY_ARTIFACT_SCHEMA_VERSION,
    LAB_REPLAY_MAX_ARTIFACT_BYTES, LAB_REPLAY_MAX_OPERATIONS,
    LAB_REPLAY_TIMELINE_KEYFRAME_INTERVAL_TICKS, MESSAGEPACK_SNAPSHOT_FRAME_MAGIC,
    PREDICTION_PROTOCOL_VERSION, SNAPSHOT_CODEC_COMPACT_JSON, SNAPSHOT_CODEC_MESSAGEPACK_COMPACT,
    SNAPSHOT_CODEC_VERSION, SNAPSHOT_FRAME_KIND_BINARY, SNAPSHOT_FRAME_KIND_TEXT,
};
use rts_protocol::{LabMapDraft, LabMapTile};

fn assert_type<T>() {}

#[test]
fn stable_rust_public_surface_compiles() {
    assert_type::<AbilityCooldownView>();
    assert_type::<AbilityObjectOwnerStateView>();
    assert_type::<AbilityObjectView>();
    assert_type::<ActionCapabilities>();
    assert_type::<AttackReveal>();
    assert_type::<AvailableMap>();
    assert_type::<BranchStagingOccupant>();
    assert_type::<BranchStagingSeat>();
    assert_type::<ClientMessage>();
    assert_type::<ClientNetReport>();
    assert_type::<Command>();
    assert_type::<CommandCapabilities>();
    assert_type::<CompactSlotSchemas>();
    assert_type::<DebugPathPoint>();
    assert_type::<DebugPathView>();
    assert_type::<DiagnosticCapabilities>();
    assert_type::<EntityView>();
    assert_type::<Event>();
    assert_type::<InitialCamera>();
    assert_type::<LabClientOp>();
    assert_type::<LabMapDraft>();
    assert_type::<LabMapTile>();
    assert_type::<LabReplayArtifactV1>();
    assert_type::<LabReplayAuthoringMetadata>();
    assert_type::<LabReplayOperation>();
    assert_type::<LabReplayOperationEntry>();
    assert_type::<LabReplayTimelineMetadata>();
    assert_type::<LabReplayValidationError>();
    assert_type::<LabResult>();
    assert_type::<LabSpawnEntitySpec>();
    assert_type::<LabUpdateSpec>();
    assert_type::<LabCheckpointScenarioMap>();
    assert_type::<LabCheckpointScenarioMapData>();
    assert_type::<LabCheckpointScenarioMetadata>();
    assert_type::<LabCheckpointScenarioSource>();
    assert_type::<LabCheckpointScenarioV1>();
    assert_type::<LabScenarioEntityIdRemap>();
    assert_type::<LabScenarioLabMetadata>();
    assert_type::<LabScenarioPayload>();
    assert_type::<LabScenarioTile>();
    assert_type::<LabStartMetadata>();
    assert_type::<LabStartRole>();
    assert_type::<LabState>();
    assert_type::<LabVisionMode>();
    assert_type::<LivePauseState>();
    assert_type::<LobbyPlayer>();
    assert_type::<MapInfo>();
    assert_type::<MatchControlCapabilities>();
    assert_type::<MovementPathDiagnosticScope>();
    assert_type::<NoticeSeverity>();
    assert_type::<ObserverAnalysisKindCount>();
    assert_type::<ObserverAnalysisAiDiagnostics>();
    assert_type::<ObserverAnalysisPayload>();
    assert_type::<ObserverAnalysisPlayer>();
    assert_type::<ObserverAnalysisProduction>();
    assert_type::<ObserverAnalysisResourcesLost>();
    assert_type::<OrderPlanMarker>();
    assert_type::<PlayerResourceSnapshot>();
    assert_type::<PlayerScore>();
    assert_type::<PlayerStart>();
    assert_type::<ProtocolCompactCodes>();
    assert_type::<ProtocolContract>();
    assert_type::<ProtocolMessageTags>();
    assert_type::<ProtocolVocabularies>();
    assert_type::<RememberedBuildingView>();
    assert_type::<ReplayBranchSeat>();
    assert_type::<ReplayStartMetadata>();
    assert_type::<VisionSelectionRequest>();
    assert_type::<ResourceDelta>();
    assert_type::<ResourceNode>();
    assert_type::<RoomCapabilities>();
    assert_type::<RoomTimeCapabilities>();
    assert_type::<RoomTimeState>();
    assert_type::<ScoutPlaneStateView>();
    assert_type::<ServerMessage>();
    assert_type::<SlotField>();
    assert_type::<SmokeCloudView>();
    assert_type::<TrenchView>();
    assert_type::<Snapshot>();
    assert_type::<SnapshotCodec>();
    assert_type::<SnapshotCodecContract>();
    assert_type::<SnapshotEncodeError>();
    assert_type::<SnapshotFrame>();
    assert_type::<SnapshotNetStatus>();
    assert_type::<StartPayload>();
    assert_type::<TeamId>();
    assert_type::<VisibilityCapabilities>();

    let _default_codec: fn() -> SnapshotCodec = default_snapshot_codec;
    let _supported_codec: fn(&str, u16) -> bool = supported_snapshot_codec;
    let _contract: fn() -> ProtocolContract = protocol_contract;
    let _encode: fn(&Snapshot, SnapshotCodec) -> Result<SnapshotFrame, SnapshotEncodeError> =
        encode_snapshot_frame;
    let _compact_json: fn(&Snapshot) -> serde_json::Result<String> = serialize_compact_snapshot;
    let _messagepack: fn(&Snapshot) -> Result<Vec<u8>, SnapshotEncodeError> =
        serialize_messagepack_compact_snapshot;
    let _lab_replay_parse: fn(&[u8]) -> Result<LabReplayArtifactV1, LabReplayValidationError> =
        lab_replay_artifact_from_slice;
    let _lab_replay_validate: fn(&LabReplayArtifactV1) -> Result<(), LabReplayValidationError> =
        validate_lab_replay_artifact;

    assert_eq!(terrain::GRASS, 0);
    assert_eq!(terrain::ROCK, 1);
    assert_eq!(terrain::WATER, 2);
    assert_eq!(kinds::WORKER, "worker");
    assert_eq!(kinds::SCOUT_PLANE, "scout_plane");
    assert_eq!(kinds::CITY_CENTRE, "city_centre");
    assert_eq!(kinds::STEEL, "steel");
    assert_eq!(states::IDLE, "idle");
    assert_eq!(states::ATTACK, "attack");
    assert_eq!(abilities::SMOKE, "smoke");
    assert_eq!(abilities::SCOUT_PLANE, "scoutPlane");
    assert_eq!(abilities::DISMISS_SCOUT_PLANE, "dismissScoutPlane");
    assert_eq!(abilities::EKAT_MAGIC_ANCHOR, "ekatMagicAnchor");
    assert_eq!(ability_object_kinds::RETURN_MARKER, "returnMarker");
    assert_eq!(upgrades::METHAMPHETAMINES, "methamphetamines");
    assert_eq!(upgrades::ENTRENCHMENT, "entrenchment");
    assert_eq!(upgrades::SMOKE_PLUS, "smoke_plus");

    assert_eq!(PREDICTION_PROTOCOL_VERSION, 1);
    assert_eq!(DEFAULT_FACTION_ID, "kriegsia");
    assert_eq!(COMPACT_SNAPSHOT_VERSION, 36);
    assert_eq!(SNAPSHOT_CODEC_VERSION, 1);
    assert_eq!(COMPACT_UNKNOWN_CODE, 255);
    assert_eq!(LAB_REPLAY_ARTIFACT_SCHEMA, "rts.labReplay");
    assert_eq!(LAB_REPLAY_ARTIFACT_KIND, "labReplay");
    assert_eq!(LAB_REPLAY_ARTIFACT_SCHEMA_VERSION, 1);
    assert_eq!(LAB_REPLAY_TIMELINE_KEYFRAME_INTERVAL_TICKS, 2_000);
    assert_eq!(LAB_REPLAY_MAX_ARTIFACT_BYTES, 8 * 1024 * 1024);
    assert_eq!(LAB_REPLAY_MAX_OPERATIONS, 50_000);
    assert_eq!(MESSAGEPACK_SNAPSHOT_FRAME_MAGIC, [0x52, 0x54, 0x53, 0x4d]);

    let codec = default_snapshot_codec();
    assert_eq!(codec, SnapshotCodec::MessagePackCompact);
    assert_eq!(codec.name(), SNAPSHOT_CODEC_MESSAGEPACK_COMPACT);
    assert_eq!(codec.version(), SNAPSHOT_CODEC_VERSION);
    assert_eq!(codec.frame_kind(), SNAPSHOT_FRAME_KIND_BINARY);
    assert!(supported_snapshot_codec(
        SNAPSHOT_CODEC_MESSAGEPACK_COMPACT,
        SNAPSHOT_CODEC_VERSION
    ));
    assert!(!supported_snapshot_codec(
        SNAPSHOT_CODEC_COMPACT_JSON,
        SNAPSHOT_CODEC_VERSION
    ));
    assert_eq!(
        SnapshotCodec::CompactJson.frame_kind(),
        SNAPSHOT_FRAME_KIND_TEXT
    );
    assert_eq!(
        SnapshotFrame::Text(String::new()).frame_kind(),
        SNAPSHOT_FRAME_KIND_TEXT
    );
    assert_eq!(
        SnapshotFrame::Binary(Vec::new()).frame_kind(),
        SNAPSHOT_FRAME_KIND_BINARY
    );

    serde_json::to_value(protocol_contract()).expect("protocol contract remains serializable");
}

#[test]
fn compact_snapshot_encodes_scout_plane_owner_state() {
    let mut plane = EntityView::new(5, 1, kinds::SCOUT_PLANE, 160.0, 170.0, 40, 40, states::IDLE);
    plane.scout_plane = Some(ScoutPlaneStateView {
        orbit_center: Some([512.0, 544.0]),
    });

    let snapshot = Snapshot {
        tick: 1,
        steel: 0,
        oil: 0,
        supply_used: 0,
        supply_cap: 0,
        entities: vec![plane],
        resource_deltas: Vec::new(),
        smokes: Vec::new(),
        ability_objects: Vec::new(),
        trenches: Vec::new(),
        visible_tiles: Vec::new(),
        remembered_buildings: Vec::new(),
        events: Vec::new(),
        upgrades: Vec::new(),
        player_resources: Vec::new(),
        production_queue: Vec::new(),
        net_status: SnapshotNetStatus {
            server_lag_ms: 0,
            tick_ms: 33,
            slow_tick: false,
            slow_tick_count: 0,
            head_of_line: false,
            head_of_line_count: 0,
            prediction_version: 0,
            last_sim_consumed_client_seq: 0,
            last_sim_consumed_client_tick: None,
        },
    };

    let compact = serialize_compact_snapshot(&snapshot).unwrap();
    let value: serde_json::Value = serde_json::from_str(&compact).unwrap();
    assert_eq!(value["e"][0][2], serde_json::json!(25));
    assert_eq!(value["e"][0][33], serde_json::json!([[512.0, 544.0]]));
}
