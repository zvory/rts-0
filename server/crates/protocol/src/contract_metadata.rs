//! Protocol vocabulary, compact codes, and structured contract metadata.

use serde::Serialize;
use std::collections::BTreeMap;

use rts_contract::{NoticeSeverity, DEFAULT_FACTION_ID};

// ---------------------------------------------------------------------------
// Shared string vocabularies (kept as constants so the simulation never sprays
// magic strings; the JS mirror has the same values).
// ---------------------------------------------------------------------------

/// Terrain codes packed into `MapInfo.terrain` (row-major).
pub mod terrain {
    pub const GRASS: u8 = 0; // passable
    pub const ROCK: u8 = 1; // impassable
    pub const WATER: u8 = 2; // impassable
    pub const ROAD_BARE: u8 = 3; // passable road terrain
    pub const ROAD_HORIZONTAL: u8 = 4; // passable road terrain
    pub const ROAD_VERTICAL: u8 = 5; // passable road terrain
    pub const ROAD_DIAGONAL_NW_SE: u8 = 6; // passable road terrain
    pub const ROAD_DIAGONAL_NE_SW: u8 = 7; // passable road terrain
}

/// `EntityView.kind` values.
pub mod kinds {
    pub const WORKER: &str = "worker";
    pub const GOLEM: &str = "golem";
    pub const RIFLEMAN: &str = "rifleman";
    pub const PANZERFAUST: &str = "panzerfaust";
    pub const MACHINE_GUNNER: &str = "machine_gunner";
    pub const ANTI_TANK_GUN: &str = "anti_tank_gun";
    pub const MORTAR_TEAM: &str = "mortar_team";
    pub const ARTILLERY: &str = "artillery";
    pub const SCOUT_CAR: &str = "scout_car";
    pub const SCOUT_PLANE: &str = "scout_plane";
    pub const TANK: &str = "tank";
    pub const COMMAND_CAR: &str = "command_car";
    pub const EKAT: &str = "ekat";
    pub const CITY_CENTRE: &str = "city_centre";
    pub const ZAMOK: &str = "zamok";
    pub const DEPOT: &str = "depot";
    pub const BARRACKS: &str = "barracks";
    pub const TRAINING_CENTRE: &str = "training_centre";
    pub const RESEARCH_COMPLEX: &str = "research_complex";
    pub const FACTORY: &str = "factory";
    pub const STEELWORKS: &str = "steelworks";
    pub const TANK_TRAP: &str = "tank_trap";
    pub const PUMP_JACK: &str = "pump_jack";
    pub const STEEL: &str = "steel";
    pub const OIL: &str = "oil";
}

/// `EntityView.state` values.
pub mod states {
    pub const IDLE: &str = "idle";
    pub const MOVE: &str = "move";
    pub const ATTACK: &str = "attack";
    pub const GATHER: &str = "gather";
    pub const BUILD: &str = "build";
    pub const TRAIN: &str = "train";
    pub const CONSTRUCT: &str = "construct";
    pub const DEAD: &str = "dead";
}

/// Ability ids used by generic ability commands and owner-only cooldown projection.
pub mod abilities {
    pub const CHARGE: &str = "charge";
    pub const SMOKE: &str = "smoke";
    pub const MORTAR_FIRE: &str = "mortarFire";
    pub const POINT_FIRE: &str = "pointFire";
    pub const BLANKET_FIRE: &str = "blanketFire";
    pub const BREAKTHROUGH: &str = "breakthrough";
    pub const SCOUT_PLANE: &str = "scoutPlane";
    pub const DISMISS_SCOUT_PLANE: &str = "dismissScoutPlane";
    pub const EKAT_TELEPORT: &str = "ekatTeleport";
    pub const EKAT_LINE_SHOT: &str = "ekatLineShot";
    pub const EKAT_MAGIC_ANCHOR: &str = "ekatMagicAnchor";
    pub const EKAT_CONSUME_GOLEM: &str = "ekatConsumeGolem";
    pub const ALL: &[&str] = &[
        CHARGE,
        SMOKE,
        MORTAR_FIRE,
        POINT_FIRE,
        BLANKET_FIRE,
        BREAKTHROUGH,
        SCOUT_PLANE,
        DISMISS_SCOUT_PLANE,
        EKAT_TELEPORT,
        EKAT_LINE_SHOT,
        EKAT_MAGIC_ANCHOR,
        EKAT_CONSUME_GOLEM,
    ];
}

/// `AbilityObjectView.kind` values.
pub mod ability_object_kinds {
    pub const RETURN_MARKER: &str = "returnMarker";
    pub const MAGIC_ANCHOR: &str = "magicAnchor";
    pub const LINE_PROJECTILE: &str = "lineProjectile";
}

/// Lobby room kinds surfaced in WebSocket lobby state and HTTP lobby-browser rows.
pub mod lobby_kinds {
    pub const NORMAL: &str = "normal";
    pub const REPLAY: &str = "replay";
}

