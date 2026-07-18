use serde::ser::{SerializeSeq, Serializer};
use serde::Serialize;
use std::collections::BTreeMap;

use crate::contract_metadata::{
    ability_code, ability_object_kind_code, event_code, kind_code, notice_severity_code,
    order_stage_code, setup_state_code, state_code, upgrade_code, weapon_kind_code,
    COMPACT_SNAPSHOT_VERSION,
};
use crate::messagepack_frame;
use crate::{
    SnapshotEncodeError, SnapshotPayloadDiagnostics, SnapshotPayloadEntityKindDiagnostics,
    SnapshotPayloadSectionDiagnostics,
};
use rts_contract::{
    AbilityCooldownView, AbilityObjectOwnerStateView, AbilityObjectView, AttackReveal,
    DebugPathView, EntityView, Event, OrderPlanMarker, RememberedBuildingView, ScoutPlaneStateView,
    SmokeCloudView, Snapshot, SnapshotNetStatus, TrenchView,
};

/// Serialize one semantic snapshot as a compact JSON text frame payload.
pub(crate) fn serialize_compact_snapshot(snapshot: &Snapshot) -> serde_json::Result<String> {
    serde_json::to_string(&CompactSnapshot(snapshot))
}

pub(crate) fn serialize_compact_snapshot_with_diagnostics(
    snapshot: &Snapshot,
) -> Result<(String, SnapshotPayloadDiagnostics), SnapshotEncodeError> {
    let compact = compact_snapshot_value(snapshot)?;
    let text = serde_json::to_string(&compact)?;
    let diagnostics = json_payload_diagnostics(snapshot, &compact, text.len())?;
    Ok((text, diagnostics))
}

/// Serialize one semantic snapshot as a versioned MessagePack compact binary frame payload.
pub(crate) fn serialize_messagepack_compact_snapshot(
    snapshot: &Snapshot,
) -> Result<Vec<u8>, SnapshotEncodeError> {
    let compact = compact_snapshot_value(snapshot)?;
    messagepack_frame::serialize_compact_snapshot_value(&compact)
}

pub(crate) fn serialize_messagepack_compact_snapshot_with_diagnostics(
    snapshot: &Snapshot,
) -> Result<(Vec<u8>, SnapshotPayloadDiagnostics), SnapshotEncodeError> {
    let compact = compact_snapshot_value(snapshot)?;
    let encoded = messagepack_frame::serialize_compact_snapshot_value_with_entry_bytes(&compact)?;
    let diagnostics =
        payload_diagnostics_from_entry_bytes(snapshot, encoded.bytes.len(), encoded.entry_bytes);
    let bytes = encoded.bytes;
    Ok((bytes, diagnostics))
}

fn compact_snapshot_value(snapshot: &Snapshot) -> serde_json::Result<serde_json::Value> {
    serde_json::to_value(CompactSnapshot(snapshot))
}

struct CompactSnapshot<'a>(&'a Snapshot);

