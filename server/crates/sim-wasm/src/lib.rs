//! Owner-safe browser prediction facade.
//!
//! This crate intentionally does not expose the authoritative [`rts_sim::game::Game`] world. The
//! browser imports only an [`OwnedPredictionBaseline`]: owned entities and owner economy fields
//! plus visible non-authoritative obstacles with no enemy ids, orders, target ids, production, or
//! economy state. Phase 3 predicts the supported movement/order surface and reports unsupported
//! systems explicitly so harness diffs can distinguish "unknown" from "divergent".

use std::collections::{BTreeMap, VecDeque};

use rts_contract::{
    EntityView, MapInfo, OrderPlanMarker, Snapshot, SnapshotNetStatus, StartPayload,
};
use rts_protocol::Command;
use rts_rules::{balance, EntityKind};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

const CORRECTION_EPS_PX: f32 = 0.01;
const UNSUPPORTED_FIELDS: &[&str] = &[
    "combat",
    "economyGathering",
    "production",
    "construction",
    "fogReconstruction",
    "enemyAuthoritativeState",
    "resourceNodeState",
    "abilities",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OwnedPredictionBaseline {
    pub tick: u32,
    pub player_id: u32,
    pub steel: Option<u32>,
    pub oil: Option<u32>,
    pub supply_used: Option<u32>,
    pub supply_cap: Option<u32>,
    #[serde(default)]
    pub owned_entities: Vec<OwnedEntityBaseline>,
    #[serde(default)]
    pub visible_obstacles: Vec<VisibleObstacle>,
}

impl OwnedPredictionBaseline {
    pub fn from_snapshot(player_id: u32, snapshot: &Snapshot) -> Self {
        let mut owned_entities = Vec::new();
        let mut visible_obstacles = Vec::new();
        for entity in &snapshot.entities {
            if entity.owner == player_id {
                owned_entities.push(OwnedEntityBaseline::from_view(entity));
            } else if entity.owner != 0 {
                visible_obstacles.push(VisibleObstacle::from_view(entity));
            }
        }
        Self {
            tick: snapshot.tick,
            player_id,
            steel: Some(snapshot.steel),
            oil: Some(snapshot.oil),
            supply_used: Some(snapshot.supply_used),
            supply_cap: Some(snapshot.supply_cap),
            owned_entities,
            visible_obstacles,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OwnedEntityBaseline {
    pub id: u32,
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub hp: u32,
    pub max_hp: u32,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub facing: Option<f32>,
    #[serde(default)]
    pub weapon_facing: Option<f32>,
    #[serde(default)]
    pub order_plan: Vec<OrderPlanMarker>,
}

impl OwnedEntityBaseline {
    fn from_view(entity: &EntityView) -> Self {
        Self {
            id: entity.id,
            kind: entity.kind.clone(),
            x: entity.x,
            y: entity.y,
            hp: entity.hp,
            max_hp: entity.max_hp,
            state: Some(entity.state.clone()),
            facing: entity.facing,
            weapon_facing: entity.weapon_facing,
            order_plan: owner_safe_order_plan(&entity.order_plan),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VisibleObstacle {
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
}

impl VisibleObstacle {
    fn from_view(entity: &EntityView) -> Self {
        Self {
            kind: entity.kind.clone(),
            x: entity.x,
            y: entity.y,
            radius: entity
                .kind
                .parse::<EntityKind>()
                .ok()
                .and_then(|kind| balance::unit_stats(kind).map(|stats| stats.radius))
                .unwrap_or(0.0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalLaneSummary {
    pub tick: u32,
    pub player_id: u32,
    pub owned_entities: Vec<PredictedEntitySummary>,
    pub pending_commands: usize,
    pub pending_client_seqs: Vec<u32>,
    pub correction_magnitude: f32,
    pub unsupported_fields: Vec<String>,
    pub disabled_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PredictedEntitySummary {
    pub id: u32,
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub state: String,
    #[serde(default)]
    pub order_plan: Vec<OrderPlanMarker>,
    #[serde(default)]
    pub queued_order_stages: Vec<OrderPlanMarker>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PredictorDiagnostics {
    pub tick: u32,
    pub owned_entity_count: usize,
    pub visible_obstacle_count: usize,
    pub pending_commands: usize,
    pub pending_client_seqs: Vec<u32>,
    pub pending_command_kinds: Vec<String>,
    pub correction_magnitude: f32,
    pub unsupported_fields: Vec<String>,
    pub disabled_reasons: Vec<String>,
}

#[derive(Debug, Clone)]
struct EntityState {
    id: u32,
    kind: EntityKind,
    x: f32,
    y: f32,
    hp: u32,
    max_hp: u32,
    state: String,
    facing: Option<f32>,
    weapon_facing: Option<f32>,
    active_order: Option<MoveOrder>,
    queued_orders: VecDeque<MoveOrder>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MoveOrder {
    kind: MoveOrderKind,
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum MoveOrderKind {
    Move,
    AttackMove,
    HoldPosition,
}

#[derive(Debug, Clone)]
struct PendingCommand {
    client_seq: u32,
    command: Command,
}

#[derive(Debug, Clone)]
struct CorePredictor {
    player_id: u32,
    tick: u32,
    map: MapInfo,
    steel: Option<u32>,
    oil: Option<u32>,
    supply_used: Option<u32>,
    supply_cap: Option<u32>,
    owned: BTreeMap<u32, EntityState>,
    visible_obstacles: Vec<VisibleObstacle>,
    pending: VecDeque<PendingCommand>,
    correction_magnitude: f32,
    disabled_reasons: Vec<String>,
}

impl CorePredictor {
    fn from_start_payload(start: StartPayload, player_id: u32) -> Self {
        Self {
            player_id,
            tick: start.tick,
            map: start.map,
            steel: None,
            oil: None,
            supply_used: None,
            supply_cap: None,
            owned: BTreeMap::new(),
            visible_obstacles: Vec::new(),
            pending: VecDeque::new(),
            correction_magnitude: 0.0,
            disabled_reasons: vec!["baselineNotImported".to_string()],
        }
    }

    fn import_baseline(&mut self, baseline: OwnedPredictionBaseline) -> Result<(), String> {
        if baseline.player_id != self.player_id {
            return Err(format!(
                "baseline player {} does not match predictor player {}",
                baseline.player_id, self.player_id
            ));
        }
        self.correction_magnitude = correction_magnitude(&self.owned, &baseline.owned_entities);
        self.tick = baseline.tick;
        self.steel = baseline.steel;
        self.oil = baseline.oil;
        self.supply_used = baseline.supply_used;
        self.supply_cap = baseline.supply_cap;
        self.owned = baseline
            .owned_entities
            .into_iter()
            .map(EntityState::try_from_baseline)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|entity| (entity.id, entity))
            .collect();
        self.visible_obstacles = baseline.visible_obstacles;
        self.pending.clear();
        self.disabled_reasons.clear();
        Ok(())
    }

    fn enqueue_command(&mut self, client_seq: u32, command: Command) {
        self.apply_command(&command);
        self.pending.push_back(PendingCommand {
            client_seq,
            command,
        });
    }

    fn advance_ticks(&mut self, ticks: u32) {
        for _ in 0..ticks {
            self.tick = self.tick.wrapping_add(1);
            for entity in self.owned.values_mut() {
                entity.advance_one_tick();
            }
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            tick: self.tick,
            world_combat_active: false,
            steel: self.steel.unwrap_or(0),
            oil: self.oil.unwrap_or(0),
            supply_used: self.supply_used.unwrap_or(0),
            supply_cap: self.supply_cap.unwrap_or(0),
            entities: self
                .owned
                .values()
                .map(|entity| entity.to_view(self.player_id))
                .collect(),
            resource_deltas: Vec::new(),
            smokes: Vec::new(),
            ability_objects: Vec::new(),
            trenches: Vec::new(),
            visible_tiles: Vec::new(),
            remembered_buildings: Vec::new(),
            events: Vec::new(),
            upgrades: Vec::new(),
            player_resources: Vec::new(),
            net_status: SnapshotNetStatus::default(),
        }
    }

    fn diagnostics(&self) -> PredictorDiagnostics {
        PredictorDiagnostics {
            tick: self.tick,
            owned_entity_count: self.owned.len(),
            visible_obstacle_count: self.visible_obstacles.len(),
            pending_commands: self.pending.len(),
            pending_client_seqs: self
                .pending
                .iter()
                .map(|pending| pending.client_seq)
                .collect(),
            pending_command_kinds: self
                .pending
                .iter()
                .map(|pending| command_kind(&pending.command).to_string())
                .collect(),
            correction_magnitude: self.correction_magnitude,
            unsupported_fields: unsupported_fields(),
            disabled_reasons: self.disabled_reasons.clone(),
        }
    }

    fn local_lane_summary(&self) -> LocalLaneSummary {
        LocalLaneSummary {
            tick: self.tick,
            player_id: self.player_id,
            owned_entities: self
                .owned
                .values()
                .map(EntityState::to_summary)
                .collect::<Vec<_>>(),
            pending_commands: self.pending.len(),
            pending_client_seqs: self
                .pending
                .iter()
                .map(|pending| pending.client_seq)
                .collect(),
            correction_magnitude: self.correction_magnitude,
            unsupported_fields: unsupported_fields(),
            disabled_reasons: self.disabled_reasons.clone(),
        }
    }

    fn apply_command(&mut self, command: &Command) {
        match command {
            Command::Move {
                units,
                x,
                y,
                queued,
            } => self.apply_move(units, *x, *y, *queued, MoveOrderKind::Move),
            Command::AttackMove {
                units,
                x,
                y,
                queued,
            } => self.apply_move(units, *x, *y, *queued, MoveOrderKind::AttackMove),
            Command::Stop { units } => {
                for id in units {
                    if let Some(entity) = self.owned.get_mut(id) {
                        entity.active_order = None;
                        entity.queued_orders.clear();
                        entity.state = "idle".to_string();
                    }
                }
            }
            Command::HoldPosition { units, queued } => self.apply_hold_position(units, *queued),
            Command::Build { .. } | Command::Deconstruct { .. } => {
                self.note_disabled("buildPredictionUnsupported");
            }
            Command::Attack { .. }
            | Command::SetupAntiTankGuns { .. }
            | Command::TearDownAntiTankGuns { .. }
            | Command::Charge { .. }
            | Command::UseAbility { .. }
            | Command::RecastAbility { .. }
            | Command::SetAutocast { .. }
            | Command::Gather { .. }
            | Command::Train { .. }
            | Command::AdjustProductionRepeat { .. }
            | Command::Research { .. }
            | Command::Cancel { .. }
            | Command::SetRally { .. } => {
                self.note_disabled("commandUnsupported");
            }
        }
    }

    fn apply_move(&mut self, units: &[u32], x: f32, y: f32, queued: bool, kind: MoveOrderKind) {
        if !x.is_finite() || !y.is_finite() {
            self.note_disabled("invalidMoveTarget");
            return;
        }
        let target_x = x.clamp(0.0, self.world_max_px());
        let target_y = y.clamp(0.0, self.world_max_px());
        for id in units {
            if let Some(entity) = self.owned.get_mut(id) {
                let order = MoveOrder {
                    kind,
                    x: target_x,
                    y: target_y,
                };
                if queued {
                    if !entity.queue_has_terminal_hold() {
                        entity.queued_orders.push_back(order);
                    }
                } else {
                    entity.active_order = Some(order);
                    entity.queued_orders.clear();
                    entity.state = state_for_order(kind).to_string();
                }
            }
        }
    }

    fn apply_hold_position(&mut self, units: &[u32], queued: bool) {
        for id in units {
            let Some(entity) = self.owned.get_mut(id) else {
                continue;
            };
            if queued {
                if entity.queue_has_terminal_hold() {
                    continue;
                }
                let (x, y) = entity
                    .queued_orders
                    .back()
                    .or(entity.active_order.as_ref())
                    .map(|order| (order.x, order.y))
                    .unwrap_or((entity.x, entity.y));
                entity.queued_orders.push_back(MoveOrder {
                    kind: MoveOrderKind::HoldPosition,
                    x,
                    y,
                });
            } else {
                entity.active_order = Some(MoveOrder {
                    kind: MoveOrderKind::HoldPosition,
                    x: entity.x,
                    y: entity.y,
                });
                entity.queued_orders.clear();
                entity.state = "idle".to_string();
            }
        }
    }

    fn world_max_px(&self) -> f32 {
        self.map.width.max(self.map.height) as f32 * self.map.tile_size as f32
    }

    fn note_disabled(&mut self, reason: &str) {
        if !self
            .disabled_reasons
            .iter()
            .any(|existing| existing == reason)
        {
            self.disabled_reasons.push(reason.to_string());
        }
    }
}

impl EntityState {
    fn try_from_baseline(baseline: OwnedEntityBaseline) -> Result<Self, String> {
        let kind = baseline
            .kind
            .parse::<EntityKind>()
            .map_err(|_| format!("unsupported entity kind {:?}", baseline.kind))?;
        let mut active_order = None;
        let mut queued_orders = VecDeque::new();
        for marker in owner_safe_order_plan(&baseline.order_plan) {
            let order = MoveOrder {
                kind: match marker.kind.as_str() {
                    "attackMove" => MoveOrderKind::AttackMove,
                    "holdPosition" => MoveOrderKind::HoldPosition,
                    _ => MoveOrderKind::Move,
                },
                x: marker.x,
                y: marker.y,
            };
            // Authoritative active HoldPosition is intentionally absent from orderPlan, so every
            // hold marker arriving in a baseline is a queued terminal stage.
            if active_order.is_none() && order.kind != MoveOrderKind::HoldPosition {
                active_order = Some(order);
            } else {
                queued_orders.push_back(order);
            }
        }
        Ok(Self {
            id: baseline.id,
            kind,
            x: baseline.x,
            y: baseline.y,
            hp: baseline.hp,
            max_hp: baseline.max_hp,
            state: baseline.state.unwrap_or_else(|| {
                active_order
                    .map(|order| state_for_order(order.kind).to_string())
                    .unwrap_or_else(|| "idle".to_string())
            }),
            facing: baseline.facing,
            weapon_facing: baseline.weapon_facing,
            active_order,
            queued_orders,
        })
    }

    fn advance_one_tick(&mut self) {
        let Some(order) = self.active_order else {
            if self.queued_orders.is_empty() {
                self.state = "idle".to_string();
            } else {
                self.finish_order();
            }
            return;
        };
        if order.kind == MoveOrderKind::HoldPosition {
            if self.queued_orders.is_empty() {
                self.state = "idle".to_string();
            } else {
                self.finish_order();
            }
            return;
        }
        let dx = order.x - self.x;
        let dy = order.y - self.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if !dist.is_finite() || dist <= CORRECTION_EPS_PX {
            self.finish_order();
            return;
        }
        let step = balance::unit_stats(self.kind)
            .map(|stats| stats.speed)
            .unwrap_or(0.0)
            .min(dist);
        if step <= CORRECTION_EPS_PX {
            self.finish_order();
            return;
        }
        self.x += dx / dist * step;
        self.y += dy / dist * step;
        self.facing = Some(dy.atan2(dx));
        self.state = state_for_order(order.kind).to_string();
        if (dist - step) <= CORRECTION_EPS_PX {
            self.x = order.x;
            self.y = order.y;
            self.finish_order();
        }
    }

    fn finish_order(&mut self) {
        self.active_order = self.queued_orders.pop_front();
        self.state = self
            .active_order
            .map(|order| state_for_order(order.kind).to_string())
            .unwrap_or_else(|| "idle".to_string());
    }

    fn to_view(&self, player_id: u32) -> EntityView {
        let mut view = EntityView::new(
            self.id,
            player_id,
            self.kind.stable_id(),
            self.x,
            self.y,
            self.hp,
            self.max_hp,
            &self.state,
        );
        view.facing = self.facing;
        view.weapon_facing = self.weapon_facing;
        view.order_plan = self.order_plan();
        view
    }

    fn to_summary(&self) -> PredictedEntitySummary {
        PredictedEntitySummary {
            id: self.id,
            kind: self.kind.stable_id().to_string(),
            x: self.x,
            y: self.y,
            state: self.state.clone(),
            order_plan: self.active_order_marker().into_iter().collect(),
            queued_order_stages: self
                .queued_orders
                .iter()
                .map(|order| order.to_marker())
                .collect(),
        }
    }

    fn order_plan(&self) -> Vec<OrderPlanMarker> {
        self.active_order_marker()
            .into_iter()
            .chain(self.queued_orders.iter().map(|order| order.to_marker()))
            .collect()
    }

    fn active_order_marker(&self) -> Option<OrderPlanMarker> {
        self.active_order
            .filter(|order| order.kind != MoveOrderKind::HoldPosition)
            .map(MoveOrder::to_marker)
    }

    fn queue_has_terminal_hold(&self) -> bool {
        self.queued_orders
            .iter()
            .any(|order| order.kind == MoveOrderKind::HoldPosition)
    }
}

impl MoveOrder {
    fn to_marker(self) -> OrderPlanMarker {
        OrderPlanMarker {
            kind: match self.kind {
                MoveOrderKind::Move => "move".to_string(),
                MoveOrderKind::AttackMove => "attackMove".to_string(),
                MoveOrderKind::HoldPosition => "holdPosition".to_string(),
            },
            x: self.x,
            y: self.y,
        }
    }
}

#[wasm_bindgen]
pub struct WasmPredictor {
    core: CorePredictor,
}

#[wasm_bindgen]
impl WasmPredictor {
    #[wasm_bindgen(js_name = fromStartJson)]
    pub fn from_start_json(start_json: &str, player_id: u32) -> Result<WasmPredictor, JsValue> {
        let start = serde_json::from_str::<StartPayload>(start_json).map_err(js_error)?;
        Ok(WasmPredictor {
            core: CorePredictor::from_start_payload(start, player_id),
        })
    }

    #[wasm_bindgen(js_name = baselineFromSnapshotJson)]
    pub fn baseline_from_snapshot_json(
        snapshot_json: &str,
        player_id: u32,
    ) -> Result<String, JsValue> {
        let snapshot = serde_json::from_str::<Snapshot>(snapshot_json).map_err(js_error)?;
        let baseline = OwnedPredictionBaseline::from_snapshot(player_id, &snapshot);
        serde_json::to_string(&baseline).map_err(js_error)
    }

    #[wasm_bindgen(js_name = importBaselineJson)]
    pub fn import_baseline_json(&mut self, baseline_json: &str) -> Result<(), JsValue> {
        let baseline =
            serde_json::from_str::<OwnedPredictionBaseline>(baseline_json).map_err(js_error)?;
        self.core.import_baseline(baseline).map_err(js_error)
    }

    #[wasm_bindgen(js_name = enqueueCommandJson)]
    pub fn enqueue_command_json(
        &mut self,
        client_seq: u32,
        command_json: &str,
    ) -> Result<(), JsValue> {
        let command = serde_json::from_str::<Command>(command_json).map_err(js_error)?;
        self.core.enqueue_command(client_seq, command);
        Ok(())
    }

    #[wasm_bindgen(js_name = advanceTicks)]
    pub fn advance_ticks(&mut self, ticks: u32) {
        self.core.advance_ticks(ticks);
    }

    #[wasm_bindgen(js_name = renderSnapshotJson)]
    pub fn render_snapshot_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.core.snapshot()).map_err(js_error)
    }

    #[wasm_bindgen(js_name = diagnosticsJson)]
    pub fn diagnostics_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.core.diagnostics()).map_err(js_error)
    }

    #[wasm_bindgen(js_name = localLaneSummaryJson)]
    pub fn local_lane_summary_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.core.local_lane_summary()).map_err(js_error)
    }
}

pub fn predictor_from_start_payload(start: StartPayload, player_id: u32) -> NativePredictor {
    NativePredictor {
        core: CorePredictor::from_start_payload(start, player_id),
    }
}

pub fn baseline_from_snapshot(player_id: u32, snapshot: &Snapshot) -> OwnedPredictionBaseline {
    OwnedPredictionBaseline::from_snapshot(player_id, snapshot)
}

#[derive(Debug, Clone)]
pub struct NativePredictor {
    core: CorePredictor,
}

impl NativePredictor {
    pub fn import_baseline(&mut self, baseline: OwnedPredictionBaseline) -> Result<(), String> {
        self.core.import_baseline(baseline)
    }

    pub fn enqueue_command(&mut self, client_seq: u32, command: Command) {
        self.core.enqueue_command(client_seq, command);
    }

    pub fn advance_ticks(&mut self, ticks: u32) {
        self.core.advance_ticks(ticks);
    }

    pub fn render_snapshot(&self) -> Snapshot {
        self.core.snapshot()
    }

    pub fn diagnostics(&self) -> PredictorDiagnostics {
        self.core.diagnostics()
    }

    pub fn local_lane_summary(&self) -> LocalLaneSummary {
        self.core.local_lane_summary()
    }
}

fn correction_magnitude(
    current: &BTreeMap<u32, EntityState>,
    baseline: &[OwnedEntityBaseline],
) -> f32 {
    baseline
        .iter()
        .filter_map(|entity| {
            current.get(&entity.id).map(|old| {
                let dx = old.x - entity.x;
                let dy = old.y - entity.y;
                (dx * dx + dy * dy).sqrt()
            })
        })
        .fold(0.0, f32::max)
}

fn owner_safe_order_plan(markers: &[OrderPlanMarker]) -> Vec<OrderPlanMarker> {
    markers
        .iter()
        .filter(|marker| matches!(marker.kind.as_str(), "move" | "attackMove" | "holdPosition"))
        .cloned()
        .collect()
}

fn unsupported_fields() -> Vec<String> {
    UNSUPPORTED_FIELDS
        .iter()
        .map(|field| (*field).to_string())
        .collect()
}

fn command_kind(command: &Command) -> &'static str {
    match command {
        Command::Move { .. } => "move",
        Command::AttackMove { .. } => "attackMove",
        Command::Attack { .. } => "attack",
        Command::Deconstruct { .. } => "deconstruct",
        Command::SetupAntiTankGuns { .. } => "setupAntiTankGuns",
        Command::TearDownAntiTankGuns { .. } => "tearDownAntiTankGuns",
        Command::Charge { .. } => "charge",
        Command::UseAbility { .. } => "useAbility",
        Command::RecastAbility { .. } => "recastAbility",
        Command::SetAutocast { .. } => "setAutocast",
        Command::Gather { .. } => "gather",
        Command::Build { .. } => "build",
        Command::Train { .. } => "train",
        Command::AdjustProductionRepeat { .. } => "adjustProductionRepeat",
        Command::Research { .. } => "research",
        Command::Cancel { .. } => "cancel",
        Command::Stop { .. } => "stop",
        Command::HoldPosition { .. } => "holdPosition",
        Command::SetRally { .. } => "setRally",
    }
}

fn state_for_order(kind: MoveOrderKind) -> &'static str {
    match kind {
        MoveOrderKind::Move => "move",
        MoveOrderKind::AttackMove => "move",
        MoveOrderKind::HoldPosition => "idle",
    }
}

fn js_error<E: ToString>(error: E) -> JsValue {
    JsValue::from_str(&error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_contract::PlayerStart;

    fn start_payload() -> StartPayload {
        StartPayload {
            player_id: 1,
            spectator: false,
            prediction_build_id: None,
            prediction_version: 0,
            match_run_id: None,
            capabilities: Default::default(),
            diagnostics: Default::default(),
            replay: None,
            lab: None,
            tick: 10,
            map: MapInfo {
                width: 64,
                height: 64,
                tile_size: balance::TILE_SIZE,
                terrain: vec![0; 64 * 64],
                resources: Vec::new(),
            },
            players: vec![
                PlayerStart {
                    id: 1,
                    team_id: 1,
                    faction_id: "kriegsia".to_string(),
                    name: "A".to_string(),
                    color: "#f00".to_string(),
                    is_ai: false,
                    start_tile_x: 5,
                    start_tile_y: 5,
                },
                PlayerStart {
                    id: 2,
                    team_id: 2,
                    faction_id: "kriegsia".to_string(),
                    name: "B".to_string(),
                    color: "#00f".to_string(),
                    is_ai: false,
                    start_tile_x: 50,
                    start_tile_y: 50,
                },
            ],
        }
    }

    fn snapshot() -> Snapshot {
        let mut owned = EntityView::new(101, 1, "worker", 100.0, 100.0, 40, 40, "idle");
        owned.order_plan = vec![OrderPlanMarker {
            kind: "move".to_string(),
            x: 120.0,
            y: 100.0,
        }];
        let mut hidden_shape = EntityView::new(202, 2, "rifleman", 500.0, 500.0, 45, 45, "attack");
        hidden_shape.target_id = Some(101);
        hidden_shape.order_plan = vec![OrderPlanMarker {
            kind: "attack".to_string(),
            x: 100.0,
            y: 100.0,
        }];
        Snapshot {
            tick: 10,
            world_combat_active: false,
            steel: 75,
            oil: 0,
            supply_used: 1,
            supply_cap: 10,
            entities: vec![owned, hidden_shape],
            resource_deltas: Vec::new(),
            smokes: Vec::new(),
            ability_objects: Vec::new(),
            trenches: Vec::new(),
            visible_tiles: vec![0; 64 * 64],
            remembered_buildings: Vec::new(),
            events: Vec::new(),
            upgrades: Vec::new(),
            player_resources: vec![],
            net_status: SnapshotNetStatus::default(),
        }
    }

    #[test]
    fn baseline_from_snapshot_is_owner_safe() {
        let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
        assert_eq!(baseline.tick, 10);
        assert_eq!(baseline.owned_entities.len(), 1);
        assert_eq!(baseline.owned_entities[0].id, 101);
        assert_eq!(baseline.owned_entities[0].order_plan.len(), 1);
        assert_eq!(baseline.visible_obstacles.len(), 1);
        let json = serde_json::to_value(&baseline).unwrap();
        let serialized = serde_json::to_string(&json).unwrap();
        assert!(!serialized.contains("202"));
        assert!(!serialized.contains("target"));
        assert!(!serialized.contains("production"));
        assert!(!serialized.contains("playerResources"));
    }

    #[test]
    fn render_snapshot_excludes_visible_obstacles_and_fog_state() {
        let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
        let mut predictor = predictor_from_start_payload(start_payload(), 1);
        predictor.import_baseline(baseline).unwrap();

        let rendered = predictor.render_snapshot();
        assert_eq!(rendered.entities.len(), 1);
        assert_eq!(rendered.entities[0].owner, 1);
        assert_eq!(rendered.entities[0].id, 101);
        assert!(rendered.visible_tiles.is_empty());
        assert!(rendered.events.is_empty());
        assert!(rendered.smokes.is_empty());
        assert!(rendered.remembered_buildings.is_empty());

        let diagnostics = predictor.diagnostics();
        assert!(diagnostics
            .unsupported_fields
            .contains(&"combat".to_string()));
        assert!(diagnostics
            .unsupported_fields
            .contains(&"fogReconstruction".to_string()));
        assert_eq!(diagnostics.visible_obstacle_count, 1);
    }

    #[test]
    fn attack_command_is_authoritative_only() {
        let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
        let mut predictor = predictor_from_start_payload(start_payload(), 1);
        predictor.import_baseline(baseline).unwrap();
        let before = predictor.render_snapshot();

        predictor.enqueue_command(
            7,
            Command::Attack {
                units: vec![101],
                target: 202,
                queued: false,
            },
        );

        let after = predictor.render_snapshot();
        assert_eq!(after.entities[0].x, before.entities[0].x);
        assert_eq!(after.entities[0].y, before.entities[0].y);
        assert_eq!(after.entities[0].hp, before.entities[0].hp);
        assert_eq!(after.events.len(), 0);
        let diagnostics = predictor.diagnostics();
        assert_eq!(diagnostics.pending_client_seqs, vec![7]);
        assert!(diagnostics
            .disabled_reasons
            .contains(&"commandUnsupported".to_string()));
        assert!(diagnostics
            .unsupported_fields
            .contains(&"combat".to_string()));
    }

    #[test]
    fn repeat_production_command_is_tracked_as_authoritative_only() {
        let mut predictor = predictor_from_start_payload(start_payload(), 1);
        predictor.enqueue_command(
            8,
            Command::AdjustProductionRepeat {
                buildings: vec![301],
                unit: "rifleman".to_string(),
                delta: 1,
            },
        );

        let diagnostics = predictor.diagnostics();
        assert_eq!(
            diagnostics.pending_command_kinds,
            vec!["adjustProductionRepeat"]
        );
        assert!(diagnostics
            .disabled_reasons
            .contains(&"commandUnsupported".to_string()));
    }

    #[test]
    fn no_op_ticks_are_deterministic() {
        let baseline = OwnedPredictionBaseline {
            tick: 1,
            player_id: 1,
            steel: Some(75),
            oil: Some(0),
            supply_used: Some(1),
            supply_cap: Some(10),
            owned_entities: vec![OwnedEntityBaseline {
                id: 1,
                kind: "worker".to_string(),
                x: 10.0,
                y: 10.0,
                hp: 40,
                max_hp: 40,
                state: Some("idle".to_string()),
                facing: None,
                weapon_facing: None,
                order_plan: Vec::new(),
            }],
            visible_obstacles: Vec::new(),
        };
        let mut a = predictor_from_start_payload(start_payload(), 1);
        let mut b = predictor_from_start_payload(start_payload(), 1);
        a.import_baseline(baseline.clone()).unwrap();
        b.import_baseline(baseline).unwrap();
        a.advance_ticks(30);
        b.advance_ticks(30);
        assert_eq!(a.render_snapshot(), b.render_snapshot());
        assert_eq!(a.render_snapshot().entities[0].x, 10.0);
    }

    #[test]
    fn simple_move_command_advances_owned_unit() {
        let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
        let mut predictor = predictor_from_start_payload(start_payload(), 1);
        predictor.import_baseline(baseline).unwrap();
        predictor.enqueue_command(
            1,
            Command::Move {
                units: vec![101],
                x: 110.0,
                y: 100.0,
                queued: false,
            },
        );
        predictor.advance_ticks(3);
        let snapshot = predictor.render_snapshot();
        let entity = &snapshot.entities[0];
        assert!(entity.x > 100.0);
        assert_eq!(entity.owner, 1);
        assert_eq!(entity.y, 100.0);
        assert_eq!(entity.state, "move");
        assert_eq!(predictor.diagnostics().pending_commands, 1);
    }

    #[test]
    fn queued_move_commands_are_preserved_in_order() {
        let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
        let mut predictor = predictor_from_start_payload(start_payload(), 1);
        predictor.import_baseline(baseline).unwrap();
        predictor.enqueue_command(
            1,
            Command::Move {
                units: vec![101],
                x: 102.0,
                y: 100.0,
                queued: false,
            },
        );
        predictor.enqueue_command(
            2,
            Command::Move {
                units: vec![101],
                x: 102.0,
                y: 104.0,
                queued: true,
            },
        );
        let summary = predictor.local_lane_summary();
        assert_eq!(summary.owned_entities[0].order_plan[0].x, 102.0);
        assert_eq!(summary.owned_entities[0].queued_order_stages[0].y, 104.0);
        predictor.advance_ticks(2);
        assert_eq!(predictor.render_snapshot().entities[0].state, "move");
    }

    #[test]
    fn queued_hold_position_follows_the_last_move_then_stands_ground() {
        let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
        let mut predictor = predictor_from_start_payload(start_payload(), 1);
        predictor.import_baseline(baseline).unwrap();
        predictor.enqueue_command(
            1,
            Command::Move {
                units: vec![101],
                x: 110.0,
                y: 100.0,
                queued: false,
            },
        );
        predictor.enqueue_command(
            2,
            Command::HoldPosition {
                units: vec![101],
                queued: true,
            },
        );

        let summary = predictor.local_lane_summary();
        assert_eq!(summary.owned_entities[0].order_plan[0].kind, "move");
        assert_eq!(
            summary.owned_entities[0].queued_order_stages[0].kind,
            "holdPosition"
        );

        predictor.advance_ticks(16);
        let entity = &predictor.render_snapshot().entities[0];
        assert_eq!(entity.x, 110.0);
        assert_eq!(entity.y, 100.0);
        assert_eq!(entity.state, "idle");
    }

    #[test]
    fn authoritative_baseline_preserves_terminal_hold_position() {
        let mut authoritative = snapshot();
        authoritative.entities[0].order_plan = vec![
            OrderPlanMarker {
                kind: "move".to_string(),
                x: 110.0,
                y: 100.0,
            },
            OrderPlanMarker {
                kind: "holdPosition".to_string(),
                x: 110.0,
                y: 100.0,
            },
        ];
        let baseline = OwnedPredictionBaseline::from_snapshot(1, &authoritative);
        assert_eq!(baseline.owned_entities[0].order_plan.len(), 2);

        let mut predictor = predictor_from_start_payload(start_payload(), 1);
        predictor.import_baseline(baseline).unwrap();
        predictor.enqueue_command(
            3,
            Command::Move {
                units: vec![101],
                x: 120.0,
                y: 100.0,
                queued: true,
            },
        );

        let summary = predictor.local_lane_summary();
        assert_eq!(summary.owned_entities[0].order_plan[0].kind, "move");
        assert_eq!(summary.owned_entities[0].queued_order_stages.len(), 1);
        assert_eq!(
            summary.owned_entities[0].queued_order_stages[0].kind,
            "holdPosition"
        );
    }

    #[test]
    fn held_unit_promotes_a_later_queued_move() {
        let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
        let mut predictor = predictor_from_start_payload(start_payload(), 1);
        predictor.import_baseline(baseline).unwrap();
        predictor.enqueue_command(
            1,
            Command::HoldPosition {
                units: vec![101],
                queued: false,
            },
        );
        let held = &predictor.render_snapshot().entities[0];
        assert_eq!(held.state, "idle");
        assert!(held.order_plan.is_empty());

        predictor.enqueue_command(
            2,
            Command::Move {
                units: vec![101],
                x: 110.0,
                y: 100.0,
                queued: true,
            },
        );

        let queued = &predictor.render_snapshot().entities[0];
        assert_eq!(queued.state, "idle");
        assert_eq!(queued.order_plan.len(), 1);
        assert_eq!(queued.order_plan[0].kind, "move");

        predictor.advance_ticks(2);
        let moving = &predictor.render_snapshot().entities[0];
        assert!(moving.x > 100.0);
        assert_eq!(moving.state, "move");
    }

    #[test]
    fn importing_authoritative_baseline_clears_replayed_pending_commands() {
        let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
        let mut predictor = predictor_from_start_payload(start_payload(), 1);
        predictor.import_baseline(baseline.clone()).unwrap();
        predictor.enqueue_command(
            7,
            Command::Move {
                units: vec![101],
                x: 140.0,
                y: 100.0,
                queued: false,
            },
        );
        assert_eq!(predictor.diagnostics().pending_client_seqs, vec![7]);

        predictor.import_baseline(baseline).unwrap();
        assert!(predictor.diagnostics().pending_client_seqs.is_empty());
        predictor.enqueue_command(
            7,
            Command::Move {
                units: vec![101],
                x: 140.0,
                y: 100.0,
                queued: false,
            },
        );
        assert_eq!(predictor.diagnostics().pending_client_seqs, vec![7]);
    }

    #[test]
    fn invalid_build_is_reported_unsupported_without_mutating_baseline() {
        let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
        let mut predictor = predictor_from_start_payload(start_payload(), 1);
        predictor.import_baseline(baseline).unwrap();
        predictor.enqueue_command(
            1,
            Command::Build {
                units: vec![101],
                building: "not_a_building".to_string(),
                tile_x: u32::MAX,
                tile_y: u32::MAX,
                queued: false,
            },
        );
        predictor.advance_ticks(1);
        let diagnostics = predictor.diagnostics();
        assert!(diagnostics
            .disabled_reasons
            .contains(&"buildPredictionUnsupported".to_string()));
        assert!(diagnostics
            .unsupported_fields
            .contains(&"construction".to_string()));
    }

    #[test]
    fn json_api_round_trips_like_wasm_binding() {
        let start_json = serde_json::to_string(&start_payload()).unwrap();
        let baseline_json =
            serde_json::to_string(&OwnedPredictionBaseline::from_snapshot(1, &snapshot())).unwrap();
        let command_json = serde_json::to_string(&Command::Move {
            units: vec![101],
            x: 108.0,
            y: 100.0,
            queued: false,
        })
        .unwrap();
        let mut predictor =
            CorePredictor::from_start_payload(serde_json::from_str(&start_json).unwrap(), 1);
        predictor
            .import_baseline(serde_json::from_str(&baseline_json).unwrap())
            .unwrap();
        predictor.enqueue_command(1, serde_json::from_str(&command_json).unwrap());
        predictor.advance_ticks(5);
        let render_json = serde_json::to_string(&predictor.snapshot()).unwrap();
        assert!(render_json.contains("\"tick\":15"));
        assert!(serde_json::to_string(&predictor.diagnostics())
            .unwrap()
            .contains("pendingCommands"));
    }
}