/// Permanent upgrade ids used by production/research and snapshot projection.
pub mod upgrades {
    pub const METHAMPHETAMINES: &str = "methamphetamines";
    pub const PANZERFAUSTS: &str = "panzerfausts";
    pub const ENTRENCHMENT: &str = "entrenchment";
    pub const ANTI_TANK_GUN_UNLOCK: &str = "anti_tank_gun_unlock";
    pub const TANK_UNLOCK: &str = "tank_unlock";
    pub const ARTILLERY_UNLOCK: &str = "artillery_unlock";
    pub const BALLISTIC_TABLES: &str = "ballistic_tables";
    pub const MORTAR_AUTOCAST: &str = "mortar_autocast";
    pub const SMOKE_PLUS: &str = "smoke_plus";
    pub const ALL: &[&str] = &[
        METHAMPHETAMINES,
        PANZERFAUSTS,
        ENTRENCHMENT,
        ANTI_TANK_GUN_UNLOCK,
        ARTILLERY_UNLOCK,
        BALLISTIC_TABLES,
        TANK_UNLOCK,
        MORTAR_AUTOCAST,
        SMOKE_PLUS,
    ];
}

/// Closed `Event::Attack.weaponKind` values.
pub mod weapons {
    pub const WORKER_TOOLS: &str = "worker_tools";
    pub const GOLEM_FISTS: &str = "golem_fists";
    pub const RIFLEMAN_RIFLE: &str = "rifleman_rifle";
    pub const MACHINE_GUNNER_MG: &str = "machine_gunner_mg";
    pub const SCOUT_CAR_MG: &str = "scout_car_mg";
    pub const ANTI_TANK_GUN: &str = "anti_tank_gun";
    pub const PANZERFAUST_LOADED_SHOT: &str = "panzerfaust_loaded_shot";
    pub const MORTAR_TEAM_MORTAR: &str = "mortar_team_mortar";
    pub const ARTILLERY_GUN: &str = "artillery_gun";
    pub const TANK_CANNON: &str = "tank_cannon";
    pub const TANK_COAX: &str = "tank_coax";
}

/// Version for the array-shaped compact snapshot representation sent over WebSocket.
///
/// [`Snapshot`] remains the semantic source of truth for game code. This format is only a
/// transport-side optimization for `ServerMessage::Snapshot`.
pub const PREDICTION_PROTOCOL_VERSION: u32 = 1;

pub const COMPACT_SNAPSHOT_VERSION: u8 = 45;

pub const SNAPSHOT_CODEC_COMPACT_JSON: &str = "compact-json";
pub const SNAPSHOT_CODEC_MESSAGEPACK_COMPACT: &str = "messagepack-compact";

pub const SNAPSHOT_CODEC_VERSION: u16 = 1;
pub const SNAPSHOT_FRAME_KIND_TEXT: &str = "text";
pub const SNAPSHOT_FRAME_KIND_BINARY: &str = "binary";