impl Serialize for CompactSnapshot<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;

        let snapshot = self.0;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("t", "snapshot")?;
        map.serialize_entry("v", &COMPACT_SNAPSHOT_VERSION)?;
        map.serialize_entry(
            "s",
            &[
                snapshot.tick,
                snapshot.steel,
                snapshot.oil,
                snapshot.supply_used,
                snapshot.supply_cap,
            ],
        )?;
        if let Some(position) = snapshot.world_combat_position {
            map.serialize_entry("wc", &position)?;
        }
        map.serialize_entry(
            "e",
            &snapshot
                .entities
                .iter()
                .map(CompactEntity)
                .collect::<Vec<_>>(),
        )?;
        if !snapshot.resource_deltas.is_empty() {
            map.serialize_entry(
                "r",
                &snapshot
                    .resource_deltas
                    .iter()
                    .map(|delta| [delta.id, delta.remaining])
                    .collect::<Vec<_>>(),
            )?;
        }
        if !snapshot.smokes.is_empty() {
            map.serialize_entry(
                "sm",
                &snapshot
                    .smokes
                    .iter()
                    .map(CompactSmokeCloud)
                    .collect::<Vec<_>>(),
            )?;
        }
        if !snapshot.ability_objects.is_empty() {
            map.serialize_entry(
                "ao",
                &snapshot
                    .ability_objects
                    .iter()
                    .map(CompactAbilityObject)
                    .collect::<Vec<_>>(),
            )?;
        }
        if !snapshot.trenches.is_empty() {
            map.serialize_entry(
                "tr",
                &snapshot
                    .trenches
                    .iter()
                    .map(CompactTrench)
                    .collect::<Vec<_>>(),
            )?;
        }
        if !snapshot.visible_tiles.is_empty() {
            map.serialize_entry("fg", &encode_visibility_runs(&snapshot.visible_tiles))?;
        }
        if !snapshot.remembered_buildings.is_empty() {
            map.serialize_entry(
                "mb",
                &snapshot
                    .remembered_buildings
                    .iter()
                    .map(CompactRememberedBuilding)
                    .collect::<Vec<_>>(),
            )?;
        }
        if !snapshot.events.is_empty() {
            map.serialize_entry(
                "ev",
                &snapshot.events.iter().map(CompactEvent).collect::<Vec<_>>(),
            )?;
        }
        if !snapshot.player_resources.is_empty() {
            map.serialize_entry(
                "pr",
                &snapshot
                    .player_resources
                    .iter()
                    .map(|p| [p.id, p.steel, p.oil, p.supply_used, p.supply_cap])
                    .collect::<Vec<_>>(),
            )?;
        }
        if !snapshot.upgrades.is_empty() {
            map.serialize_entry(
                "u",
                &snapshot
                    .upgrades
                    .iter()
                    .map(|upgrade| upgrade_code(upgrade))
                    .collect::<Vec<_>>(),
            )?;
        }
        map.serialize_entry("n", &CompactNetStatus(&snapshot.net_status))?;
        map.end()
    }
}

const SECTION_ENTITIES: &str = "entities";
const SECTION_VISIBILITY: &str = "visibility";
const SECTION_RESOURCE_DELTAS: &str = "resourceDeltas";
const SECTION_EVENTS: &str = "events";
const SECTION_SMOKES: &str = "smokes";
const SECTION_ABILITY_OBJECTS: &str = "abilityObjects";
const SECTION_TRENCHES: &str = "trenches";
const SECTION_PLAYER_STATUS: &str = "playerStatus";
const SECTION_NET_STATUS: &str = "netStatus";
const SECTION_OTHER: &str = "other";
const SECTION_ORDER: [&str; 10] = [
    SECTION_ENTITIES,
    SECTION_VISIBILITY,
    SECTION_RESOURCE_DELTAS,
    SECTION_EVENTS,
    SECTION_SMOKES,
    SECTION_ABILITY_OBJECTS,
    SECTION_TRENCHES,
    SECTION_PLAYER_STATUS,
    SECTION_NET_STATUS,
    SECTION_OTHER,
];

fn json_payload_diagnostics(
    snapshot: &Snapshot,
    compact: &serde_json::Value,
    total_bytes: usize,
) -> Result<SnapshotPayloadDiagnostics, SnapshotEncodeError> {
    let mut entry_bytes = Vec::new();
    if let serde_json::Value::Object(map) = compact {
        for (key, value) in map {
            entry_bytes.push((key.as_str(), json_map_entry_len(key, value)?));
        }
    }
    Ok(payload_diagnostics_from_entry_bytes(
        snapshot,
        total_bytes,
        entry_bytes,
    ))
}

fn payload_diagnostics_from_entry_bytes<'a>(
    snapshot: &Snapshot,
    total_bytes: usize,
    entries: impl IntoIterator<Item = (&'a str, usize)>,
) -> SnapshotPayloadDiagnostics {
    let mut counts = section_counts(snapshot);
    let mut bytes: BTreeMap<&'static str, u32> = BTreeMap::new();
    let mut accounted_bytes = 0usize;

    for (key, entry_bytes) in entries {
        let section = section_for_compact_key(key);
        accounted_bytes = accounted_bytes.saturating_add(entry_bytes);
        add_u32(&mut bytes, section, entry_bytes);
    }

    if total_bytes > accounted_bytes {
        add_u32(
            &mut bytes,
            SECTION_OTHER,
            total_bytes.saturating_sub(accounted_bytes),
        );
    }

    counts.entry(SECTION_OTHER).or_insert(1);

    let sections = SECTION_ORDER
        .iter()
        .filter_map(|section| {
            let count = counts.get(section).copied().unwrap_or(0);
            let bytes = bytes.get(section).copied().unwrap_or(0);
            if count == 0 && bytes == 0 {
                None
            } else {
                Some(SnapshotPayloadSectionDiagnostics {
                    section,
                    count,
                    bytes,
                })
            }
        })
        .collect::<Vec<_>>();

    let entity_kinds = entity_kind_diagnostics(snapshot, &sections);

    SnapshotPayloadDiagnostics {
        bytes: saturating_usize_u32(total_bytes),
        sections,
        entity_kinds,
    }
}

