//! Pure command/order planning policy.
//!
//! This module intentionally does not mutate the simulation. It answers one question:
//! given an already-validated command shape plus issue-time facts about the selected
//! units, which unit-local actions should the authoritative command service apply?
use std::collections::{HashMap, HashSet};

pub type UnitId = u32;
pub type EntityId = u32;
pub type BuildKind = u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AbilityId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Point { x, y }
    }

    fn valid(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueMode {
    Immediate,
    Queue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitActivity {
    Idle,
    Moving,
    Busy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnitFacts {
    pub id: UnitId,
    pub pos: Point,
    pub can_receive_orders: bool,
    /// Immediate orders may replace this unit's current activity. Queued handoffs only require
    /// `can_receive_orders`, so an active constructor can accept future work without being pulled.
    pub can_replace_active: bool,
    pub queue_len: usize,
    pub active_build: bool,
    pub activity: UnitActivity,
    pub can_attack_move: bool,
    pub can_attack: bool,
    pub can_hold_position: bool,
    pub can_gather: bool,
    pub can_build: bool,
    pub can_setup_anti_tank_gun: bool,
    pub queue_terminal: bool,
    pub abilities: Vec<AbilityFacts>,
}

impl UnitFacts {
    pub fn new(id: UnitId) -> Self {
        UnitFacts {
            id,
            pos: Point::new(0.0, 0.0),
            can_receive_orders: true,
            can_replace_active: true,
            queue_len: 0,
            active_build: false,
            activity: UnitActivity::Idle,
            can_attack_move: true,
            can_attack: false,
            can_hold_position: false,
            can_gather: false,
            can_build: false,
            can_setup_anti_tank_gun: false,
            queue_terminal: false,
            abilities: Vec::new(),
        }
    }

    fn ability(&self, ability: AbilityId) -> Option<&AbilityFacts> {
        self.abilities.iter().find(|a| a.ability == ability)
    }

    fn ability_ready(&self, ability: AbilityId) -> bool {
        matches!(self.ability(ability), Some(a) if a.ready_at_issue)
    }

    fn ability_queue_admissible(&self, ability: AbilityId) -> bool {
        matches!(self.ability(ability), Some(a) if a.queue_admissible_at_issue)
    }

    fn can_execute_ability_without_interrupt(&self, ability: AbilityId) -> bool {
        matches!(self.ability(ability), Some(a) if a.ready_at_issue && a.can_execute_without_interrupt)
    }

    fn can_interrupt_with_ability(&self, ability: AbilityId) -> bool {
        matches!(self.ability(ability), Some(a) if a.ready_at_issue && a.can_interrupt_active_order)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbilityFacts {
    pub ability: AbilityId,
    pub ready_at_issue: bool,
    pub queue_admissible_at_issue: bool,
    /// True when this ability can fire now without replacing the active order.
    ///
    /// For a moving scout car and an in-range smoke target, this lets reactive smoke
    /// launch while the move order and future queue remain intact.
    pub can_execute_without_interrupt: bool,
    /// True when this ability may replace a non-idle active order.
    ///
    /// Mortar Fire uses this to make a manual fire order interrupt movement, while Smoke keeps its
    /// reactive noninterrupting behavior and otherwise falls back to idle carriers only.
    pub can_interrupt_active_order: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AbilityTarget {
    SelfTarget,
    WorldPoint(Point),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RequestedOrder {
    Move {
        to: Point,
    },
    AttackMove {
        to: Point,
    },
    HoldPosition,
    AttackTarget {
        target: EntityId,
        target_valid: bool,
    },
    Gather {
        node: EntityId,
        node_valid: bool,
    },
    Build {
        kind: BuildKind,
        tile_x: u32,
        tile_y: u32,
        target: Point,
        placement_valid: bool,
    },
    Deconstruct {
        target: EntityId,
        target_point: Point,
        target_valid: bool,
    },
    SetupAntiTankGuns {
        face_toward: Point,
    },
    UseAbility {
        ability: AbilityId,
        target: AbilityTarget,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderRequest {
    pub units: Vec<UnitId>,
    pub mode: IssueMode,
    pub order: RequestedOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlannerConfig {
    pub max_units_per_command: usize,
    pub max_queue_len: usize,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        PlannerConfig {
            max_units_per_command: 256,
            max_queue_len: 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OrderIntent {
    Move(Point),
    AttackMove(Point),
    HoldPosition,
    AttackTarget(EntityId),
    Gather(EntityId),
    Build {
        kind: BuildKind,
        tile_x: u32,
        tile_y: u32,
    },
    Deconstruct(EntityId),
    SetupAntiTankGuns {
        face_toward: Point,
    },
    WorldAbility {
        ability: AbilityId,
        target: Point,
    },
    SelfAbility {
        ability: AbilityId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlannedAction {
    /// Replace the active order and clear queued future orders for this unit.
    ReplaceActive { unit: UnitId, intent: OrderIntent },
    /// Append a future order stage for this unit.
    AppendQueued { unit: UnitId, intent: OrderIntent },
    /// Execute an ability immediately. When `preserve_orders` is true, the caller
    /// must leave the active order and queued intents untouched.
    ExecuteAbilityNow {
        unit: UnitId,
        ability: AbilityId,
        target: AbilityTarget,
        preserve_orders: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannerNotice {
    QueueFull { unit: UnitId },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlannerOutput {
    pub actions: Vec<PlannedAction>,
    pub notices: Vec<PlannerNotice>,
}

pub fn plan_order(
    config: PlannerConfig,
    facts: &[UnitFacts],
    request: &OrderRequest,
) -> PlannerOutput {
    let by_id: HashMap<UnitId, &UnitFacts> = facts.iter().map(|u| (u.id, u)).collect();
    let requested = dedupe_cap(&request.units, config.max_units_per_command);
    let ordered_facts: Vec<&UnitFacts> = requested
        .iter()
        .filter_map(|id| by_id.get(id).copied())
        .filter(|u| u.can_receive_orders)
        .collect();

    match request.order {
        RequestedOrder::Move { to } if to.valid() => {
            plan_simple_point(config, request.mode, &ordered_facts, OrderIntent::Move(to))
        }
        RequestedOrder::AttackMove { to } if to.valid() => plan_filtered_units(
            config,
            request.mode,
            &ordered_facts,
            |u| u.can_attack_move,
            OrderIntent::AttackMove(to),
        ),
        RequestedOrder::HoldPosition => plan_filtered_units(
            config,
            request.mode,
            &ordered_facts,
            |u| u.can_hold_position,
            OrderIntent::HoldPosition,
        ),
        RequestedOrder::AttackTarget {
            target,
            target_valid: true,
        } => plan_filtered_units(
            config,
            request.mode,
            &ordered_facts,
            |u| u.can_attack,
            OrderIntent::AttackTarget(target),
        ),
        RequestedOrder::Gather {
            node,
            node_valid: true,
        } => plan_filtered_units(
            config,
            request.mode,
            &ordered_facts,
            |u| u.can_gather,
            OrderIntent::Gather(node),
        ),
        RequestedOrder::Build {
            kind,
            tile_x,
            tile_y,
            target,
            placement_valid: true,
        } if target.valid() => plan_build(
            config,
            request.mode,
            &ordered_facts,
            kind,
            tile_x,
            tile_y,
            target,
        ),
        RequestedOrder::Deconstruct {
            target,
            target_point,
            target_valid: true,
        } if target_point.valid() => {
            plan_deconstruct(config, request.mode, &ordered_facts, target, target_point)
        }
        RequestedOrder::SetupAntiTankGuns { face_toward } if face_toward.valid() => {
            plan_filtered_units(
                config,
                request.mode,
                &ordered_facts,
                |u| u.can_setup_anti_tank_gun,
                OrderIntent::SetupAntiTankGuns { face_toward },
            )
        }
        RequestedOrder::UseAbility { ability, target } => {
            plan_ability(config, request.mode, &ordered_facts, ability, target)
        }
        _ => PlannerOutput::default(),
    }
}

fn plan_simple_point(
    config: PlannerConfig,
    mode: IssueMode,
    units: &[&UnitFacts],
    intent: OrderIntent,
) -> PlannerOutput {
    plan_filtered_units(config, mode, units, |_| true, intent)
}

fn plan_filtered_units(
    config: PlannerConfig,
    mode: IssueMode,
    units: &[&UnitFacts],
    predicate: impl Fn(&UnitFacts) -> bool,
    intent: OrderIntent,
) -> PlannerOutput {
    let mut out = PlannerOutput::default();
    for unit in units.iter().copied().filter(|u| predicate(u)) {
        match mode {
            IssueMode::Immediate => out.actions.push(PlannedAction::ReplaceActive {
                unit: unit.id,
                intent,
            }),
            IssueMode::Queue => append_or_notice(config, &mut out, unit, intent),
        }
    }
    out
}

fn plan_ability(
    config: PlannerConfig,
    mode: IssueMode,
    units: &[&UnitFacts],
    ability: AbilityId,
    target: AbilityTarget,
) -> PlannerOutput {
    match target {
        AbilityTarget::SelfTarget => plan_self_ability(config, mode, units, ability),
        AbilityTarget::WorldPoint(point) if point.valid() => {
            plan_world_ability(config, mode, units, ability, point)
        }
        AbilityTarget::WorldPoint(_) => PlannerOutput::default(),
    }
}

fn plan_build(
    config: PlannerConfig,
    mode: IssueMode,
    units: &[&UnitFacts],
    kind: BuildKind,
    tile_x: u32,
    tile_y: u32,
    target: Point,
) -> PlannerOutput {
    let mut out = PlannerOutput::default();
    let intent = OrderIntent::Build {
        kind,
        tile_x,
        tile_y,
    };
    let builders: Vec<&UnitFacts> = units.iter().copied().filter(|u| u.can_build).collect();
    if builders.is_empty() {
        return out;
    }

    match mode {
        IssueMode::Immediate => {
            let candidates: Vec<&UnitFacts> = builders
                .iter()
                .copied()
                .filter(|u| u.can_replace_active)
                .collect();
            if let Some(unit) = choose_immediate_work_worker(&candidates, target) {
                out.actions.push(PlannedAction::ReplaceActive {
                    unit: unit.id,
                    intent,
                });
            }
        }
        IssueMode::Queue => {
            if let Some(unit) = choose_queued_build_worker(&builders, config.max_queue_len, target)
            {
                append_or_notice(config, &mut out, unit, intent);
            } else {
                for unit in builders {
                    out.notices.push(PlannerNotice::QueueFull { unit: unit.id });
                }
            }
        }
    }
    out
}

fn plan_deconstruct(
    config: PlannerConfig,
    mode: IssueMode,
    units: &[&UnitFacts],
    target: EntityId,
    target_point: Point,
) -> PlannerOutput {
    let mut out = PlannerOutput::default();
    let intent = OrderIntent::Deconstruct(target);
    let builders: Vec<&UnitFacts> = units.iter().copied().filter(|u| u.can_build).collect();
    if builders.is_empty() {
        return out;
    }

    match mode {
        IssueMode::Immediate => {
            let candidates: Vec<&UnitFacts> = builders
                .iter()
                .copied()
                .filter(|u| u.can_replace_active)
                .collect();
            if let Some(unit) = choose_immediate_work_worker(&candidates, target_point) {
                out.actions.push(PlannedAction::ReplaceActive {
                    unit: unit.id,
                    intent,
                });
            }
        }
        IssueMode::Queue => {
            if let Some(unit) =
                choose_queued_build_worker(&builders, config.max_queue_len, target_point)
            {
                append_or_notice(config, &mut out, unit, intent);
            } else {
                for unit in builders {
                    out.notices.push(PlannerNotice::QueueFull { unit: unit.id });
                }
            }
        }
    }
    out
}

fn plan_self_ability(
    config: PlannerConfig,
    mode: IssueMode,
    units: &[&UnitFacts],
    ability: AbilityId,
) -> PlannerOutput {
    let mut out = PlannerOutput::default();
    match mode {
        IssueMode::Immediate => {
            for unit in units.iter().copied().filter(|u| u.ability_ready(ability)) {
                out.actions.push(PlannedAction::ExecuteAbilityNow {
                    unit: unit.id,
                    ability,
                    target: AbilityTarget::SelfTarget,
                    preserve_orders: true,
                });
            }
        }
        IssueMode::Queue => {
            for unit in units
                .iter()
                .copied()
                .filter(|u| u.ability_queue_admissible(ability))
            {
                append_or_notice(config, &mut out, unit, OrderIntent::SelfAbility { ability });
            }
        }
    }
    out
}

fn plan_world_ability(
    config: PlannerConfig,
    mode: IssueMode,
    units: &[&UnitFacts],
    ability: AbilityId,
    target: Point,
) -> PlannerOutput {
    let mut out = PlannerOutput::default();
    let ready: Vec<&UnitFacts> = units
        .iter()
        .copied()
        .filter(|u| u.ability_ready(ability))
        .collect();

    match mode {
        IssueMode::Immediate => {
            if ready.is_empty() {
                return out;
            }
            if let Some(unit) = choose_immediate_world_ability_unit(&ready, ability) {
                let preserve_orders = unit.can_execute_ability_without_interrupt(ability);
                if preserve_orders {
                    out.actions.push(PlannedAction::ExecuteAbilityNow {
                        unit: unit.id,
                        ability,
                        target: AbilityTarget::WorldPoint(target),
                        preserve_orders: true,
                    });
                } else {
                    out.actions.push(PlannedAction::ReplaceActive {
                        unit: unit.id,
                        intent: OrderIntent::WorldAbility { ability, target },
                    });
                }
            }
        }
        IssueMode::Queue => {
            let queued: Vec<&UnitFacts> = units
                .iter()
                .copied()
                .filter(|u| u.ability_queue_admissible(ability))
                .collect();
            if queued.is_empty() {
                return out;
            }
            if let Some(unit) = choose_queued_world_ability_unit(&queued, config.max_queue_len) {
                append_or_notice(
                    config,
                    &mut out,
                    unit,
                    OrderIntent::WorldAbility { ability, target },
                );
            } else {
                for unit in queued {
                    out.notices.push(PlannerNotice::QueueFull { unit: unit.id });
                }
            }
        }
    }
    out
}

fn choose_immediate_world_ability_unit<'a>(
    units: &'a [&'a UnitFacts],
    ability: AbilityId,
) -> Option<&'a UnitFacts> {
    units
        .iter()
        .copied()
        .find(|u| u.can_execute_ability_without_interrupt(ability))
        .or_else(|| {
            units
                .iter()
                .copied()
                .find(|u| matches!(u.activity, UnitActivity::Idle))
        })
        .or_else(|| {
            units
                .iter()
                .copied()
                .find(|u| u.can_interrupt_with_ability(ability))
        })
}

fn choose_queued_world_ability_unit<'a>(
    units: &'a [&'a UnitFacts],
    max_queue_len: usize,
) -> Option<&'a UnitFacts> {
    units
        .iter()
        .copied()
        .filter(|u| u.queue_len < max_queue_len)
        .min_by_key(|u| u.queue_len)
}

fn choose_queued_build_worker<'a>(
    units: &'a [&'a UnitFacts],
    max_queue_len: usize,
    target: Point,
) -> Option<&'a UnitFacts> {
    units
        .iter()
        .copied()
        .filter(|u| u.queue_len < max_queue_len)
        .min_by(|a, b| {
            (a.queue_len + usize::from(a.active_build))
                .cmp(&(b.queue_len + usize::from(b.active_build)))
                .then_with(|| distance2(a.pos, target).total_cmp(&distance2(b.pos, target)))
                .then_with(|| a.id.cmp(&b.id))
        })
}

fn choose_immediate_work_worker<'a>(
    units: &'a [&'a UnitFacts],
    target: Point,
) -> Option<&'a UnitFacts> {
    units.iter().copied().min_by(|a, b| {
        immediate_work_priority(a)
            .cmp(&immediate_work_priority(b))
            .then_with(|| distance2(a.pos, target).total_cmp(&distance2(b.pos, target)))
            .then_with(|| a.id.cmp(&b.id))
    })
}

fn immediate_work_priority(unit: &UnitFacts) -> u8 {
    if matches!(unit.activity, UnitActivity::Idle) {
        0
    } else if !unit.active_build {
        1
    } else {
        2
    }
}

fn distance2(a: Point, b: Point) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}

fn append_or_notice(
    config: PlannerConfig,
    out: &mut PlannerOutput,
    unit: &UnitFacts,
    intent: OrderIntent,
) {
    if unit.queue_terminal {
        return;
    }
    if unit.queue_len >= config.max_queue_len {
        out.notices.push(PlannerNotice::QueueFull { unit: unit.id });
        return;
    }
    out.actions.push(PlannedAction::AppendQueued {
        unit: unit.id,
        intent,
    });
}

fn dedupe_cap(units: &[UnitId], cap: usize) -> Vec<UnitId> {
    let mut out = Vec::with_capacity(units.len().min(cap));
    let mut seen = HashSet::new();
    for id in units.iter().copied() {
        if out.len() >= cap {
            break;
        }
        if seen.insert(id) {
            out.push(id);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMOKE: AbilityId = AbilityId(1);
    const CHARGE: AbilityId = AbilityId(2);
    const MORTAR_FIRE: AbilityId = AbilityId(3);

    fn smoke_click(units: &[UnitId], mode: IssueMode, x: f32) -> OrderRequest {
        OrderRequest {
            units: units.to_vec(),
            mode,
            order: RequestedOrder::UseAbility {
                ability: SMOKE,
                target: AbilityTarget::WorldPoint(Point::new(x, 100.0)),
            },
        }
    }

    fn attack_move(units: &[UnitId], mode: IssueMode) -> OrderRequest {
        OrderRequest {
            units: units.to_vec(),
            mode,
            order: RequestedOrder::AttackMove {
                to: Point::new(500.0, 100.0),
            },
        }
    }

    fn deconstruct(units: &[UnitId], mode: IssueMode, target: EntityId, x: f32) -> OrderRequest {
        OrderRequest {
            units: units.to_vec(),
            mode,
            order: RequestedOrder::Deconstruct {
                target,
                target_point: Point::new(x, 100.0),
                target_valid: true,
            },
        }
    }

    fn build(units: &[UnitId], mode: IssueMode, x: f32) -> OrderRequest {
        OrderRequest {
            units: units.to_vec(),
            mode,
            order: RequestedOrder::Build {
                kind: 1,
                tile_x: 4,
                tile_y: 4,
                target: Point::new(x, 100.0),
                placement_valid: true,
            },
        }
    }

    fn unit(id: UnitId) -> UnitFacts {
        UnitFacts::new(id)
    }

    fn with_ability(mut unit: UnitFacts, ability: AbilityId, ready: bool) -> UnitFacts {
        unit.abilities.push(AbilityFacts {
            ability,
            ready_at_issue: ready,
            queue_admissible_at_issue: ready,
            can_execute_without_interrupt: false,
            can_interrupt_active_order: false,
        });
        unit
    }

    fn with_queue_admissible_ability(
        mut unit: UnitFacts,
        ability: AbilityId,
        ready: bool,
    ) -> UnitFacts {
        unit.abilities.push(AbilityFacts {
            ability,
            ready_at_issue: ready,
            queue_admissible_at_issue: true,
            can_execute_without_interrupt: false,
            can_interrupt_active_order: false,
        });
        unit
    }

    fn with_non_interrupting_ability(
        mut unit: UnitFacts,
        ability: AbilityId,
        ready: bool,
    ) -> UnitFacts {
        unit.abilities.push(AbilityFacts {
            ability,
            ready_at_issue: ready,
            queue_admissible_at_issue: ready,
            can_execute_without_interrupt: true,
            can_interrupt_active_order: false,
        });
        unit
    }

    fn with_interrupting_ability(
        mut unit: UnitFacts,
        ability: AbilityId,
        ready: bool,
    ) -> UnitFacts {
        unit.abilities.push(AbilityFacts {
            ability,
            ready_at_issue: ready,
            queue_admissible_at_issue: ready,
            can_execute_without_interrupt: false,
            can_interrupt_active_order: true,
        });
        unit
    }

    fn queued_units(out: &PlannerOutput) -> Vec<UnitId> {
        out.actions
            .iter()
            .filter_map(|action| match action {
                PlannedAction::AppendQueued { unit, .. } => Some(*unit),
                _ => None,
            })
            .collect()
    }

    fn replace_units(out: &PlannerOutput) -> Vec<UnitId> {
        out.actions
            .iter()
            .filter_map(|action| match action {
                PlannedAction::ReplaceActive { unit, .. } => Some(*unit),
                _ => None,
            })
            .collect()
    }

    fn apply_queue_appends(facts: &mut [UnitFacts], out: &PlannerOutput) {
        for action in &out.actions {
            if let PlannedAction::AppendQueued { unit, .. } = action {
                if let Some(fact) = facts.iter_mut().find(|u| u.id == *unit) {
                    fact.queue_len += 1;
                }
            }
        }
    }

    #[test]
    fn queued_world_ability_assigns_one_ready_carrier_per_click_round_robin() {
        let config = PlannerConfig::default();
        let mut facts = vec![
            with_ability(unit(1), SMOKE, true),
            with_ability(unit(2), SMOKE, true),
        ];
        let mut assigned = Vec::new();

        for i in 0..4 {
            let out = plan_order(
                config,
                &facts,
                &smoke_click(&[1, 2], IssueMode::Queue, 100.0 + i as f32),
            );
            assert!(out.notices.is_empty());
            assigned.extend(queued_units(&out));
            apply_queue_appends(&mut facts, &out);
        }

        assert_eq!(assigned, vec![1, 2, 1, 2]);
    }

    #[test]
    fn queued_world_ability_requires_ready_at_issue_without_projection() {
        let config = PlannerConfig::default();
        let facts = vec![
            with_ability(unit(1), SMOKE, false),
            with_ability(unit(2), SMOKE, true),
        ];

        let out = plan_order(
            config,
            &facts,
            &smoke_click(&[1, 2], IssueMode::Queue, 100.0),
        );
        assert_eq!(queued_units(&out), vec![2]);

        let out = plan_order(config, &facts, &smoke_click(&[1], IssueMode::Queue, 100.0));
        assert!(out.actions.is_empty());
        assert!(out.notices.is_empty());
    }

    #[test]
    fn queued_wait_policy_world_ability_can_append_before_ready() {
        let config = PlannerConfig::default();
        let facts = vec![with_queue_admissible_ability(unit(1), SMOKE, false)];

        let out = plan_order(config, &facts, &smoke_click(&[1], IssueMode::Queue, 100.0));

        assert_eq!(queued_units(&out), vec![1]);
        assert!(out.notices.is_empty());

        let immediate = plan_order(
            config,
            &facts,
            &smoke_click(&[1], IssueMode::Immediate, 100.0),
        );
        assert!(
            immediate.actions.is_empty(),
            "immediate abilities still require ready-now facts"
        );
    }

    #[test]
    fn queued_self_ability_broadcasts_to_every_ready_carrier() {
        let config = PlannerConfig::default();
        let facts = vec![
            with_ability(unit(1), CHARGE, true),
            with_ability(unit(2), CHARGE, true),
            with_ability(unit(3), CHARGE, false),
        ];
        let request = OrderRequest {
            units: vec![1, 2, 3],
            mode: IssueMode::Queue,
            order: RequestedOrder::UseAbility {
                ability: CHARGE,
                target: AbilityTarget::SelfTarget,
            },
        };

        let out = plan_order(config, &facts, &request);

        assert_eq!(queued_units(&out), vec![1, 2]);
        assert_eq!(
            out.actions[0],
            PlannedAction::AppendQueued {
                unit: 1,
                intent: OrderIntent::SelfAbility { ability: CHARGE },
            }
        );
    }

    #[test]
    fn queued_smoke_then_attack_move_applies_later_attack_to_whole_selection() {
        let config = PlannerConfig::default();
        let scout = with_ability(unit(1), SMOKE, true);
        let mut tank = unit(2);
        tank.can_attack = true;
        let facts = vec![scout, tank];

        let smoke = plan_order(
            config,
            &facts,
            &smoke_click(&[1, 2], IssueMode::Queue, 100.0),
        );
        assert_eq!(queued_units(&smoke), vec![1]);

        let attack = plan_order(config, &facts, &attack_move(&[1, 2], IssueMode::Queue));
        assert_eq!(queued_units(&attack), vec![1, 2]);
    }

    #[test]
    fn invalid_targets_do_not_create_queued_stages() {
        let config = PlannerConfig::default();
        let mut attacker = unit(1);
        attacker.can_attack = true;
        let mut worker = unit(2);
        worker.can_gather = true;
        let facts = vec![attacker, worker];

        let bad_attack = OrderRequest {
            units: vec![1],
            mode: IssueMode::Queue,
            order: RequestedOrder::AttackTarget {
                target: 99,
                target_valid: false,
            },
        };
        let bad_gather = OrderRequest {
            units: vec![2],
            mode: IssueMode::Queue,
            order: RequestedOrder::Gather {
                node: 77,
                node_valid: false,
            },
        };

        assert!(plan_order(config, &facts, &bad_attack).actions.is_empty());
        assert!(plan_order(config, &facts, &bad_gather).actions.is_empty());
    }

    #[test]
    fn queue_full_emits_notice_for_valid_queued_order() {
        let config = PlannerConfig::default();
        let mut scout = with_ability(unit(1), SMOKE, true);
        scout.queue_len = config.max_queue_len;

        let out = plan_order(
            config,
            &[scout],
            &smoke_click(&[1], IssueMode::Queue, 100.0),
        );

        assert!(out.actions.is_empty());
        assert_eq!(out.notices, vec![PlannerNotice::QueueFull { unit: 1 }]);
    }

    #[test]
    fn immediate_world_ability_can_execute_without_interrupting_move() {
        let config = PlannerConfig::default();
        let mut scout = with_non_interrupting_ability(unit(1), SMOKE, true);
        scout.activity = UnitActivity::Moving;

        let out = plan_order(
            config,
            &[scout],
            &smoke_click(&[1], IssueMode::Immediate, 100.0),
        );

        assert_eq!(
            out.actions,
            vec![PlannedAction::ExecuteAbilityNow {
                unit: 1,
                ability: SMOKE,
                target: AbilityTarget::WorldPoint(Point::new(100.0, 100.0)),
                preserve_orders: true,
            }]
        );
    }

    #[test]
    fn immediate_world_ability_replaces_only_an_idle_ready_carrier_when_not_non_interrupting() {
        let config = PlannerConfig::default();
        let mut moving = with_ability(unit(1), SMOKE, true);
        moving.activity = UnitActivity::Moving;
        let idle = with_ability(unit(2), SMOKE, true);

        let out = plan_order(
            config,
            &[moving, idle],
            &smoke_click(&[1, 2], IssueMode::Immediate, 100.0),
        );

        assert_eq!(replace_units(&out), vec![2]);
    }

    #[test]
    fn immediate_world_ability_can_replace_moving_carrier_when_allowed() {
        let config = PlannerConfig::default();
        let mut moving = with_interrupting_ability(unit(1), MORTAR_FIRE, true);
        moving.activity = UnitActivity::Moving;

        let request = OrderRequest {
            units: vec![1],
            mode: IssueMode::Immediate,
            order: RequestedOrder::UseAbility {
                ability: MORTAR_FIRE,
                target: AbilityTarget::WorldPoint(Point::new(256.0, 128.0)),
            },
        };
        let out = plan_order(config, &[moving], &request);

        assert_eq!(replace_units(&out), vec![1]);
    }

    #[test]
    fn setup_anti_tank_guns_is_queueable_and_filters_to_setup_capable_units() {
        let config = PlannerConfig::default();
        let mut anti_tank_gun = unit(1);
        anti_tank_gun.can_setup_anti_tank_gun = true;
        let rifle = unit(2);
        let request = OrderRequest {
            units: vec![1, 2],
            mode: IssueMode::Queue,
            order: RequestedOrder::SetupAntiTankGuns {
                face_toward: Point::new(400.0, 200.0),
            },
        };

        let out = plan_order(config, &[anti_tank_gun, rifle], &request);

        assert_eq!(queued_units(&out), vec![1]);
        assert_eq!(
            out.actions,
            vec![PlannedAction::AppendQueued {
                unit: 1,
                intent: OrderIntent::SetupAntiTankGuns {
                    face_toward: Point::new(400.0, 200.0),
                },
            }]
        );
    }

    #[test]
    fn queued_deconstruct_assigns_one_worker_per_click_by_build_queue_load() {
        let config = PlannerConfig::default();
        let mut first = unit(1);
        first.can_build = true;
        let mut second = unit(2);
        second.can_build = true;
        let mut facts = vec![first, second];
        let mut assigned = Vec::new();

        for i in 0..4 {
            let out = plan_order(
                config,
                &facts,
                &deconstruct(&[1, 2], IssueMode::Queue, 100 + i, 100.0 + i as f32),
            );
            assert!(out.notices.is_empty());
            assigned.extend(queued_units(&out));
            apply_queue_appends(&mut facts, &out);
        }

        assert_eq!(assigned, vec![1, 2, 1, 2]);
    }

    #[test]
    fn immediate_deconstruct_prefers_idle_worker_like_build() {
        let config = PlannerConfig::default();
        let mut busy_close = unit(1);
        busy_close.can_build = true;
        busy_close.activity = UnitActivity::Busy;
        busy_close.pos = Point::new(100.0, 100.0);
        let mut idle_far = unit(2);
        idle_far.can_build = true;
        idle_far.pos = Point::new(300.0, 100.0);

        let out = plan_order(
            config,
            &[busy_close, idle_far],
            &deconstruct(&[1, 2], IssueMode::Immediate, 99, 96.0),
        );

        assert_eq!(
            out.actions,
            vec![PlannedAction::ReplaceActive {
                unit: 2,
                intent: OrderIntent::Deconstruct(99),
            }]
        );
    }

    #[test]
    fn immediate_deconstruct_prefers_busy_non_builder_over_active_builder() {
        let config = PlannerConfig::default();
        let mut active_builder = unit(1);
        active_builder.can_build = true;
        active_builder.activity = UnitActivity::Busy;
        active_builder.active_build = true;
        active_builder.pos = Point::new(100.0, 100.0);
        let mut gatherer = unit(2);
        gatherer.can_build = true;
        gatherer.activity = UnitActivity::Busy;
        gatherer.pos = Point::new(300.0, 100.0);

        let out = plan_order(
            config,
            &[active_builder, gatherer],
            &deconstruct(&[1, 2], IssueMode::Immediate, 99, 96.0),
        );

        assert_eq!(
            out.actions,
            vec![PlannedAction::ReplaceActive {
                unit: 2,
                intent: OrderIntent::Deconstruct(99),
            }]
        );
    }

    #[test]
    fn nonreplaceable_builder_can_receive_queued_handoff_but_not_immediate_work() {
        let config = PlannerConfig::default();
        let mut constructing = unit(1);
        constructing.can_build = true;
        constructing.can_replace_active = false;
        constructing.active_build = true;
        constructing.activity = UnitActivity::Busy;

        let immediate = plan_order(
            config,
            &[constructing.clone()],
            &build(&[1], IssueMode::Immediate, 100.0),
        );
        let queued = plan_order(
            config,
            &[constructing],
            &build(&[1], IssueMode::Queue, 100.0),
        );

        assert!(immediate.actions.is_empty());
        assert_eq!(queued_units(&queued), vec![1]);
    }

    #[test]
    fn invalid_deconstruct_target_does_not_create_queued_stage() {
        let config = PlannerConfig::default();
        let mut worker = unit(1);
        worker.can_build = true;
        let request = OrderRequest {
            units: vec![1],
            mode: IssueMode::Queue,
            order: RequestedOrder::Deconstruct {
                target: 99,
                target_point: Point::new(100.0, 100.0),
                target_valid: false,
            },
        };

        let out = plan_order(config, &[worker], &request);

        assert!(out.actions.is_empty());
        assert!(out.notices.is_empty());
    }

    #[test]
    fn immediate_move_replaces_active_order_for_all_selected_units() {
        let config = PlannerConfig::default();
        let mut first = unit(1);
        first.queue_len = 3;
        let second = unit(2);
        let request = OrderRequest {
            units: vec![1, 2],
            mode: IssueMode::Immediate,
            order: RequestedOrder::Move {
                to: Point::new(256.0, 128.0),
            },
        };

        let out = plan_order(config, &[first, second], &request);

        assert_eq!(replace_units(&out), vec![1, 2]);
        assert!(out.notices.is_empty());
    }

    #[test]
    fn queued_charge_then_attack_move_stacks_for_ready_carriers_and_whole_group() {
        let config = PlannerConfig::default();
        let rifle = with_ability(unit(1), CHARGE, true);
        let tank = unit(2);
        let facts = vec![rifle, tank];
        let charge = OrderRequest {
            units: vec![1, 2],
            mode: IssueMode::Queue,
            order: RequestedOrder::UseAbility {
                ability: CHARGE,
                target: AbilityTarget::SelfTarget,
            },
        };

        let charge_plan = plan_order(config, &facts, &charge);
        let attack_plan = plan_order(config, &facts, &attack_move(&[1, 2], IssueMode::Queue));

        assert_eq!(queued_units(&charge_plan), vec![1]);
        assert_eq!(queued_units(&attack_plan), vec![1, 2]);
    }
}