pub const COMPACT_UNKNOWN_CODE: u8 = 255;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolContract {
    schema_version: u8,
    compact_snapshot_version: u8,
    prediction_protocol_version: u32,
    snapshot_codecs: SnapshotCodecContract,
    default_faction_id: &'static str,
    unknown_code_sentinel: u8,
    message_tags: ProtocolMessageTags,
    command_tags: BTreeMap<&'static str, &'static str>,
    vocabularies: ProtocolVocabularies,
    compact_codes: ProtocolCompactCodes,
    compact_slot_schemas: CompactSlotSchemas,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotCodecContract {
    default_codec: &'static str,
    codec_version: u16,
    default_frame_kind: &'static str,
    supported: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProtocolMessageTags {
    client: BTreeMap<&'static str, &'static str>,
    server: BTreeMap<&'static str, &'static str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolVocabularies {
    terrain: BTreeMap<&'static str, u8>,
    kinds: BTreeMap<&'static str, &'static str>,
    states: BTreeMap<&'static str, &'static str>,
    setup_states: BTreeMap<&'static str, &'static str>,
    events: BTreeMap<&'static str, &'static str>,
    abilities: BTreeMap<&'static str, &'static str>,
    ability_object_kinds: BTreeMap<&'static str, &'static str>,
    lobby_kinds: BTreeMap<&'static str, &'static str>,
    upgrades: BTreeMap<&'static str, &'static str>,
    weapon_kinds: BTreeMap<&'static str, &'static str>,
    notice_severities: BTreeMap<&'static str, &'static str>,
    order_stages: BTreeMap<&'static str, &'static str>,
    resource_kinds: BTreeMap<&'static str, &'static str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolCompactCodes {
    terrain: BTreeMap<&'static str, u8>,
    kind: BTreeMap<&'static str, u8>,
    state: BTreeMap<&'static str, u8>,
    setup_state: BTreeMap<&'static str, u8>,
    event: BTreeMap<&'static str, u8>,
    order_stage: BTreeMap<&'static str, u8>,
    ability: BTreeMap<&'static str, u8>,
    ability_object_kind: BTreeMap<&'static str, u8>,
    upgrade: BTreeMap<&'static str, u8>,
    weapon_kind: BTreeMap<&'static str, u8>,
    notice_severity: BTreeMap<&'static str, u8>,
    resource_kind: BTreeMap<&'static str, u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactSlotSchemas {
    snapshot: Vec<SlotField>,
    entity: Vec<SlotField>,
    event: BTreeMap<&'static str, Vec<SlotField>>,
    trench: Vec<SlotField>,
    ability_object: Vec<SlotField>,
    ability_object_owner_state: Vec<SlotField>,
    ability_cooldown: Vec<SlotField>,
    order_plan_marker: Vec<SlotField>,
    net_status: Vec<SlotField>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotField {
    index: u8,
    name: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    code_map: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    optional: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    omitted_when_empty: Option<bool>,
}

fn field(index: u8, name: &'static str) -> SlotField {
    SlotField {
        index,
        name,
        code_map: None,
        optional: None,
        omitted_when_empty: None,
    }
}

fn code_field(index: u8, name: &'static str, code_map: &'static str) -> SlotField {
    SlotField {
        index,
        name,
        code_map: Some(code_map),
        optional: None,
        omitted_when_empty: None,
    }
}

fn optional_field(index: u8, name: &'static str) -> SlotField {
    SlotField {
        index,
        name,
        code_map: None,
        optional: Some(true),
        omitted_when_empty: None,
    }
}

fn optional_code_field(index: u8, name: &'static str, code_map: &'static str) -> SlotField {
    SlotField {
        index,
        name,
        code_map: Some(code_map),
        optional: Some(true),
        omitted_when_empty: None,
    }
}

fn omitted_field(index: u8, name: &'static str) -> SlotField {
    SlotField {
        index,
        name,
        code_map: None,
        optional: None,
        omitted_when_empty: Some(true),
    }
}

fn string_map(entries: &[(&'static str, &'static str)]) -> BTreeMap<&'static str, &'static str> {
    entries.iter().copied().collect()
}

fn code_map(entries: &[(&'static str, u8)]) -> BTreeMap<&'static str, u8> {
    entries.iter().copied().collect()
}

const KIND_CODES: &[(&str, u8)] = &[
    (kinds::WORKER, 1),
    (kinds::GOLEM, 22),
    (kinds::RIFLEMAN, 2),
    (kinds::PANZERFAUST, 24),
    (kinds::MACHINE_GUNNER, 3),
    (kinds::ANTI_TANK_GUN, 4),
    (kinds::MORTAR_TEAM, 15),
    (kinds::ARTILLERY, 16),
    (kinds::TANK, 5),
    (kinds::SCOUT_CAR, 14),
    (kinds::SCOUT_PLANE, 25),
    (kinds::CITY_CENTRE, 6),
    (kinds::DEPOT, 7),
    (kinds::BARRACKS, 8),
    (kinds::TRAINING_CENTRE, 9),
    (kinds::FACTORY, 10),
    (kinds::STEEL, 11),
    (kinds::OIL, 12),
    (kinds::STEELWORKS, 13),
    (kinds::RESEARCH_COMPLEX, 17),
    (kinds::COMMAND_CAR, 18),
    (kinds::EKAT, 19),
    (kinds::ZAMOK, 20),
    (kinds::TANK_TRAP, 21),
    (kinds::PUMP_JACK, 23),
];

const STATE_CODES: &[(&str, u8)] = &[
    (states::IDLE, 1),
    (states::MOVE, 2),
    (states::ATTACK, 3),
    (states::GATHER, 4),
    (states::BUILD, 5),
    (states::TRAIN, 6),
    (states::CONSTRUCT, 7),
    (states::DEAD, 8),
];

const SETUP_STATE_CODES: &[(&str, u8)] = &[
    ("packed", 1),
    ("setting_up", 2),
    ("deployed", 3),
    ("tearing_down", 4),
];

const EVENT_CODES: &[(&str, u8)] = &[
    ("attack", 1),
    ("death", 2),
    ("build", 3),
    ("notice", 4),
    ("smokeLaunch", 5),
    ("mortarImpact", 6),
    ("artilleryTarget", 7),
    ("artilleryImpact", 8),
    ("mortarLaunch", 9),
    ("overpenetration", 10),
    ("artilleryFiring", 11),
    ("panzerfaustLaunch", 12),
    ("panzerfaustImpact", 13),
    ("miss", 15),
];

const ORDER_STAGE_CODES: &[(&str, u8)] = &[
    ("move", 1),
    ("attackMove", 2),
    ("attack", 3),
    ("gather", 4),
    ("build", 5),
    (abilities::SMOKE, 6),
    ("setupAntiTankGuns", 7),
    (abilities::CHARGE, 8),
    (abilities::MORTAR_FIRE, 9),
    (abilities::POINT_FIRE, 10),
    (abilities::BREAKTHROUGH, 11),
    (abilities::EKAT_TELEPORT, 12),
    (abilities::EKAT_LINE_SHOT, 13),
    (abilities::EKAT_MAGIC_ANCHOR, 14),
    ("deconstruct", 15),
    (abilities::EKAT_CONSUME_GOLEM, 16),
    (abilities::BLANKET_FIRE, 17),
    (abilities::DISMISS_SCOUT_PLANE, 18),
    (abilities::SCOUT_PLANE, 19),
    ("holdPosition", 20),
];

const ABILITY_CODES: &[(&str, u8)] = &[
    (abilities::CHARGE, 1),
    (abilities::SMOKE, 2),
    (abilities::MORTAR_FIRE, 3),
    (abilities::POINT_FIRE, 4),
    (abilities::BREAKTHROUGH, 5),
    (abilities::EKAT_TELEPORT, 6),
    (abilities::EKAT_LINE_SHOT, 7),
    (abilities::EKAT_MAGIC_ANCHOR, 8),
    (abilities::EKAT_CONSUME_GOLEM, 9),
    (abilities::BLANKET_FIRE, 10),
    (abilities::DISMISS_SCOUT_PLANE, 11),
    (abilities::SCOUT_PLANE, 12),
];

const ABILITY_OBJECT_KIND_CODES: &[(&str, u8)] = &[
    (ability_object_kinds::RETURN_MARKER, 1),
    (ability_object_kinds::MAGIC_ANCHOR, 2),
    (ability_object_kinds::LINE_PROJECTILE, 3),
];

const UPGRADE_CODES: &[(&str, u8)] = &[
    (upgrades::METHAMPHETAMINES, 1),
    (upgrades::PANZERFAUSTS, 10),
    (upgrades::ANTI_TANK_GUN_UNLOCK, 2),
    (upgrades::TANK_UNLOCK, 3),
    (upgrades::ARTILLERY_UNLOCK, 4),
    (upgrades::MORTAR_AUTOCAST, 5),
    (upgrades::BALLISTIC_TABLES, 7),
    (upgrades::ENTRENCHMENT, 8),
    (upgrades::SMOKE_PLUS, 9),
];

const WEAPON_KIND_CODES: &[(&str, u8)] = &[
    (weapons::WORKER_TOOLS, 1),
    (weapons::GOLEM_FISTS, 2),
    (weapons::RIFLEMAN_RIFLE, 3),
    (weapons::MACHINE_GUNNER_MG, 4),
    (weapons::SCOUT_CAR_MG, 5),
    (weapons::ANTI_TANK_GUN, 6),
    (weapons::PANZERFAUST_LOADED_SHOT, 7),
    (weapons::MORTAR_TEAM_MORTAR, 8),
    (weapons::ARTILLERY_GUN, 9),
    (weapons::TANK_CANNON, 10),
    (weapons::TANK_COAX, 11),
];

fn lookup_code(entries: &[(&str, u8)], value: &str) -> u8 {
    entries
        .iter()
        .find_map(|(name, code)| (*name == value).then_some(*code))
        .unwrap_or(COMPACT_UNKNOWN_CODE)
}

pub(crate) fn event_code(event: &str) -> u8 {
    lookup_code(EVENT_CODES, event)
}

pub fn protocol_contract() -> ProtocolContract {
    ProtocolContract {
        schema_version: 1,
        compact_snapshot_version: COMPACT_SNAPSHOT_VERSION,
        prediction_protocol_version: PREDICTION_PROTOCOL_VERSION,
        snapshot_codecs: SnapshotCodecContract {
            default_codec: SNAPSHOT_CODEC_MESSAGEPACK_COMPACT,
            codec_version: SNAPSHOT_CODEC_VERSION,
            default_frame_kind: SNAPSHOT_FRAME_KIND_BINARY,
            supported: vec![SNAPSHOT_CODEC_MESSAGEPACK_COMPACT],
        },
        default_faction_id: DEFAULT_FACTION_ID,
        unknown_code_sentinel: COMPACT_UNKNOWN_CODE,
        message_tags: ProtocolMessageTags {
            client: string_map(&[
                ("JOIN", "join"),
                ("SET_NAME", "setName"),
                ("READY", "ready"),
                ("START", "start"),
                ("SET_TEAM_PRESET", "setTeamPreset"),
                ("SET_TEAM", "setTeam"),
                ("SET_FACTION", "setFaction"),
                ("ADD_AI", "addAi"),
                ("SET_AI_PROFILE", "setAiProfile"),
                ("REMOVE_AI", "removeAi"),
                ("SET_SPECTATOR", "setSpectator"),
                ("COMMAND", "command"),
                ("GIVE_UP", "giveUp"),
                ("PAUSE_GAME", "pauseGame"),
                ("UNPAUSE_GAME", "unpauseGame"),
                ("RETURN_TO_LOBBY", "returnToLobby"),
                ("PING", "ping"),
                ("NET_REPORT", "netReport"),
                ("ACTIVITY", "activity"),
                ("SET_ROOM_TIME_SPEED", "setRoomTimeSpeed"),
                ("STEP_ROOM_TIME", "stepRoomTime"),
                ("SEEK_ROOM_TIME", "seekRoomTime"),
                ("SEEK_ROOM_TIME_TO", "seekRoomTimeTo"),
                ("SET_VISION_SELECTION", "setVisionSelection"),
                ("LAB", "lab"),
                ("REQUEST_BRANCH_FROM_TICK", "requestBranchFromTick"),
                ("CLAIM_BRANCH_SEAT", "claimBranchSeat"),
                ("RELEASE_BRANCH_SEAT", "releaseBranchSeat"),
                ("START_BRANCH", "startBranch"),
                ("SELECT_MAP", "selectMap"),
            ]),
            server: string_map(&[
                ("WELCOME", "welcome"),
                ("LOBBY", "lobby"),
                ("MATCH_COUNTDOWN", "matchCountdown"),
                ("START", "start"),
                ("SNAPSHOT", "snapshot"),
                ("ROOM_TIME_STATE", "roomTimeState"),
                ("ROOM_TIME_SEEK_STARTED", "roomTimeSeekStarted"),
                ("LIVE_PAUSE_STATE", "livePauseState"),
                ("OBSERVER_ANALYSIS", "observerAnalysis"),
                ("JOIN_REPLAY_PROMPT", "joinReplayPrompt"),
                ("BRANCH_FROM_TICK_CREATED", "branchFromTickCreated"),
                ("BRANCH_STAGING", "branchStaging"),
                ("LAB_STATE", "labState"),
                ("LAB_RESULT", "labResult"),
                ("SHUTDOWN_WARNING", "shutdownWarning"),
                ("OBSERVATION_READY", "observationReady"),
                ("GAME_OVER", "gameOver"),
                ("PONG", "pong"),
                ("COMMAND_RECEIPT", "commandReceipt"),
                ("ERROR", "error"),
            ]),
        },
        command_tags: string_map(&[
            ("MOVE", "move"),
            ("FORMATION_MOVE", "formationMove"),
            ("ATTACK_MOVE", "attackMove"),
            ("ATTACK", "attack"),
            ("DECONSTRUCT", "deconstruct"),
            ("SETUP_ANTI_TANK_GUNS", "setupAntiTankGuns"),
            ("TEAR_DOWN_ANTI_TANK_GUNS", "tearDownAntiTankGuns"),
            ("CHARGE", "charge"),
            ("USE_ABILITY", "useAbility"),
            ("RECAST_ABILITY", "recastAbility"),
            ("SET_AUTOCAST", "setAutocast"),
            ("GATHER", "gather"),
            ("BUILD", "build"),
            ("TRAIN", "train"),
            ("ADJUST_PRODUCTION_REPEAT", "adjustProductionRepeat"),
            ("RESEARCH", "research"),
            ("CANCEL", "cancel"),
            ("STOP", "stop"),
            ("HOLD_POSITION", "holdPosition"),
            ("SET_RALLY", "setRally"),
        ]),
        vocabularies: ProtocolVocabularies {
            terrain: terrain_codes(),
            kinds: kind_vocabulary(),
            states: string_map(&[
                ("IDLE", states::IDLE),
                ("MOVE", states::MOVE),
                ("ATTACK", states::ATTACK),
                ("GATHER", states::GATHER),
                ("BUILD", states::BUILD),
                ("TRAIN", states::TRAIN),
                ("CONSTRUCT", states::CONSTRUCT),
                ("DEAD", states::DEAD),
            ]),
            setup_states: setup_state_vocabulary(),
            events: event_vocabulary(),
            abilities: ability_vocabulary(),
            ability_object_kinds: ability_object_kind_vocabulary(),
            lobby_kinds: lobby_kind_vocabulary(),
            upgrades: upgrade_vocabulary(),
            weapon_kinds: weapon_kind_vocabulary(),
            notice_severities: notice_severity_vocabulary(),
            order_stages: order_stage_vocabulary(),
            resource_kinds: resource_kind_vocabulary(),
        },
        compact_codes: ProtocolCompactCodes {
            terrain: terrain_codes(),
            kind: kind_codes(),
            state: state_codes(),
            setup_state: setup_state_codes(),
            event: event_codes(),
            order_stage: order_stage_codes(),
            ability: ability_codes(),
            ability_object_kind: ability_object_kind_codes(),
            upgrade: upgrade_codes(),
            weapon_kind: weapon_kind_codes(),
            notice_severity: notice_severity_codes(),
            resource_kind: code_map(&[
                (kinds::STEEL, kind_code(kinds::STEEL)),
                (kinds::OIL, kind_code(kinds::OIL)),
            ]),
        },
        compact_slot_schemas: compact_slot_schemas(),
    }
}

fn terrain_codes() -> BTreeMap<&'static str, u8> {
    code_map(&[
        ("GRASS", terrain::GRASS),
        ("ROCK", terrain::ROCK),
        ("WATER", terrain::WATER),
        ("ROAD_BARE", terrain::ROAD_BARE),
        ("ROAD_HORIZONTAL", terrain::ROAD_HORIZONTAL),
        ("ROAD_VERTICAL", terrain::ROAD_VERTICAL),
        ("ROAD_DIAGONAL_NW_SE", terrain::ROAD_DIAGONAL_NW_SE),
        ("ROAD_DIAGONAL_NE_SW", terrain::ROAD_DIAGONAL_NE_SW),
    ])
}

fn kind_vocabulary() -> BTreeMap<&'static str, &'static str> {
    string_map(&[
        ("WORKER", kinds::WORKER),
        ("GOLEM", kinds::GOLEM),
        ("RIFLEMAN", kinds::RIFLEMAN),
        ("PANZERFAUST", kinds::PANZERFAUST),
        ("MACHINE_GUNNER", kinds::MACHINE_GUNNER),
        ("ANTI_TANK_GUN", kinds::ANTI_TANK_GUN),
        ("MORTAR_TEAM", kinds::MORTAR_TEAM),
        ("ARTILLERY", kinds::ARTILLERY),
        ("SCOUT_CAR", kinds::SCOUT_CAR),
        ("SCOUT_PLANE", kinds::SCOUT_PLANE),
        ("TANK", kinds::TANK),
        ("COMMAND_CAR", kinds::COMMAND_CAR),
        ("EKAT", kinds::EKAT),
        ("CITY_CENTRE", kinds::CITY_CENTRE),
        ("ZAMOK", kinds::ZAMOK),
        ("DEPOT", kinds::DEPOT),
        ("BARRACKS", kinds::BARRACKS),
        ("TRAINING_CENTRE", kinds::TRAINING_CENTRE),
        ("RESEARCH_COMPLEX", kinds::RESEARCH_COMPLEX),
        ("FACTORY", kinds::FACTORY),
        ("STEELWORKS", kinds::STEELWORKS),
        ("TANK_TRAP", kinds::TANK_TRAP),
        ("PUMP_JACK", kinds::PUMP_JACK),
        ("STEEL", kinds::STEEL),
        ("OIL", kinds::OIL),
    ])
}

fn setup_state_vocabulary() -> BTreeMap<&'static str, &'static str> {
    string_map(&[
        ("PACKED", "packed"),
        ("SETTING_UP", "setting_up"),
        ("DEPLOYED", "deployed"),
        ("TEARING_DOWN", "tearing_down"),
    ])
}

fn event_vocabulary() -> BTreeMap<&'static str, &'static str> {
    string_map(&[
        ("ATTACK", "attack"),
        ("DEATH", "death"),
        ("BUILD", "build"),
        ("NOTICE", "notice"),
        ("SMOKE_LAUNCH", "smokeLaunch"),
        ("MORTAR_LAUNCH", "mortarLaunch"),
        ("MORTAR_IMPACT", "mortarImpact"),
        ("ARTILLERY_TARGET", "artilleryTarget"),
        ("ARTILLERY_IMPACT", "artilleryImpact"),
        ("OVERPENETRATION", "overpenetration"),
        ("ARTILLERY_FIRING", "artilleryFiring"),
        ("PANZERFAUST_LAUNCH", "panzerfaustLaunch"),
        ("PANZERFAUST_IMPACT", "panzerfaustImpact"),
        ("MISS", "miss"),
    ])
}

fn ability_vocabulary() -> BTreeMap<&'static str, &'static str> {
    string_map(&[
        ("CHARGE", abilities::CHARGE),
        ("SMOKE", abilities::SMOKE),
        ("MORTAR_FIRE", abilities::MORTAR_FIRE),
        ("POINT_FIRE", abilities::POINT_FIRE),
        ("BLANKET_FIRE", abilities::BLANKET_FIRE),
        ("BREAKTHROUGH", abilities::BREAKTHROUGH),
        ("SCOUT_PLANE", abilities::SCOUT_PLANE),
        ("DISMISS_SCOUT_PLANE", abilities::DISMISS_SCOUT_PLANE),
        ("EKAT_TELEPORT", abilities::EKAT_TELEPORT),
        ("EKAT_LINE_SHOT", abilities::EKAT_LINE_SHOT),
        ("EKAT_MAGIC_ANCHOR", abilities::EKAT_MAGIC_ANCHOR),
        ("EKAT_CONSUME_GOLEM", abilities::EKAT_CONSUME_GOLEM),
    ])
}

fn ability_object_kind_vocabulary() -> BTreeMap<&'static str, &'static str> {
    string_map(&[
        ("RETURN_MARKER", ability_object_kinds::RETURN_MARKER),
        ("MAGIC_ANCHOR", ability_object_kinds::MAGIC_ANCHOR),
        ("LINE_PROJECTILE", ability_object_kinds::LINE_PROJECTILE),
    ])
}

fn lobby_kind_vocabulary() -> BTreeMap<&'static str, &'static str> {
    string_map(&[
        ("NORMAL", lobby_kinds::NORMAL),
        ("REPLAY", lobby_kinds::REPLAY),
    ])
}

fn upgrade_vocabulary() -> BTreeMap<&'static str, &'static str> {
    string_map(&[
        ("METHAMPHETAMINES", upgrades::METHAMPHETAMINES),
        ("PANZERFAUSTS", upgrades::PANZERFAUSTS),
        ("ENTRENCHMENT", upgrades::ENTRENCHMENT),
        ("ANTI_TANK_GUN_UNLOCK", upgrades::ANTI_TANK_GUN_UNLOCK),
        ("TANK_UNLOCK", upgrades::TANK_UNLOCK),
        ("ARTILLERY_UNLOCK", upgrades::ARTILLERY_UNLOCK),
        ("BALLISTIC_TABLES", upgrades::BALLISTIC_TABLES),
        ("MORTAR_AUTOCAST", upgrades::MORTAR_AUTOCAST),
        ("SMOKE_PLUS", upgrades::SMOKE_PLUS),
    ])
}

fn weapon_kind_vocabulary() -> BTreeMap<&'static str, &'static str> {
    string_map(&[
        ("WORKER_TOOLS", weapons::WORKER_TOOLS),
        ("GOLEM_FISTS", weapons::GOLEM_FISTS),
        ("RIFLEMAN_RIFLE", weapons::RIFLEMAN_RIFLE),
        ("MACHINE_GUNNER_MG", weapons::MACHINE_GUNNER_MG),
        ("SCOUT_CAR_MG", weapons::SCOUT_CAR_MG),
        ("ANTI_TANK_GUN", weapons::ANTI_TANK_GUN),
        ("PANZERFAUST_LOADED_SHOT", weapons::PANZERFAUST_LOADED_SHOT),
        ("MORTAR_TEAM_MORTAR", weapons::MORTAR_TEAM_MORTAR),
        ("ARTILLERY_GUN", weapons::ARTILLERY_GUN),
        ("TANK_CANNON", weapons::TANK_CANNON),
        ("TANK_COAX", weapons::TANK_COAX),
    ])
}

fn notice_severity_vocabulary() -> BTreeMap<&'static str, &'static str> {
    string_map(&[("INFO", "info"), ("WARN", "warn"), ("ALERT", "alert")])
}

fn order_stage_vocabulary() -> BTreeMap<&'static str, &'static str> {
    string_map(&[
        ("MOVE", "move"),
        ("ATTACK_MOVE", "attackMove"),
        ("HOLD_POSITION", "holdPosition"),
        ("ATTACK", "attack"),
        ("DECONSTRUCT", "deconstruct"),
        ("GATHER", "gather"),
        ("BUILD", "build"),
        ("CHARGE", abilities::CHARGE),
        ("SMOKE", abilities::SMOKE),
        ("MORTAR_FIRE", abilities::MORTAR_FIRE),
        ("POINT_FIRE", abilities::POINT_FIRE),
        ("BLANKET_FIRE", abilities::BLANKET_FIRE),
        ("BREAKTHROUGH", abilities::BREAKTHROUGH),
        ("SCOUT_PLANE", abilities::SCOUT_PLANE),
        ("DISMISS_SCOUT_PLANE", abilities::DISMISS_SCOUT_PLANE),
        ("EKAT_TELEPORT", abilities::EKAT_TELEPORT),
        ("EKAT_LINE_SHOT", abilities::EKAT_LINE_SHOT),
        ("EKAT_MAGIC_ANCHOR", abilities::EKAT_MAGIC_ANCHOR),
        ("EKAT_CONSUME_GOLEM", abilities::EKAT_CONSUME_GOLEM),
        ("SETUP_ANTI_TANK_GUNS", "setupAntiTankGuns"),
    ])
}

fn resource_kind_vocabulary() -> BTreeMap<&'static str, &'static str> {
    string_map(&[("STEEL", kinds::STEEL), ("OIL", kinds::OIL)])
}

fn kind_codes() -> BTreeMap<&'static str, u8> {
    code_map(KIND_CODES)
}

fn state_codes() -> BTreeMap<&'static str, u8> {
    code_map(STATE_CODES)
}

fn setup_state_codes() -> BTreeMap<&'static str, u8> {
    code_map(SETUP_STATE_CODES)
}

fn event_codes() -> BTreeMap<&'static str, u8> {
    code_map(EVENT_CODES)
}

fn order_stage_codes() -> BTreeMap<&'static str, u8> {
    code_map(ORDER_STAGE_CODES)
}

fn ability_codes() -> BTreeMap<&'static str, u8> {
    code_map(ABILITY_CODES)
}

fn ability_object_kind_codes() -> BTreeMap<&'static str, u8> {
    code_map(ABILITY_OBJECT_KIND_CODES)
}