fn section_counts(snapshot: &Snapshot) -> BTreeMap<&'static str, u32> {
    let mut counts = BTreeMap::new();
    insert_count(&mut counts, SECTION_ENTITIES, snapshot.entities.len());
    insert_count(
        &mut counts,
        SECTION_VISIBILITY,
        snapshot
            .visible_tiles
            .iter()
            .filter(|tile| **tile != 0)
            .count()
            .saturating_add(snapshot.remembered_buildings.len()),
    );
    insert_count(
        &mut counts,
        SECTION_RESOURCE_DELTAS,
        snapshot.resource_deltas.len(),
    );
    insert_count(&mut counts, SECTION_EVENTS, snapshot.events.len());
    insert_count(&mut counts, SECTION_SMOKES, snapshot.smokes.len());
    insert_count(
        &mut counts,
        SECTION_ABILITY_OBJECTS,
        snapshot.ability_objects.len(),
    );
    insert_count(&mut counts, SECTION_TRENCHES, snapshot.trenches.len());
    insert_count(
        &mut counts,
        SECTION_PLAYER_STATUS,
        1usize
            .saturating_add(snapshot.player_resources.len())
            .saturating_add(snapshot.upgrades.len()),
    );
    counts.insert(SECTION_NET_STATUS, 1);
    counts
}

fn insert_count(counts: &mut BTreeMap<&'static str, u32>, section: &'static str, count: usize) {
    if count > 0 {
        counts.insert(section, saturating_usize_u32(count));
    }
}

fn add_u32(counts: &mut BTreeMap<&'static str, u32>, section: &'static str, value: usize) {
    let value = saturating_usize_u32(value);
    let entry = counts.entry(section).or_insert(0);
    *entry = entry.saturating_add(value);
}

fn section_for_compact_key(key: &str) -> &'static str {
    match key {
        "e" => SECTION_ENTITIES,
        "fg" | "mb" => SECTION_VISIBILITY,
        "r" => SECTION_RESOURCE_DELTAS,
        "ev" => SECTION_EVENTS,
        "sm" => SECTION_SMOKES,
        "ao" => SECTION_ABILITY_OBJECTS,
        "tr" => SECTION_TRENCHES,
        "s" | "pr" | "u" | "wc" => SECTION_PLAYER_STATUS,
        "n" => SECTION_NET_STATUS,
        _ => SECTION_OTHER,
    }
}

fn json_map_entry_len(key: &str, value: &serde_json::Value) -> Result<usize, SnapshotEncodeError> {
    let key_bytes = serde_json::to_string(key)?.len();
    let value_bytes = serde_json::to_string(value)?.len();
    Ok(key_bytes.saturating_add(1).saturating_add(value_bytes))
}

fn entity_kind_diagnostics(
    snapshot: &Snapshot,
    sections: &[SnapshotPayloadSectionDiagnostics],
) -> Vec<SnapshotPayloadEntityKindDiagnostics> {
    let entity_bytes = sections
        .iter()
        .find(|section| section.section == SECTION_ENTITIES)
        .map(|section| section.bytes)
        .unwrap_or(0);
    let total_entities = snapshot.entities.len() as u32;
    if total_entities == 0 {
        return Vec::new();
    }

    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for entity in &snapshot.entities {
        let entry = counts.entry(entity.kind.clone()).or_insert(0);
        *entry = entry.saturating_add(1);
    }

    let mut kinds = counts
        .into_iter()
        .map(|(kind, count)| {
            let approx_bytes = ((entity_bytes as u64).saturating_mul(count as u64)
                / total_entities as u64)
                .min(u32::MAX as u64) as u32;
            SnapshotPayloadEntityKindDiagnostics {
                kind,
                count,
                approx_bytes,
            }
        })
        .collect::<Vec<_>>();
    kinds.sort_by(|a, b| {
        b.approx_bytes
            .cmp(&a.approx_bytes)
            .then_with(|| b.count.cmp(&a.count))
            .then_with(|| a.kind.cmp(&b.kind))
    });
    kinds
}

