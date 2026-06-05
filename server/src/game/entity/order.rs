use super::EntityKind;

/// The high-level order a unit/building is currently executing.
///
/// Orders drive the per-tick systems. Buildings only ever sit in [`Order::Idle`]; their
/// activity (production, construction) is tracked by their dedicated fields. Each active order
/// keeps immutable intent separate from execution phase, so systems transition explicit state
/// machines instead of smuggling progress through unrelated fields.
#[derive(Debug, Clone, PartialEq)]
pub enum Order {
    /// No order: units hold position; idle army units and armed buildings may auto-acquire,
    /// but workers stay passive unless explicitly ordered.
    Idle,
    /// Move to a world point; stop on arrival. No engaging en route.
    Move(MoveOrder),
    /// Move to a world point while engaging enemies encountered along the way.
    AttackMove(MoveOrder),
    /// Chase and attack a specific entity until it dies, then go idle.
    Attack(AttackOrder),
    /// Harvest from a resource node, depositing each completed load directly into the economy.
    Gather(GatherOrder),
    /// Walk to a target tile and construct a building of `kind` there. The building does
    /// not exist until the worker arrives, re-validates placement/affordability, and pays
    /// the cost; until then the order carries only the intent (kind + top-left tile).
    Build(BuildOrder),
}

impl Order {
    pub fn move_to(x: f32, y: f32) -> Self {
        Order::Move(MoveOrder::new(x, y))
    }

    pub fn attack_move_to(x: f32, y: f32) -> Self {
        Order::AttackMove(MoveOrder::new(x, y))
    }

    pub fn attack(target: u32) -> Self {
        Order::Attack(AttackOrder::new(target))
    }

    pub fn gather(node: u32) -> Self {
        Order::Gather(GatherOrder::new(node))
    }

    pub fn build(kind: EntityKind, tile_x: u32, tile_y: u32) -> Self {
        Order::Build(BuildOrder::new(kind, tile_x, tile_y))
    }

    pub fn attack_target(&self) -> Option<u32> {
        match self {
            Order::Attack(order) => Some(order.intent.target),
            _ => None,
        }
    }

    pub fn gather_node(&self) -> Option<u32> {
        match self {
            Order::Gather(order) => Some(order.intent.node),
            _ => None,
        }
    }

    /// The id of the building being constructed, if construction has actually begun.
    /// Returns `None` while the worker is still walking to the site.
    pub fn build_site(&self) -> Option<u32> {
        match self {
            Order::Build(order) => match order.execution.phase {
                BuildPhase::Constructing { site } => Some(site),
                BuildPhase::ToSite => None,
            },
            _ => None,
        }
    }

    /// The pending placement intent for a build order, if any: (kind, tile_x, tile_y) of
    /// the footprint's top-left tile. Available in any build phase.
    pub fn build_intent_tile(&self) -> Option<(EntityKind, u32, u32)> {
        match self {
            Order::Build(order) => {
                Some((order.intent.kind, order.intent.tile_x, order.intent.tile_y))
            }
            _ => None,
        }
    }

    pub fn gather_phase(&self) -> Option<GatherPhase> {
        match self {
            Order::Gather(order) => Some(order.execution.phase),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointIntent {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MoveOrder {
    pub intent: PointIntent,
    pub execution: MoveExecution,
}

impl MoveOrder {
    fn new(x: f32, y: f32) -> Self {
        MoveOrder {
            intent: PointIntent { x, y },
            execution: MoveExecution {
                phase: MovePhase::AwaitingPath,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveExecution {
    pub phase: MovePhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovePhase {
    AwaitingPath,
    Moving,
    Arrived,
    PathFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetIntent {
    pub target: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttackOrder {
    pub intent: TargetIntent,
    pub execution: AttackExecution,
}

impl AttackOrder {
    fn new(target: u32) -> Self {
        AttackOrder {
            intent: TargetIntent { target },
            execution: AttackExecution {
                phase: AttackPhase::Chasing,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackExecution {
    pub phase: AttackPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackPhase {
    Chasing,
    Firing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatherIntent {
    pub node: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GatherOrder {
    pub intent: GatherIntent,
    pub execution: GatherExecution,
}

impl GatherOrder {
    fn new(node: u32) -> Self {
        GatherOrder {
            intent: GatherIntent { node },
            execution: GatherExecution {
                phase: GatherPhase::ToNode,
                harvest_progress: 0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatherExecution {
    pub phase: GatherPhase,
    pub harvest_progress: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildIntent {
    pub kind: EntityKind,
    pub tile_x: u32,
    pub tile_y: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildOrder {
    pub intent: BuildIntent,
    pub execution: BuildExecution,
}

impl BuildOrder {
    fn new(kind: EntityKind, tile_x: u32, tile_y: u32) -> Self {
        BuildOrder {
            intent: BuildIntent {
                kind,
                tile_x,
                tile_y,
            },
            execution: BuildExecution {
                phase: BuildPhase::ToSite,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildExecution {
    pub phase: BuildPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildPhase {
    /// Worker is walking toward the target tile. No building has been spawned and no
    /// resources have been deducted yet.
    ToSite,
    /// Worker has arrived, the building has been spawned in CONSTRUCT state, and
    /// construction is progressing. `site` is the building entity id.
    Constructing { site: u32 },
}

/// The phase a gathering worker is in. Kept inside [`GatherOrder`] so the order's intent
/// (which node) stays stable while the worker's execution cycles through phases.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatherPhase {
    /// Walking out to the resource node.
    ToNode,
    /// Standing on the node, accumulating harvest ticks.
    Harvesting,
    /// Walking back to the home City Centre with a load.
    ToHome,
}