fn upgrade_codes() -> BTreeMap<&'static str, u8> {
    code_map(UPGRADE_CODES)
}

fn weapon_kind_codes() -> BTreeMap<&'static str, u8> {
    code_map(WEAPON_KIND_CODES)
}

fn notice_severity_codes() -> BTreeMap<&'static str, u8> {
    code_map(&[("info", 1), ("warn", 2), ("alert", 3)])
}

fn compact_slot_schemas() -> CompactSlotSchemas {
    CompactSlotSchemas {
        snapshot: vec![
            field(0, "tick"),
            field(1, "steel"),
            field(2, "oil"),
            field(3, "supplyUsed"),
            field(4, "supplyCap"),
        ],
        entity: vec![
            field(0, "id"),
            field(1, "owner"),
            code_field(2, "kind", "kind"),
            field(3, "x"),
            field(4, "y"),
            field(5, "hp"),
            field(6, "maxHp"),
            code_field(7, "state", "state"),
            optional_field(8, "facing"),
            optional_field(9, "weaponFacing"),
            optional_code_field(10, "prodKind", "kind"),
            optional_field(11, "prodProgress"),
            optional_field(12, "prodQueue"),
            optional_field(13, "buildProgress"),
            optional_field(14, "latchedNode"),
            optional_field(15, "targetId"),
            optional_code_field(16, "setupState", "setupState"),
            optional_field(17, "remaining"),
            optional_field(18, "rally"),
            optional_field(19, "oilUsed"),
            optional_field(20, "setupFacing"),
            optional_field(21, "orderPlan"),
            optional_field(22, "chargeCooldownLeft"),
            optional_field(23, "abilities"),
            optional_field(24, "breakthroughTicks"),
            optional_field(25, "visionOnly"),
            optional_field(26, "debugPath"),
            optional_field(27, "rallyPlan"),
            optional_code_field(28, "prodUpgrade", "upgrade"),
            optional_field(29, "buildActive"),
            optional_field(30, "deconstructProgress"),
            optional_field(31, "weaponRangeTiles"),
            optional_field(32, "occupiedTrenchId"),
            optional_field(33, "scoutPlane"),
            optional_field(34, "prodScoutPlaneQueued"),
            optional_field(35, "panzerfaustLoaded"),
            optional_code_field(36, "prodRepeatKinds", "kind"),
            optional_field(37, "prodWaiting"),
            optional_field(38, "breakthroughAuraTicks"),
            optional_field(39, "extractorActive"),
            optional_code_field(40, "prodUpgradeQueue", "upgrade"),
        ],
        event: event_slot_schemas(),
        trench: vec![
            field(0, "id"),
            field(1, "x"),
            field(2, "y"),
            field(3, "radiusTiles"),
        ],
        ability_object: vec![
            field(0, "id"),
            field(1, "owner"),
            code_field(2, "ability", "ability"),
            code_field(3, "kind", "abilityObjectKind"),
            field(4, "x"),
            field(5, "y"),
            optional_field(6, "expiresIn"),
            optional_field(7, "sourceCasterId"),
            optional_field(8, "ownerState"),
        ],
        ability_object_owner_state: vec![
            optional_field(0, "earliestReturnTick"),
            optional_field(1, "hp"),
            optional_field(2, "radius"),
            optional_field(3, "destroyedLockoutTicks"),
            optional_field(4, "distanceTraveled"),
            optional_field(5, "ticksOut"),
        ],
        ability_cooldown: vec![
            code_field(0, "ability", "ability"),
            field(1, "cooldownLeft"),
            optional_field(2, "remainingUses"),
            optional_field(3, "autocastEnabled"),
            optional_field(4, "activeObjectId"),
            optional_field(5, "availableTick"),
            optional_field(6, "lockoutUntilTick"),
            optional_field(7, "expiresIn"),
        ],
        order_plan_marker: vec![
            code_field(0, "kind", "orderStage"),
            field(1, "x"),
            field(2, "y"),
        ],
        net_status: vec![
            field(0, "serverLagMs"),
            field(1, "tickMs"),
            field(2, "flags"),
            field(3, "slowTickCount"),
            field(4, "headOfLineCount"),
            omitted_field(5, "predictionVersion"),
            omitted_field(6, "lastSimConsumedClientSeq"),
            omitted_field(7, "lastSimConsumedClientTick"),
        ],
    }
}