fn saturating_usize_u32(value: usize) -> u32 {
    value.min(u32::MAX as usize) as u32
}

fn encode_visibility_runs(visible_tiles: &[u8]) -> Vec<u32> {
    let Some((&first, rest)) = visible_tiles.split_first() else {
        return Vec::new();
    };
    let mut runs = vec![u32::from(first != 0)];
    let mut current = first != 0;
    let mut len: u32 = 1;
    for &tile in rest {
        let value = tile != 0;
        if value == current && len < u32::MAX {
            len += 1;
        } else {
            runs.push(len);
            current = value;
            len = 1;
        }
    }
    runs.push(len);
    runs
}

struct CompactRememberedBuilding<'a>(&'a RememberedBuildingView);

impl Serialize for CompactRememberedBuilding<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let building = self.0;
        let mut seq = serializer.serialize_seq(Some(7))?;
        seq.serialize_element(&building.id)?;
        seq.serialize_element(&building.owner)?;
        seq.serialize_element(&kind_code(&building.kind))?;
        seq.serialize_element(&building.x)?;
        seq.serialize_element(&building.y)?;
        seq.serialize_element(&building.footprint)?;
        seq.serialize_element(&building.observed_tick)?;
        seq.end()
    }
}

struct CompactSmokeCloud<'a>(&'a SmokeCloudView);

impl Serialize for CompactSmokeCloud<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let smoke = self.0;
        let mut seq = serializer.serialize_seq(Some(5))?;
        seq.serialize_element(&smoke.id)?;
        seq.serialize_element(&smoke.x)?;
        seq.serialize_element(&smoke.y)?;
        seq.serialize_element(&smoke.radius_tiles)?;
        seq.serialize_element(&smoke.expires_in)?;
        seq.end()
    }
}

struct CompactTrench<'a>(&'a TrenchView);

impl Serialize for CompactTrench<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let trench = self.0;
        let mut seq = serializer.serialize_seq(Some(4))?;
        seq.serialize_element(&trench.id)?;
        seq.serialize_element(&trench.x)?;
        seq.serialize_element(&trench.y)?;
        seq.serialize_element(&trench.radius_tiles)?;
        seq.end()
    }
}

struct CompactAbilityObject<'a>(&'a AbilityObjectView);

impl Serialize for CompactAbilityObject<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let object = self.0;
        let mut seq = serializer.serialize_seq(Some(9))?;
        seq.serialize_element(&object.id)?;
        seq.serialize_element(&object.owner)?;
        seq.serialize_element(&ability_code(&object.ability))?;
        seq.serialize_element(&ability_object_kind_code(&object.kind))?;
        seq.serialize_element(&object.x)?;
        seq.serialize_element(&object.y)?;
        seq.serialize_element(&object.expires_in)?;
        seq.serialize_element(&object.source_caster_id)?;
        seq.serialize_element(
            &object
                .owner_state
                .as_ref()
                .map(CompactAbilityObjectOwnerState),
        )?;
        seq.end()
    }
}

struct CompactAbilityObjectOwnerState<'a>(&'a AbilityObjectOwnerStateView);

impl Serialize for CompactAbilityObjectOwnerState<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let state = self.0;
        let mut seq = serializer.serialize_seq(Some(6))?;
        seq.serialize_element(&state.earliest_return_tick)?;
        seq.serialize_element(&state.hp)?;
        seq.serialize_element(&state.radius)?;
        seq.serialize_element(&state.destroyed_lockout_ticks)?;
        seq.serialize_element(&state.distance_traveled)?;
        seq.serialize_element(&state.ticks_out)?;
        seq.end()
    }
}

struct CompactNetStatus<'a>(&'a SnapshotNetStatus);

impl Serialize for CompactNetStatus<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let status = self.0;
        let flags = u8::from(status.slow_tick) | (u8::from(status.head_of_line) << 1);
        let mut len = 5;
        if status.prediction_version != 0
            || status.last_sim_consumed_client_seq != 0
            || status.last_sim_consumed_client_tick.is_some()
        {
            len = 8;
        }
        let mut seq = serializer.serialize_seq(Some(len))?;
        seq.serialize_element(&(status.server_lag_ms as u32))?;
        seq.serialize_element(&(status.tick_ms as u32))?;
        seq.serialize_element(&(flags as u32))?;
        seq.serialize_element(&status.slow_tick_count)?;
        seq.serialize_element(&status.head_of_line_count)?;
        if len > 5 {
            seq.serialize_element(&status.prediction_version)?;
            seq.serialize_element(&status.last_sim_consumed_client_seq)?;
            seq.serialize_element(&status.last_sim_consumed_client_tick)?;
        }
        seq.end()
    }
}

