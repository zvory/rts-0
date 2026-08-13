//! Owner-safe browser prediction facade.
//!
//! This crate intentionally does not expose the authoritative [`rts_sim::game::Game`] world. The
//! browser imports only an [`OwnedPredictionBaseline`]: owned entities and owner economy fields
//! plus exact-owner progress baselines and visible non-authoritative obstacles with no enemy ids,
//! orders, target ids, or economy state. The runtime predicts the supported movement/order surface
//! and extrapolates already-active progress without claiming lifecycle completion.

use std::collections::{BTreeMap, VecDeque};

use rts_contract::{EntityView, MapInfo, OrderPlanMarker, Snapshot, StartPayload};
use rts_protocol::Command;
use rts_rules::{balance, static_blocker_class, EntityKind, StaticBlockerClass};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

const CORRECTION_EPS_PX: f32 = 0.01;
pub const PROGRESS_PREDICTION_MAX: f32 = 0.98;
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
    pub progress: Vec<OwnedProgressBaseline>,
    #[serde(default)]
    pub visible_obstacles: Vec<VisibleObstacle>,
}

impl OwnedPredictionBaseline {
    pub fn from_snapshot(player_id: u32, snapshot: &Snapshot) -> Self {
        let mut owned_entities = Vec::new();
        let mut progress = Vec::new();
        let mut visible_obstacles = Vec::new();
        for entity in &snapshot.entities {
            if entity.owner == player_id {
                owned_entities.push(OwnedEntityBaseline::from_view(entity));
                if let Some(baseline) = OwnedProgressBaseline::from_view(entity) {
                    progress.push(baseline);
                }
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
            progress,
            visible_obstacles,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OwnedProgressBaseline {
    pub id: u32,
    pub kind: ProgressPredictionKind,
    pub identity: String,
    pub fraction: f32,
    pub total_ticks: u32,
}

impl OwnedProgressBaseline {
    fn from_view(entity: &EntityView) -> Option<Self> {
        let entity_kind = entity.kind.parse::<EntityKind>().ok()?;
        if entity.build_active {
            let fraction = valid_incomplete_fraction(entity.build_progress?)?;
            let total_ticks = balance::building_stats(entity_kind)?.build_ticks;
            return (total_ticks > 0).then(|| Self {
                id: entity.id,
                kind: ProgressPredictionKind::Construction,
                identity: format!("build:{}", entity.kind),
                fraction,
                total_ticks,
            });
        }
        balance::building_stats(entity_kind)?;
        if entity.build_progress.is_some()
            || entity.state == "construct"
            || entity.prod_waiting
            || entity.prod_queue.unwrap_or(0) == 0
        {
            return None;
        }
        let fraction = valid_incomplete_fraction(entity.prod_progress?)?;
        let (identity, total_ticks) =
            match (entity.prod_upgrade.as_deref(), entity.prod_kind.as_deref()) {
                (Some(upgrade_id), None) => {
                    let upgrade = upgrade_id
                        .parse::<rts_sim::game::upgrade::UpgradeKind>()
                        .ok()?;
                    (
                        format!("upgrade:{upgrade_id}"),
                        rts_sim::game::upgrade::definition(upgrade).research_ticks,
                    )
                }
                (None, Some(unit_id)) => {
                    let unit = unit_id.parse::<EntityKind>().ok()?;
                    (
                        format!("unit:{unit_id}"),
                        balance::unit_stats(unit)?.build_ticks,
                    )
                }
                _ => return None,
            };
        (total_ticks > 0).then_some(Self {
            id: entity.id,
            kind: ProgressPredictionKind::Production,
            identity,
            fraction,
            total_ticks,
        })
    }
}

fn valid_incomplete_fraction(fraction: f32) -> Option<f32> {
    (fraction.is_finite() && (0.0..1.0).contains(&fraction)).then_some(fraction)
}

fn progress_baseline_matches_entity(
    baseline: &OwnedProgressBaseline,
    entity_kind: EntityKind,
) -> bool {
    match baseline.kind {
        ProgressPredictionKind::Construction => {
            balance::building_stats(entity_kind).is_some_and(|stats| {
                baseline.identity == format!("build:{}", entity_kind.stable_id())
                    && baseline.total_ticks == stats.build_ticks
            })
        }
        ProgressPredictionKind::Production => {
            if balance::building_stats(entity_kind).is_none() {
                return false;
            }
            if let Some(unit_id) = baseline.identity.strip_prefix("unit:") {
                return unit_id
                    .parse::<EntityKind>()
                    .ok()
                    .and_then(balance::unit_stats)
                    .is_some_and(|stats| baseline.total_ticks == stats.build_ticks);
            }
            if let Some(upgrade_id) = baseline.identity.strip_prefix("upgrade:") {
                return upgrade_id
                    .parse::<rts_sim::game::upgrade::UpgradeKind>()
                    .ok()
                    .is_some_and(|upgrade| {
                        baseline.total_ticks
                            == rts_sim::game::upgrade::definition(upgrade).research_ticks
                    });
            }
            false
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
    pub progress_lane_count: usize,
    pub progress_correction_magnitude: f32,
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
    pub progress: Vec<ProgressPredictionPatch>,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProgressPredictionKind {
    Construction,
    Production,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProgressPredictionPatch {
    pub id: u32,
    pub kind: ProgressPredictionKind,
    pub identity: String,
    pub fraction: f32,
}

#[derive(Debug, Clone)]
struct ProgressState {
    id: u32,
    kind: ProgressPredictionKind,
    identity: String,
    baseline_fraction: f32,
    total_ticks: u32,
    baseline_tick: u32,
}

impl ProgressState {
    fn try_from_baseline(
        baseline: OwnedProgressBaseline,
        baseline_tick: u32,
    ) -> Result<Self, String> {
        if valid_incomplete_fraction(baseline.fraction).is_none() || baseline.total_ticks == 0 {
            return Err(format!(
                "invalid progress baseline for entity {}",
                baseline.id
            ));
        }
        Ok(Self {
            id: baseline.id,
            kind: baseline.kind,
            identity: baseline.identity,
            baseline_fraction: baseline.fraction,
            total_ticks: baseline.total_ticks,
            baseline_tick,
        })
    }

    fn predicted_fraction(&self, tick: u32, visual_tick_fraction: f32) -> f32 {
        let elapsed_ticks = tick.wrapping_sub(self.baseline_tick) as f32 + visual_tick_fraction;
        (self.baseline_fraction + elapsed_ticks / self.total_ticks as f32)
            .min(PROGRESS_PREDICTION_MAX)
    }

    fn patch(&self, tick: u32, visual_tick_fraction: f32) -> Option<ProgressPredictionPatch> {
        let fraction = self.predicted_fraction(tick, visual_tick_fraction);
        (fraction > self.baseline_fraction).then(|| ProgressPredictionPatch {
            id: self.id,
            kind: self.kind,
            identity: self.identity.clone(),
            fraction,
        })
    }
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
    progress: BTreeMap<u32, ProgressState>,
    visible_obstacles: Vec<VisibleObstacle>,
    pending: VecDeque<PendingCommand>,
    correction_magnitude: f32,
    progress_correction_magnitude: f32,
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
            progress: BTreeMap::new(),
            visible_obstacles: Vec::new(),
            pending: VecDeque::new(),
            correction_magnitude: 0.0,
            progress_correction_magnitude: 0.0,
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
        self.progress_correction_magnitude =
            progress_correction_magnitude(&self.progress, self.tick, &baseline.progress);
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
        let progress = baseline
            .progress
            .into_iter()
            .map(|progress| {
                let entity = self.owned.get(&progress.id).ok_or_else(|| {
                    format!("progress baseline entity {} is not owned", progress.id)
                })?;
                if !progress_baseline_matches_entity(&progress, entity.kind) {
                    return Err(format!(
                        "progress baseline does not match owned building {}",
                        progress.id
                    ));
                }
                ProgressState::try_from_baseline(progress, self.tick)
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|progress| (progress.id, progress))
            .collect();
        self.progress = progress;
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

    fn prediction_frame(&self, visual_tick_fraction: f32) -> OwnedPredictionFrame {
        let visual_tick_fraction = normalize_visual_tick_fraction(visual_tick_fraction);
        OwnedPredictionFrame {
            tick: self.tick,
            entities: self
                .owned
                .values()
                .filter(|entity| entity.has_pose_claim())
                .map(EntityState::to_prediction_patch)
                .collect(),
            progress: self
                .progress
                .values()
                .filter_map(|progress| progress.patch(self.tick, visual_tick_fraction))
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
            progress_lane_count: self.progress.len(),
            progress_correction_magnitude: self.progress_correction_magnitude,
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
            | Command::ClearObstacleArea { .. }
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
        let target_x = x.clamp(0.0, self.world_max_x_px());
        let target_y = y.clamp(0.0, self.world_max_y_px());
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

    fn world_max_x_px(&self) -> f32 {
        (self.map.width as f32 * self.map.tile_size as f32 - 0.01).max(0.0)
    }

    fn world_max_y_px(&self) -> f32 {
        (self.map.height as f32 * self.map.tile_size as f32 - 0.01).max(0.0)
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
    pub fn render_prediction_frame_json(
        &self,
        visual_tick_fraction: f32,
    ) -> Result<String, JsValue> {
        serde_json::to_string(&self.core.prediction_frame(visual_tick_fraction)).map_err(js_error)
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

    pub fn render_prediction_frame(&self, visual_tick_fraction: f32) -> OwnedPredictionFrame {
        self.core.prediction_frame(visual_tick_fraction)
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

fn progress_correction_magnitude(
    current: &BTreeMap<u32, ProgressState>,
    current_tick: u32,
    baseline: &[OwnedProgressBaseline],
) -> f32 {
    baseline
        .iter()
        .filter_map(|next| {
            let old = current.get(&next.id)?;
            (old.kind == next.kind && old.identity == next.identity)
                .then(|| (old.predicted_fraction(current_tick, 0.0) - next.fraction).abs())
        })
        .fold(0.0, f32::max)
}

fn normalize_visual_tick_fraction(fraction: f32) -> f32 {
    if fraction.is_finite() {
        fraction.clamp(0.0, 1.0 - f32::EPSILON)
    } else {
        0.0
    }
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
        Command::ClearObstacleArea { .. } => "clearObstacleArea",
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
mod tests;