fn event_slot_schemas() -> BTreeMap<&'static str, Vec<SlotField>> {
    [
        (
            "attack",
            vec![
                code_field(0, "kind", "event"),
                field(1, "from"),
                field(2, "to"),
                optional_field(3, "reveal"),
                optional_field(4, "toPos"),
                optional_code_field(5, "weaponKind", "weaponKind"),
            ],
        ),
        (
            "death",
            vec![
                code_field(0, "kind", "event"),
                field(1, "id"),
                field(2, "x"),
                field(3, "y"),
                code_field(4, "kind", "kind"),
            ],
        ),
        (
            "overpenetration",
            vec![code_field(0, "kind", "event"), field(1, "to")],
        ),
        (
            "build",
            vec![
                code_field(0, "kind", "event"),
                field(1, "id"),
                code_field(2, "kind", "kind"),
            ],
        ),
        (
            "notice",
            vec![
                code_field(0, "kind", "event"),
                field(1, "msg"),
                optional_code_field(2, "severity", "noticeSeverity"),
                optional_field(3, "x"),
                optional_field(4, "y"),
            ],
        ),
        (
            "smokeLaunch",
            vec![
                code_field(0, "kind", "event"),
                field(1, "from"),
                field(2, "to"),
                field(3, "delayTicks"),
            ],
        ),
        (
            "mortarLaunch",
            vec![
                code_field(0, "kind", "event"),
                field(1, "from"),
                field(2, "fromPos"),
                field(3, "toPos"),
                field(4, "radiusTiles"),
                field(5, "delayTicks"),
            ],
        ),
        (
            "mortarImpact",
            vec![
                code_field(0, "kind", "event"),
                field(1, "x"),
                field(2, "y"),
                field(3, "radiusTiles"),
                optional_field(4, "from"),
                optional_field(5, "reveal"),
            ],
        ),
        (
            "artilleryTarget",
            vec![
                code_field(0, "kind", "event"),
                field(1, "from"),
                field(2, "target"),
                field(3, "radiusTiles"),
                field(4, "delayTicks"),
            ],
        ),
        (
            "artilleryImpact",
            vec![
                code_field(0, "kind", "event"),
                field(1, "x"),
                field(2, "y"),
                field(3, "radiusTiles"),
            ],
        ),
        (
            "artilleryFiring",
            vec![
                code_field(0, "kind", "event"),
                field(1, "owner"),
                field(2, "x"),
                field(3, "y"),
                field(4, "facing"),
            ],
        ),
        (
            "panzerfaustLaunch",
            vec![
                code_field(0, "kind", "event"),
                field(1, "from"),
                field(2, "fromPos"),
                field(3, "toPos"),
                field(4, "delayTicks"),
            ],
        ),
        (
            "panzerfaustImpact",
            vec![code_field(0, "kind", "event"), field(1, "x"), field(2, "y")],
        ),
    ]
    .into_iter()
    .collect()
}