struct CompactEntity<'a>(&'a EntityView);

impl Serialize for CompactEntity<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let entity = self.0;
        let prod_kind = entity.prod_kind.as_deref().map(kind_code);
        let prod_upgrade = entity.prod_upgrade.as_deref().map(upgrade_code);
        let setup_state = entity.setup_state.as_deref().map(setup_state_code);

        let mut len = 8;
        if entity.facing.is_some() {
            len = 9;
        }
        if entity.weapon_facing.is_some() {
            len = 10;
        }
        if prod_kind.is_some() {
            len = 11;
        }
        if entity.prod_progress.is_some() {
            len = 12;
        }
        if entity.prod_queue.is_some() {
            len = 13;
        }
        if entity.build_progress.is_some() {
            len = 14;
        }
        if entity.latched_node.is_some() {
            len = 15;
        }
        if entity.target_id.is_some() {
            len = 16;
        }
        if setup_state.is_some() {
            len = 17;
        }
        if entity.remaining.is_some() {
            len = 18;
        }
        if entity.rally.is_some() {
            len = 19;
        }
        if entity.oil_used.is_some() {
            len = 20;
        }
        if entity.setup_facing.is_some() {
            len = 21;
        }
        if !entity.order_plan.is_empty() {
            len = 22;
        }
        if entity.charge_cooldown_left.is_some() {
            len = 23;
        }
        if !entity.abilities.is_empty() {
            len = 24;
        }
        if entity.breakthrough_ticks.is_some() {
            len = 25;
        }
        if entity.vision_only {
            len = 26;
        }
        if entity.debug_path.is_some() {
            len = 27;
        }
        if !entity.rally_plan.is_empty() {
            len = 28;
        }
        if prod_upgrade.is_some() {
            len = 29;
        }
        if entity.build_active {
            len = 30;
        }
        if entity.deconstruct_progress.is_some() {
            len = 31;
        }
        if entity.weapon_range_tiles.is_some() {
            len = 32;
        }
        if entity.occupied_trench_id.is_some() {
            len = 33;
        }
        if entity.scout_plane.is_some() {
            len = 34;
        }
        if entity.prod_scout_plane_queued {
            len = 35;
        }
        if entity.panzerfaust_loaded.is_some() {
            len = 36;
        }
        if !entity.prod_repeat_kinds.is_empty() {
            len = 37;
        }
        if entity.prod_waiting {
            len = 38;
        }
        if entity.breakthrough_aura_ticks.is_some() {
            len = 39;
        }
        if entity.extractor_active.is_some() {
            len = 40;
        }

        let mut seq = serializer.serialize_seq(Some(len))?;
        seq.serialize_element(&entity.id)?;
        seq.serialize_element(&entity.owner)?;
        seq.serialize_element(&kind_code(&entity.kind))?;
        seq.serialize_element(&entity.x)?;
        seq.serialize_element(&entity.y)?;
        seq.serialize_element(&entity.hp)?;
        seq.serialize_element(&entity.max_hp)?;
        seq.serialize_element(&state_code(&entity.state))?;
        if len > 8 {
            seq.serialize_element(&entity.facing)?;
        }
        if len > 9 {
            seq.serialize_element(&entity.weapon_facing)?;
        }
        if len > 10 {
            seq.serialize_element(&prod_kind)?;
        }
        if len > 11 {
            seq.serialize_element(&entity.prod_progress)?;
        }
        if len > 12 {
            seq.serialize_element(&entity.prod_queue)?;
        }
        if len > 13 {
            seq.serialize_element(&entity.build_progress)?;
        }
        if len > 14 {
            seq.serialize_element(&entity.latched_node)?;
        }
        if len > 15 {
            seq.serialize_element(&entity.target_id)?;
        }
        if len > 16 {
            seq.serialize_element(&setup_state)?;
        }
        if len > 17 {
            seq.serialize_element(&entity.remaining)?;
        }
        if len > 18 {
            seq.serialize_element(&entity.rally)?;
        }
        if len > 19 {
            seq.serialize_element(&entity.oil_used)?;
        }
        if len > 20 {
            seq.serialize_element(&entity.setup_facing)?;
        }
        if len > 21 {
            seq.serialize_element(
                &entity
                    .order_plan
                    .iter()
                    .map(CompactOrderPlanMarker)
                    .collect::<Vec<_>>(),
            )?;
        }
        if len > 22 {
            seq.serialize_element(&entity.charge_cooldown_left)?;
        }
        if len > 23 {
            seq.serialize_element(
                &entity
                    .abilities
                    .iter()
                    .map(CompactAbilityCooldown)
                    .collect::<Vec<_>>(),
            )?;
        }
        if len > 24 {
            seq.serialize_element(&entity.breakthrough_ticks)?;
        }
        if len > 25 {
            seq.serialize_element(&entity.vision_only)?;
        }
        if len > 26 {
            seq.serialize_element(&entity.debug_path.as_ref().map(CompactDebugPath))?;
        }
        if len > 27 {
            seq.serialize_element(
                &entity
                    .rally_plan
                    .iter()
                    .map(CompactOrderPlanMarker)
                    .collect::<Vec<_>>(),
            )?;
        }
        if len > 28 {
            seq.serialize_element(&prod_upgrade)?;
        }
        if len > 29 {
            seq.serialize_element(&entity.build_active)?;
        }
        if len > 30 {
            seq.serialize_element(&entity.deconstruct_progress)?;
        }
        if len > 31 {
            seq.serialize_element(&entity.weapon_range_tiles)?;
        }
        if len > 32 {
            seq.serialize_element(&entity.occupied_trench_id)?;
        }
        if len > 33 {
            seq.serialize_element(&entity.scout_plane.as_ref().map(CompactScoutPlaneState))?;
        }
        if len > 34 {
            seq.serialize_element(&entity.prod_scout_plane_queued)?;
        }
        if len > 35 {
            seq.serialize_element(&entity.panzerfaust_loaded)?;
        }
        if len > 36 {
            seq.serialize_element(
                &entity
                    .prod_repeat_kinds
                    .iter()
                    .map(|kind| kind_code(kind))
                    .collect::<Vec<_>>(),
            )?;
        }
        if len > 37 {
            seq.serialize_element(&entity.prod_waiting)?;
        }
        if len > 38 {
            seq.serialize_element(&entity.breakthrough_aura_ticks)?;
        }
        if len > 39 {
            seq.serialize_element(&entity.extractor_active)?;
        }
        seq.end()
    }
}

