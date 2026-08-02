//! Owner-safe browser prediction facade.
//!
//! This crate intentionally does not expose the authoritative [`rts_sim::game::Game`] world. The
//! browser imports only an [`OwnedPredictionBaseline`]: owned entities and owner economy fields
//! plus visible non-authoritative obstacles with no enemy ids, orders, target ids, production, or
//! economy state. Phase 3 predicts the supported movement/order surface and reports unsupported
//! systems explicitly so harness diffs can distinguish "unknown" from "divergent".

use std::collections::{BTreeMap, VecDeque};

use rts_contract::{EntityView, MapInfo, OrderPlanMarker, Snapshot, StartPayload};
use rts_protocol::Command;
use rts_rules::{balance, static_blocker_class, EntityKind, StaticBlockerClass};
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
            } else if is_visible_prediction_obstacle(entity) {
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

fn is_visible_prediction_obstacle(entity: &EntityView) -> bool {
    entity.owner != 0
        || entity
            .kind
            .parse()
            .is_ok_and(|kind| static_blocker_class(kind) != StaticBlockerClass::None)
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
    /// Number of retained supported stages before an authoritative stage this partial predictor
    /// cannot model. `Some(0)` means even the first retained/local queued stage is behind authority.
    #[serde(default)]
    pub authoritative_barrier_after: Option<usize>,
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
            order_plan: owner_safe_order_plan_prefix(&entity.order_plan),
            authoritative_barrier_after: authoritative_order_barrier(entity),
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

/// Sparse, owner-scoped presentation claims produced by the partial predictor.
///
/// This intentionally is not a protocol `Snapshot`: the predictor cannot claim authoritative
/// identity, health, activity, combat, economy, or fog state. Each entry may only adjust the pose
/// of an entity that the client already has in its authoritative owned-entity snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OwnedPredictionFrame {
    pub tick: u32,
    pub entities: Vec<EntityPredictionPatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EntityPredictionPatch {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facing: Option<f32>,
    /// Present only when a locally issued, supported command gives the predictor explicit
    /// presentation authority. Baseline activity never populates this field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motion: Option<PredictedMotion>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PredictedMotion {
    Move,
    Idle,
}

#[derive(Debug, Clone)]
struct EntityState {
    id: u32,
    kind: EntityKind,
    x: f32,
    y: f32,
    state: String,
    facing: Option<f32>,
    motion: Option<PredictedMotion>,
    authoritative_barrier_after: Option<usize>,
    terminal_pose_claim: bool,
    active_order: Option<MoveOrder>,
    queued_orders: VecDeque<MoveOrder>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MoveOrder {
    kind: MoveOrderKind,
    x: f32,
    y: f32,
    motion: Option<PredictedMotion>,
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

    fn prediction_frame(&self) -> OwnedPredictionFrame {
        OwnedPredictionFrame {
            tick: self.tick,
            entities: self
                .owned
                .values()
                .filter(|entity| entity.has_pose_claim())
                .map(EntityState::to_prediction_patch)
                .collect(),
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
            Command::FormationMove {
                units,
                points,
                attack_move,
                queued,
            } => {
                if let Some(point) = points.last() {
                    self.apply_move(
                        units,
                        point.x,
                        point.y,
                        *queued,
                        if *attack_move {
                            MoveOrderKind::AttackMove
                        } else {
                            MoveOrderKind::Move
                        },
                    );
                } else {
                    self.note_disabled("invalidFormationMoveTarget");
                }
            }
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
                        entity.motion = Some(PredictedMotion::Idle);
                        entity.authoritative_barrier_after = None;
                        entity.terminal_pose_claim = false;
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
            | Command::ArtilleryFire { .. }
            | Command::UseAbility { .. }
            | Command::RecastAbility { .. }
            | Command::SetAutocast { .. }
            | Command::Gather { .. }
            | Command::Train { .. }
            | Command::AdjustProductionRepeat { .. }
            | Command::SetAutoBuildSettings { .. }
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
                    motion: Some(PredictedMotion::Move),
                };
                if queued {
                    if !entity.queue_has_terminal_hold() {
                        entity.queued_orders.push_back(order);
                    }
                } else {
                    entity.active_order = Some(order);
                    entity.queued_orders.clear();
                    entity.state = state_for_order(kind).to_string();
                    entity.motion = order.motion;
                    entity.authoritative_barrier_after = None;
                    entity.terminal_pose_claim = false;
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
                    motion: Some(PredictedMotion::Idle),
                });
            } else {
                entity.active_order = Some(MoveOrder {
                    kind: MoveOrderKind::HoldPosition,
                    x: entity.x,
                    y: entity.y,
                    motion: Some(PredictedMotion::Idle),
                });
                entity.queued_orders.clear();
                entity.state = "idle".to_string();
                entity.motion = Some(PredictedMotion::Idle);
                entity.authoritative_barrier_after = None;
                entity.terminal_pose_claim = false;
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
        let authoritative_barrier_after = baseline.authoritative_barrier_after;
        let kind = baseline
            .kind
            .parse::<EntityKind>()
            .map_err(|_| format!("unsupported entity kind {:?}", baseline.kind))?;
        let mut active_order = None;
        let mut queued_orders = VecDeque::new();
        for marker in owner_safe_order_plan_prefix(&baseline.order_plan) {
            let order = MoveOrder {
                kind: match marker.kind.as_str() {
                    "attackMove" => MoveOrderKind::AttackMove,
                    "holdPosition" => MoveOrderKind::HoldPosition,
                    _ => MoveOrderKind::Move,
                },
                x: marker.x,
                y: marker.y,
                // Reconstructing an authoritative baseline order is sufficient for pose
                // prediction, but does not grant presentation authority over gameplay activity.
                motion: None,
            };
            // Authoritative active HoldPosition is intentionally absent from orderPlan, so every
            // hold marker arriving in a baseline is a queued terminal stage.
            if authoritative_barrier_after != Some(0)
                && active_order.is_none()
                && order.kind != MoveOrderKind::HoldPosition
            {
                active_order = Some(order);
            } else {
                queued_orders.push_back(order);
            }
        }
        let state = baseline.state.unwrap_or_else(|| {
            active_order
                .map(|order| state_for_order(order.kind).to_string())
                .unwrap_or_else(|| "idle".to_string())
        });
        Ok(Self {
            id: baseline.id,
            kind,
            x: baseline.x,
            y: baseline.y,
            state,
            facing: baseline.facing,
            motion: None,
            authoritative_barrier_after,
            terminal_pose_claim: false,
            active_order,
            queued_orders,
        })
    }

    fn advance_one_tick(&mut self) {
        let Some(order) = self.active_order else {
            if self.queued_orders.is_empty() {
                self.state = "idle".to_string();
            } else if self.authoritative_barrier_after == Some(0) {
                // Gathering, construction, combat, and other unsupported authoritative activity
                // may own the current stage. A queued local move is not active merely because the
                // movement-only model cannot see that stage.
                return;
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
        let completed_pose_order = self.active_order.is_some();
        let completed_predicted_order = self
            .active_order
            .is_some_and(|order| order.motion.is_some());
        if completed_pose_order {
            if let Some(stages_before_barrier) = self.authoritative_barrier_after.as_mut() {
                *stages_before_barrier = stages_before_barrier.saturating_sub(1);
            }
        }
        self.active_order = if self.authoritative_barrier_after == Some(0) {
            None
        } else {
            self.queued_orders.pop_front()
        };
        self.motion = self
            .active_order
            .and_then(|order| order.motion)
            .or_else(|| completed_predicted_order.then_some(PredictedMotion::Idle));
        self.terminal_pose_claim = completed_pose_order && self.active_order.is_none();
        self.state = self
            .active_order
            .map(|order| state_for_order(order.kind).to_string())
            .unwrap_or_else(|| "idle".to_string());
    }

    fn to_prediction_patch(&self) -> EntityPredictionPatch {
        EntityPredictionPatch {
            id: self.id,
            x: self.x,
            y: self.y,
            facing: self.facing,
            motion: self.motion,
        }
    }

    fn has_pose_claim(&self) -> bool {
        self.active_order.is_some() || self.motion.is_some() || self.terminal_pose_claim
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

    #[wasm_bindgen(js_name = renderPredictionFrameJson)]
    pub fn render_prediction_frame_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.core.prediction_frame()).map_err(js_error)
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

    pub fn render_prediction_frame(&self) -> OwnedPredictionFrame {
        self.core.prediction_frame()
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

fn owner_safe_order_plan_prefix(markers: &[OrderPlanMarker]) -> Vec<OrderPlanMarker> {
    markers
        .iter()
        .take_while(|marker| is_supported_order_marker(marker))
        .cloned()
        .collect()
}

fn authoritative_order_barrier(entity: &EntityView) -> Option<usize> {
    let first_unsupported = entity
        .order_plan
        .iter()
        .position(|marker| !is_supported_order_marker(marker));
    if entity.order_plan.is_empty() {
        (entity.state != "idle").then_some(0)
    } else if entity.state != "move" {
        Some(0)
    } else {
        first_unsupported
    }
}

fn is_supported_order_marker(marker: &OrderPlanMarker) -> bool {
    matches!(marker.kind.as_str(), "move" | "attackMove" | "holdPosition")
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
        Command::FormationMove { .. } => "formationMove",
        Command::AttackMove { .. } => "attackMove",
        Command::Attack { .. } => "attack",
        Command::Deconstruct { .. } => "deconstruct",
        Command::SetupAntiTankGuns { .. } => "setupAntiTankGuns",
        Command::TearDownAntiTankGuns { .. } => "tearDownAntiTankGuns",
        Command::Charge { .. } => "charge",
        Command::ArtilleryFire { .. } => "artilleryFire",
        Command::UseAbility { .. } => "useAbility",
        Command::RecastAbility { .. } => "recastAbility",
        Command::SetAutocast { .. } => "setAutocast",
        Command::Gather { .. } => "gather",
        Command::Build { .. } => "build",
        Command::Train { .. } => "train",
        Command::AdjustProductionRepeat { .. } => "adjustProductionRepeat",
        Command::SetAutoBuildSettings { .. } => "setAutoBuildSettings",
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
    use rts_contract::{PlayerStart, SnapshotNetStatus};

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
            observer_view: None,
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
        let mut owned = EntityView::new(101, 1, "worker", 100.0, 100.0, 40, 40, "move");
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
        let neutral_tank_trap =
            EntityView::new(303, 0, "tank_trap", 300.0, 300.0, 100, 100, "idle");
        Snapshot {
            tick: 10,
            world_combat_position: None,
            steel: 75,
            oil: 0,
            supply_used: 1,
            supply_cap: 10,
            auto_build: None,
            entities: vec![owned, hidden_shape, neutral_tank_trap],
            resource_deltas: Vec::new(),
            smokes: Vec::new(),
            ability_objects: Vec::new(),
            trenches: Vec::new(),
            visible_tiles: vec![0; 64 * 64],
            explored_tiles: vec![0; 64 * 64],
            remembered_buildings: Vec::new(),
            remembered_anti_tank_guns: Vec::new(),
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
        assert_eq!(baseline.visible_obstacles.len(), 2);
        assert!(baseline
            .visible_obstacles
            .iter()
            .any(|obstacle| obstacle.kind == "tank_trap"));
        let json = serde_json::to_value(&baseline).unwrap();
        let serialized = serde_json::to_string(&json).unwrap();
        assert!(!serialized.contains("202"));
        assert!(!serialized.contains("target"));
        assert!(!serialized.contains("production"));
        assert!(!serialized.contains("playerResources"));
    }

    #[test]
    fn prediction_frame_contains_only_owned_pose_patches() {
        let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
        let mut predictor = predictor_from_start_payload(start_payload(), 1);
        predictor.import_baseline(baseline).unwrap();

        let rendered = predictor.render_prediction_frame();
        assert_eq!(rendered.entities.len(), 1);
        assert_eq!(rendered.entities[0].id, 101);
        assert_eq!(rendered.entities[0].x, 100.0);
        assert_eq!(rendered.entities[0].y, 100.0);
        assert_eq!(rendered.entities[0].motion, None);

        let json = serde_json::to_value(&rendered).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "tick": 10,
                "entities": [{ "id": 101, "x": 100.0, "y": 100.0 }]
            })
        );

        let diagnostics = predictor.diagnostics();
        assert!(diagnostics
            .unsupported_fields
            .contains(&"combat".to_string()));
        assert!(diagnostics
            .unsupported_fields
            .contains(&"fogReconstruction".to_string()));
        assert_eq!(diagnostics.visible_obstacle_count, 2);
    }

    #[test]
    fn baseline_gather_and_build_activity_are_preserved_by_absence() {
        for authoritative_state in ["gather", "build"] {
            let mut authoritative = snapshot();
            authoritative.entities[0].state = authoritative_state.to_string();
            authoritative.entities[0].order_plan.clear();
            let baseline = OwnedPredictionBaseline::from_snapshot(1, &authoritative);
            let mut predictor = predictor_from_start_payload(start_payload(), 1);
            predictor.import_baseline(baseline).unwrap();
            predictor.enqueue_command(
                5,
                Command::Move {
                    units: vec![101],
                    x: 140.0,
                    y: 100.0,
                    queued: true,
                },
            );

            predictor.advance_ticks(5);
            let frame = predictor.render_prediction_frame();
            assert!(frame.entities.is_empty(), "{authoritative_state}");
        }
    }

    #[test]
    fn unsupported_active_order_plan_stage_discards_unreachable_supported_suffix() {
        for unsupported_kind in ["gather", "build"] {
            let mut authoritative = snapshot();
            authoritative.entities[0].state = unsupported_kind.to_string();
            authoritative.entities[0].order_plan = vec![
                OrderPlanMarker {
                    kind: unsupported_kind.to_string(),
                    x: 100.0,
                    y: 100.0,
                },
                OrderPlanMarker {
                    kind: "move".to_string(),
                    x: 140.0,
                    y: 100.0,
                },
            ];

            let baseline = OwnedPredictionBaseline::from_snapshot(1, &authoritative);
            assert_eq!(
                baseline.owned_entities[0].authoritative_barrier_after,
                Some(0)
            );
            assert!(baseline.owned_entities[0].order_plan.is_empty());

            let mut predictor = predictor_from_start_payload(start_payload(), 1);
            predictor.import_baseline(baseline).unwrap();
            predictor.advance_ticks(30);
            assert!(
                predictor.render_prediction_frame().entities.is_empty(),
                "{unsupported_kind}"
            );
        }
    }

    #[test]
    fn middle_authoritative_barrier_stops_retained_and_local_queued_moves() {
        for barrier_kind in ["gather", "build"] {
            let mut authoritative = snapshot();
            authoritative.entities[0].state = "move".to_string();
            authoritative.entities[0].order_plan = vec![
                OrderPlanMarker {
                    kind: "move".to_string(),
                    x: 102.0,
                    y: 100.0,
                },
                OrderPlanMarker {
                    kind: barrier_kind.to_string(),
                    x: 102.0,
                    y: 100.0,
                },
                OrderPlanMarker {
                    kind: "move".to_string(),
                    x: 140.0,
                    y: 100.0,
                },
            ];
            let baseline = OwnedPredictionBaseline::from_snapshot(1, &authoritative);
            assert_eq!(
                baseline.owned_entities[0].authoritative_barrier_after,
                Some(1)
            );
            assert_eq!(baseline.owned_entities[0].order_plan.len(), 1);

            let mut predictor = predictor_from_start_payload(start_payload(), 1);
            predictor.import_baseline(baseline).unwrap();
            predictor.enqueue_command(
                9,
                Command::Move {
                    units: vec![101],
                    x: 160.0,
                    y: 100.0,
                    queued: true,
                },
            );
            predictor.advance_ticks(60);

            let frame = predictor.render_prediction_frame();
            assert_eq!(frame.entities[0].x, 102.0, "{barrier_kind}");
            assert_eq!(frame.entities[0].motion, None, "{barrier_kind}");
            let summary = predictor.local_lane_summary();
            assert!(summary.owned_entities[0].order_plan.is_empty());
            assert_eq!(summary.owned_entities[0].queued_order_stages[0].x, 160.0);
        }
    }

    #[test]
    fn non_movement_authoritative_state_blocks_visible_move_marker() {
        for authoritative_state in ["attack", "gather", "build", "idle"] {
            let mut authoritative = snapshot();
            authoritative.entities[0].state = authoritative_state.to_string();
            authoritative.entities[0].order_plan = vec![OrderPlanMarker {
                kind: "move".to_string(),
                x: 140.0,
                y: 100.0,
            }];
            let baseline = OwnedPredictionBaseline::from_snapshot(1, &authoritative);
            assert_eq!(
                baseline.owned_entities[0].authoritative_barrier_after,
                Some(0)
            );

            let mut predictor = predictor_from_start_payload(start_payload(), 1);
            predictor.import_baseline(baseline).unwrap();
            predictor.advance_ticks(60);
            assert!(
                predictor.render_prediction_frame().entities.is_empty(),
                "{authoritative_state}"
            );
            assert_eq!(
                predictor.local_lane_summary().owned_entities[0].queued_order_stages[0].x,
                140.0
            );
        }
    }

    #[test]
    fn baseline_movement_keeps_terminal_pose_claim_until_reconciliation() {
        let mut predictor = predictor_from_start_payload(start_payload(), 1);
        predictor
            .import_baseline(OwnedPredictionBaseline::from_snapshot(1, &snapshot()))
            .unwrap();

        predictor.advance_ticks(100);
        assert_eq!(
            serde_json::to_value(predictor.render_prediction_frame()).unwrap(),
            serde_json::json!({
                "tick": 110,
                "entities": [{
                    "id": 101,
                    "x": 120.0,
                    "y": 100.0,
                    "facing": 0.0
                }]
            })
        );

        predictor.advance_ticks(30);
        let retained = predictor.render_prediction_frame();
        assert_eq!(retained.entities[0].x, 120.0);
        assert_eq!(retained.entities[0].motion, None);

        let mut reconciled = snapshot();
        reconciled.tick = 140;
        reconciled.entities[0].x = 120.0;
        reconciled.entities[0].state = "idle".to_string();
        reconciled.entities[0].order_plan.clear();
        predictor
            .import_baseline(OwnedPredictionBaseline::from_snapshot(1, &reconciled))
            .unwrap();
        assert!(predictor.render_prediction_frame().entities.is_empty());
    }

    #[test]
    fn serialized_pose_patch_has_no_identity_or_full_state_claims() {
        let patch = EntityPredictionPatch {
            id: 101,
            x: 101.5,
            y: 99.25,
            facing: Some(0.5),
            motion: Some(PredictedMotion::Move),
        };

        assert_eq!(
            serde_json::to_value(patch).unwrap(),
            serde_json::json!({
                "id": 101,
                "x": 101.5,
                "y": 99.25,
                "facing": 0.5,
                "motion": "move"
            })
        );
    }

    #[test]
    fn attack_command_is_authoritative_only() {
        let baseline = OwnedPredictionBaseline::from_snapshot(1, &snapshot());
        let mut predictor = predictor_from_start_payload(start_payload(), 1);
        predictor.import_baseline(baseline).unwrap();
        let before = predictor.render_prediction_frame();

        predictor.enqueue_command(
            7,
            Command::Attack {
                units: vec![101],
                target: 202,
                tank_trap_cluster: false,
                queued: false,
            },
        );

        let after = predictor.render_prediction_frame();
        assert_eq!(after.entities[0].x, before.entities[0].x);
        assert_eq!(after.entities[0].y, before.entities[0].y);
        assert_eq!(after.entities[0].motion, None);
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
                authoritative_barrier_after: None,
            }],
            visible_obstacles: Vec::new(),
        };
        let mut a = predictor_from_start_payload(start_payload(), 1);
        let mut b = predictor_from_start_payload(start_payload(), 1);
        a.import_baseline(baseline.clone()).unwrap();
        b.import_baseline(baseline).unwrap();
        a.advance_ticks(30);
        b.advance_ticks(30);
        assert_eq!(a.render_prediction_frame(), b.render_prediction_frame());
        assert!(a.render_prediction_frame().entities.is_empty());
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
        let frame = predictor.render_prediction_frame();
        let entity = &frame.entities[0];
        assert!(entity.x > 100.0);
        assert_eq!(entity.y, 100.0);
        assert_eq!(entity.motion, Some(PredictedMotion::Move));
        assert_eq!(entity.facing, Some(0.0));
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
        assert_eq!(
            predictor.render_prediction_frame().entities[0].motion,
            Some(PredictedMotion::Move)
        );
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
        let frame = predictor.render_prediction_frame();
        let entity = &frame.entities[0];
        assert_eq!(entity.x, 110.0);
        assert_eq!(entity.y, 100.0);
        assert_eq!(entity.motion, Some(PredictedMotion::Idle));
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
        let held = &predictor.render_prediction_frame().entities[0];
        assert_eq!(held.motion, Some(PredictedMotion::Idle));

        predictor.enqueue_command(
            2,
            Command::Move {
                units: vec![101],
                x: 110.0,
                y: 100.0,
                queued: true,
            },
        );

        let queued = &predictor.render_prediction_frame().entities[0];
        assert_eq!(queued.motion, Some(PredictedMotion::Idle));
        assert_eq!(
            predictor.local_lane_summary().owned_entities[0].queued_order_stages[0].kind,
            "move"
        );

        predictor.advance_ticks(2);
        let moving = &predictor.render_prediction_frame().entities[0];
        assert!(moving.x > 100.0);
        assert_eq!(moving.motion, Some(PredictedMotion::Move));
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
        let render_json = serde_json::to_string(&predictor.prediction_frame()).unwrap();
        assert!(render_json.contains("\"tick\":15"));
        assert!(serde_json::to_string(&predictor.diagnostics())
            .unwrap()
            .contains("pendingCommands"));
    }
}