pub(crate) fn kind_code(kind: &str) -> u8 {
    lookup_code(KIND_CODES, kind)
}

pub(crate) fn state_code(state: &str) -> u8 {
    lookup_code(STATE_CODES, state)
}

pub(crate) fn setup_state_code(setup_state: &str) -> u8 {
    lookup_code(SETUP_STATE_CODES, setup_state)
}

pub(crate) fn order_stage_code(kind: &str) -> u8 {
    lookup_code(ORDER_STAGE_CODES, kind)
}

pub fn ability_code(ability: &str) -> u8 {
    lookup_code(ABILITY_CODES, ability)
}

pub(crate) fn ability_object_kind_code(kind: &str) -> u8 {
    lookup_code(ABILITY_OBJECT_KIND_CODES, kind)
}

pub fn upgrade_code(upgrade: &str) -> u8 {
    lookup_code(UPGRADE_CODES, upgrade)
}

pub(crate) fn weapon_kind_code(weapon_kind: &str) -> u8 {
    lookup_code(WEAPON_KIND_CODES, weapon_kind)
}

pub(crate) fn notice_severity_code(severity: NoticeSeverity) -> u8 {
    match severity {
        NoticeSeverity::Info => 1,
        NoticeSeverity::Warn => 2,
        NoticeSeverity::Alert => 3,
    }
}