struct CompactScoutPlaneState<'a>(&'a ScoutPlaneStateView);

impl Serialize for CompactScoutPlaneState<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let state = self.0;
        let len = if state.source_command_car.is_some() {
            2
        } else {
            1
        };
        let mut seq = serializer.serialize_seq(Some(len))?;
        seq.serialize_element(&state.orbit_center)?;
        if len > 1 {
            seq.serialize_element(&state.source_command_car)?;
        }
        seq.end()
    }
}

struct CompactAbilityCooldown<'a>(&'a AbilityCooldownView);

impl Serialize for CompactAbilityCooldown<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let ability = self.0;
        let mut len = 2;
        if ability.remaining_uses.is_some() {
            len = 3;
        }
        if ability.autocast_enabled.is_some() {
            len = 4;
        }
        if ability.active_object_id.is_some() {
            len = 5;
        }
        if ability.available_tick.is_some() {
            len = 6;
        }
        if ability.lockout_until_tick.is_some() {
            len = 7;
        }
        if ability.expires_in.is_some() {
            len = 8;
        }
        let mut seq = serializer.serialize_seq(Some(len))?;
        seq.serialize_element(&ability_code(&ability.ability))?;
        seq.serialize_element(&ability.cooldown_left)?;
        if len > 2 {
            seq.serialize_element(&ability.remaining_uses)?;
        }
        if len > 3 {
            seq.serialize_element(&ability.autocast_enabled)?;
        }
        if len > 4 {
            seq.serialize_element(&ability.active_object_id)?;
        }
        if len > 5 {
            seq.serialize_element(&ability.available_tick)?;
        }
        if len > 6 {
            seq.serialize_element(&ability.lockout_until_tick)?;
        }
        if len > 7 {
            seq.serialize_element(&ability.expires_in)?;
        }
        seq.end()
    }
}

