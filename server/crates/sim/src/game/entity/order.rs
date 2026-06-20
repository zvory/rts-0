use crate::game::ability::AbilityKind;

use super::EntityKind;

/// Maximum number of queued command intents stored on one mobile unit.
pub const MAX_QUEUED_ORDERS: usize = 8;

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
    /// Stand ground without chasing or walking to auto-acquire; fire only at enemies already in
    /// weapon range.
    HoldPosition,
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
    /// Walk to a completed Tank Trap and dismantle it for the issuing player's refund.
    Deconstruct(DeconstructOrder),
    /// Move into range of a world-targeted ability, then execute it at the target point.
    Ability(AbilityOrder),
    /// Artillery repeats point fire at a fixed world position until interrupted.
    ArtilleryPointFire(ArtilleryPointFireOrder),
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

    pub fn deconstruct(target: u32) -> Self {
        Order::Deconstruct(DeconstructOrder::new(target))
    }

    pub fn ability(ability: AbilityKind, x: f32, y: f32, staging_x: f32, staging_y: f32) -> Self {
        Order::Ability(AbilityOrder::new(ability, x, y, staging_x, staging_y))
    }

    pub fn artillery_point_fire(x: f32, y: f32) -> Self {
        Order::ArtilleryPointFire(ArtilleryPointFireOrder::new(x, y))
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

    pub fn deconstruct_target(&self) -> Option<u32> {
        match self {
            Order::Deconstruct(order) => Some(order.intent.target),
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

/// Lightweight future order intent. Unlike [`Order`], this stores no execution phase, path,
/// progress, or target latch state.
#[derive(Debug, Clone, PartialEq)]
pub enum OrderIntent {
    Move(PointIntent),
    AttackMove(PointIntent),
    Attack(TargetIntent),
    Gather(GatherIntent),
    Build(BuildIntent),
    Deconstruct(TargetIntent),
    WorldAbility(AbilityIntent),
    SelfAbility(SelfAbilityIntent),
    SetupAntiTankGuns(PointIntent),
    PointFire(PointIntent),
}

impl OrderIntent {
    pub fn move_to(x: f32, y: f32) -> Self {
        OrderIntent::Move(PointIntent { x, y })
    }

    pub fn attack_move_to(x: f32, y: f32) -> Self {
        OrderIntent::AttackMove(PointIntent { x, y })
    }

    pub fn attack(target: u32) -> Self {
        OrderIntent::Attack(TargetIntent { target })
    }

    pub fn gather(node: u32) -> Self {
        OrderIntent::Gather(GatherIntent { node })
    }

    pub fn build(kind: EntityKind, tile_x: u32, tile_y: u32) -> Self {
        OrderIntent::Build(BuildIntent {
            kind,
            tile_x,
            tile_y,
        })
    }

    pub fn deconstruct(target: u32) -> Self {
        OrderIntent::Deconstruct(TargetIntent { target })
    }

    pub fn ability(ability: AbilityKind, x: f32, y: f32) -> Self {
        OrderIntent::WorldAbility(AbilityIntent { ability, x, y })
    }

    pub fn self_ability(ability: AbilityKind) -> Self {
        OrderIntent::SelfAbility(SelfAbilityIntent { ability })
    }

    pub fn setup_anti_tank_guns(x: f32, y: f32) -> Self {
        OrderIntent::SetupAntiTankGuns(PointIntent { x, y })
    }

    pub fn point_fire(x: f32, y: f32) -> Self {
        OrderIntent::PointFire(PointIntent { x, y })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointIntent {
    pub x: f32,
    pub y: f32,
}

/// Future order intent applied to units as they leave a production building.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RallyKind {
    Move,
    AttackMove,
}

impl RallyKind {
    pub fn from_protocol_str(kind: Option<&str>) -> Option<Self> {
        match kind.unwrap_or("move") {
            "move" => Some(RallyKind::Move),
            "attackMove" => Some(RallyKind::AttackMove),
            _ => None,
        }
    }

    pub fn to_protocol_str(self) -> &'static str {
        match self {
            RallyKind::Move => "move",
            RallyKind::AttackMove => "attackMove",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RallyIntent {
    pub kind: RallyKind,
    pub point: PointIntent,
}

impl RallyIntent {
    pub fn new(kind: RallyKind, x: f32, y: f32) -> Self {
        RallyIntent {
            kind,
            point: PointIntent { x, y },
        }
    }

    pub fn to_order_intent(self) -> OrderIntent {
        match self.kind {
            RallyKind::Move => OrderIntent::Move(self.point),
            RallyKind::AttackMove => OrderIntent::AttackMove(self.point),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbilityIntent {
    pub ability: AbilityKind,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelfAbilityIntent {
    pub ability: AbilityKind,
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
                unreachable_checks: 0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackExecution {
    pub phase: AttackPhase,
    /// Consecutive failed chase-path checks while the target could not be fired on.
    /// Bounded queue promotion uses this to skip stale/unreachable explicit attacks.
    pub unreachable_checks: u16,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeconstructOrder {
    pub intent: TargetIntent,
    pub execution: DeconstructExecution,
}

impl DeconstructOrder {
    fn new(target: u32) -> Self {
        DeconstructOrder {
            intent: TargetIntent { target },
            execution: DeconstructExecution {
                phase: DeconstructPhase::ToTarget,
                progress: 0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeconstructExecution {
    pub phase: DeconstructPhase,
    pub progress: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeconstructPhase {
    /// Worker is walking toward the Tank Trap.
    ToTarget,
    /// Worker is dismantling the Tank Trap.
    Deconstructing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AbilityOrder {
    pub intent: AbilityIntent,
    pub execution: AbilityExecution,
}

impl AbilityOrder {
    fn new(ability: AbilityKind, x: f32, y: f32, staging_x: f32, staging_y: f32) -> Self {
        AbilityOrder {
            intent: AbilityIntent { ability, x, y },
            execution: AbilityExecution {
                phase: MovePhase::AwaitingPath,
                staging: PointIntent {
                    x: staging_x,
                    y: staging_y,
                },
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbilityExecution {
    pub phase: MovePhase,
    pub staging: PointIntent,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtilleryPointFireOrder {
    pub intent: PointIntent,
}

impl ArtilleryPointFireOrder {
    fn new(x: f32, y: f32) -> Self {
        ArtilleryPointFireOrder {
            intent: PointIntent { x, y },
        }
    }
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