struct CompactOrderPlanMarker<'a>(&'a OrderPlanMarker);

impl Serialize for CompactOrderPlanMarker<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let marker = self.0;
        let mut seq = serializer.serialize_seq(Some(3))?;
        seq.serialize_element(&order_stage_code(&marker.kind))?;
        seq.serialize_element(&marker.x)?;
        seq.serialize_element(&marker.y)?;
        seq.end()
    }
}

struct CompactDebugPath<'a>(&'a DebugPathView);

impl Serialize for CompactDebugPath<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let path = self.0;
        let mut seq = serializer.serialize_seq(Some(6))?;
        seq.serialize_element(
            &path
                .waypoints
                .iter()
                .map(|p| [p.x, p.y])
                .collect::<Vec<_>>(),
        )?;
        seq.serialize_element(&path.goal.map(|p| [p.x, p.y]))?;
        seq.serialize_element(&path.last_repath_tick)?;
        seq.serialize_element(&path.stuck_ticks)?;
        seq.serialize_element(&path.static_blocked_ticks)?;
        seq.serialize_element(&path.total_waypoints)?;
        seq.end()
    }
}

struct CompactEvent<'a>(&'a Event);

impl Serialize for CompactEvent<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            Event::Attack {
                from,
                to,
                reveal,
                to_pos,
                weapon_kind,
            } => {
                let len = if weapon_kind.is_some() {
                    6
                } else if to_pos.is_some() {
                    5
                } else if reveal.is_some() {
                    4
                } else {
                    3
                };
                let mut seq = serializer.serialize_seq(Some(len))?;
                seq.serialize_element(&event_code("attack"))?;
                seq.serialize_element(from)?;
                seq.serialize_element(to)?;
                if len > 3 {
                    seq.serialize_element(&reveal.as_ref().map(CompactAttackReveal))?;
                }
                if len > 4 {
                    seq.serialize_element(to_pos)?;
                }
                if len > 5 {
                    seq.serialize_element(&weapon_kind.as_deref().map(weapon_kind_code))?;
                }
                seq.end()
            }
            Event::Overpenetration { to } => {
                let mut seq = serializer.serialize_seq(Some(2))?;
                seq.serialize_element(&event_code("overpenetration"))?;
                seq.serialize_element(to)?;
                seq.end()
            }
            Event::Miss { to } => {
                let mut seq = serializer.serialize_seq(Some(2))?;
                seq.serialize_element(&event_code("miss"))?;
                seq.serialize_element(to)?;
                seq.end()
            }
            Event::Death { id, x, y, kind } => {
                let mut seq = serializer.serialize_seq(Some(5))?;
                seq.serialize_element(&event_code("death"))?;
                seq.serialize_element(id)?;
                seq.serialize_element(x)?;
                seq.serialize_element(y)?;
                seq.serialize_element(&kind_code(kind))?;
                seq.end()
            }
            Event::Build { id, kind } => {
                let mut seq = serializer.serialize_seq(Some(3))?;
                seq.serialize_element(&event_code("build"))?;
                seq.serialize_element(id)?;
                seq.serialize_element(&kind_code(kind))?;
                seq.end()
            }
            Event::SmokeLaunch {
                from_x,
                from_y,
                to_x,
                to_y,
                delay_ticks,
            } => {
                let mut seq = serializer.serialize_seq(Some(4))?;
                seq.serialize_element(&event_code("smokeLaunch"))?;
                seq.serialize_element(&[from_x, from_y])?;
                seq.serialize_element(&[to_x, to_y])?;
                seq.serialize_element(delay_ticks)?;
                seq.end()
            }
            Event::MortarLaunch {
                from,
                from_x,
                from_y,
                to_x,
                to_y,
                radius_tiles,
                delay_ticks,
            } => {
                let mut seq = serializer.serialize_seq(Some(6))?;
                seq.serialize_element(&event_code("mortarLaunch"))?;
                seq.serialize_element(from)?;
                seq.serialize_element(&[from_x, from_y])?;
                seq.serialize_element(&[to_x, to_y])?;
                seq.serialize_element(radius_tiles)?;
                seq.serialize_element(delay_ticks)?;
                seq.end()
            }
            Event::MortarImpact {
                from,
                x,
                y,
                radius_tiles,
                reveal,
            } => {
                let len = if reveal.is_some() {
                    6
                } else if from.is_some() {
                    5
                } else {
                    4
                };
                let mut seq = serializer.serialize_seq(Some(len))?;
                seq.serialize_element(&event_code("mortarImpact"))?;
                seq.serialize_element(x)?;
                seq.serialize_element(y)?;
                seq.serialize_element(radius_tiles)?;
                if len > 4 {
                    seq.serialize_element(from)?;
                }
                if len > 5 {
                    seq.serialize_element(&reveal.as_ref().map(CompactAttackReveal))?;
                }
                seq.end()
            }
            Event::ArtilleryTarget {
                from,
                x,
                y,
                radius_tiles,
                delay_ticks,
            } => {
                let mut seq = serializer.serialize_seq(Some(5))?;
                seq.serialize_element(&event_code("artilleryTarget"))?;
                seq.serialize_element(from)?;
                seq.serialize_element(&[x, y])?;
                seq.serialize_element(radius_tiles)?;
                seq.serialize_element(delay_ticks)?;
                seq.end()
            }
            Event::ArtilleryFiring {
                owner,
                x,
                y,
                facing,
            } => {
                let mut seq = serializer.serialize_seq(Some(5))?;
                seq.serialize_element(&event_code("artilleryFiring"))?;
                seq.serialize_element(owner)?;
                seq.serialize_element(x)?;
                seq.serialize_element(y)?;
                seq.serialize_element(facing)?;
                seq.end()
            }
            Event::ArtilleryImpact { x, y, radius_tiles } => {
                let mut seq = serializer.serialize_seq(Some(4))?;
                seq.serialize_element(&event_code("artilleryImpact"))?;
                seq.serialize_element(x)?;
                seq.serialize_element(y)?;
                seq.serialize_element(radius_tiles)?;
                seq.end()
            }
            Event::PanzerfaustLaunch {
                from,
                from_x,
                from_y,
                to_x,
                to_y,
                delay_ticks,
            } => {
                let mut seq = serializer.serialize_seq(Some(5))?;
                seq.serialize_element(&event_code("panzerfaustLaunch"))?;
                seq.serialize_element(from)?;
                seq.serialize_element(&[from_x, from_y])?;
                seq.serialize_element(&[to_x, to_y])?;
                seq.serialize_element(delay_ticks)?;
                seq.end()
            }
            Event::PanzerfaustImpact { x, y } => {
                let mut seq = serializer.serialize_seq(Some(3))?;
                seq.serialize_element(&event_code("panzerfaustImpact"))?;
                seq.serialize_element(x)?;
                seq.serialize_element(y)?;
                seq.end()
            }
            Event::Notice {
                msg,
                x,
                y,
                severity,
            } => {
                let has_position = x.is_some() && y.is_some();
                let len = if has_position {
                    5
                } else if !severity.is_info() {
                    3
                } else {
                    2
                };
                let mut seq = serializer.serialize_seq(Some(len))?;
                seq.serialize_element(&event_code("notice"))?;
                seq.serialize_element(msg)?;
                if len > 2 {
                    seq.serialize_element(&notice_severity_code(*severity))?;
                }
                if len > 3 {
                    seq.serialize_element(x)?;
                    seq.serialize_element(y)?;
                }
                seq.end()
            }
        }
    }
}

struct CompactAttackReveal<'a>(&'a AttackReveal);

impl Serialize for CompactAttackReveal<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let reveal = self.0;
        let setup_state = reveal.setup_state.as_deref().map(setup_state_code);

        let mut len = 4;
        if reveal.facing.is_some() {
            len = 5;
        }
        if reveal.weapon_facing.is_some() {
            len = 6;
        }
        if setup_state.is_some() {
            len = 7;
        }

        let mut seq = serializer.serialize_seq(Some(len))?;
        seq.serialize_element(&reveal.owner)?;
        seq.serialize_element(&kind_code(&reveal.kind))?;
        seq.serialize_element(&reveal.x)?;
        seq.serialize_element(&reveal.y)?;
        if len > 4 {
            seq.serialize_element(&reveal.facing)?;
        }
        if len > 5 {
            seq.serialize_element(&reveal.weapon_facing)?;
        }
        if len > 6 {
            seq.serialize_element(&setup_state)?;
        }
        seq.end()
    }
}
