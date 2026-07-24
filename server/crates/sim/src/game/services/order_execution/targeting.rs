use crate::config;
use crate::game::entity::{Entity, EntityStore, Order, WeaponSetup};
use crate::game::map::Map;

#[derive(Clone, Copy)]
pub(crate) struct ArtilleryPointFireTarget {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) facing: f32,
    pub(crate) inside_field_of_fire: bool,
    pub(crate) in_range: bool,
}

#[derive(Clone, Copy)]
pub(crate) enum ArtilleryPointFireAcceptance {
    BasicTarget,
    Command,
    QueuedCommand,
}

pub(crate) fn artillery_point_fire_target(
    map: &Map,
    entities: &EntityStore,
    player: u32,
    unit: u32,
    x: f32,
    y: f32,
    acceptance: ArtilleryPointFireAcceptance,
) -> Option<ArtilleryPointFireTarget> {
    artillery_point_fire_target_from_context(ArtilleryPointFireTargetRequest {
        map,
        entities,
        player,
        unit,
        x,
        y,
        acceptance,
        context_for: current_artillery_target_context,
        require_stationary: false,
        interpretation: FireTargetInterpretation::RawClick,
    })
}

pub(crate) fn queued_artillery_point_fire_target(
    map: &Map,
    entities: &EntityStore,
    player: u32,
    unit: u32,
    x: f32,
    y: f32,
) -> Option<ArtilleryPointFireTarget> {
    let e = entities.get(unit)?;
    if artillery_point_fire_queue_terminal(e) {
        return None;
    }
    artillery_point_fire_target_from_context(ArtilleryPointFireTargetRequest {
        map,
        entities,
        player,
        unit,
        x,
        y,
        acceptance: ArtilleryPointFireAcceptance::QueuedCommand,
        context_for: queued_artillery_target_context,
        require_stationary: false,
        interpretation: FireTargetInterpretation::RawClick,
    })
}

pub(crate) fn stored_artillery_point_fire_target(
    map: &Map,
    entities: &EntityStore,
    player: u32,
    unit: u32,
    x: f32,
    y: f32,
    acceptance: ArtilleryPointFireAcceptance,
) -> Option<ArtilleryPointFireTarget> {
    artillery_point_fire_target_from_context(ArtilleryPointFireTargetRequest {
        map,
        entities,
        player,
        unit,
        x,
        y,
        acceptance,
        context_for: current_artillery_target_context,
        require_stationary: true,
        interpretation: FireTargetInterpretation::StoredEffectivePoint,
    })
}

#[derive(Clone, Copy)]
struct ArtilleryTargetContext {
    origin_x: f32,
    origin_y: f32,
    setup_facing: Option<f32>,
}

#[derive(Clone, Copy)]
enum FireTargetInterpretation {
    RawClick,
    StoredEffectivePoint,
}

struct ArtilleryPointFireTargetRequest<'a> {
    map: &'a Map,
    entities: &'a EntityStore,
    player: u32,
    unit: u32,
    x: f32,
    y: f32,
    acceptance: ArtilleryPointFireAcceptance,
    context_for: fn(&Entity) -> ArtilleryTargetContext,
    require_stationary: bool,
    interpretation: FireTargetInterpretation,
}

fn artillery_point_fire_target_from_context(
    request: ArtilleryPointFireTargetRequest<'_>,
) -> Option<ArtilleryPointFireTarget> {
    let ArtilleryPointFireTargetRequest {
        map,
        entities,
        player,
        unit,
        x,
        y,
        acceptance,
        context_for,
        require_stationary,
        interpretation,
    } = request;
    let e = entities.get(unit)?;
    if e.owner != player
        || !super::is_artillery_entity(e)
        || e.hp == 0
        || e.under_construction()
        || (require_stationary && !e.path_is_empty())
    {
        return None;
    }
    if matches!(acceptance, ArtilleryPointFireAcceptance::Command)
        && !artillery_can_accept_point_fire_command(e)
    {
        return None;
    }
    if matches!(acceptance, ArtilleryPointFireAcceptance::QueuedCommand)
        && !artillery_can_accept_queued_point_fire_command(e)
    {
        return None;
    }
    let context = context_for(e);
    let min_px = config::ARTILLERY_MIN_RANGE_TILES as f32 * config::TILE_SIZE as f32;
    let max_px = config::ARTILLERY_MAX_RANGE_TILES as f32 * config::TILE_SIZE as f32;
    let (target, in_range) = match interpretation {
        FireTargetInterpretation::RawClick => {
            if !point_inside_playable_map(map.world_size_px(), x, y) {
                return None;
            }
            let facing = (y - context.origin_y).atan2(x - context.origin_x);
            if !facing.is_finite() {
                return None;
            }
            let distance = (x - context.origin_x).hypot(y - context.origin_y);
            (
                LockedArtilleryFireTarget { x, y, facing },
                distance.is_finite() && distance >= min_px && distance <= max_px,
            )
        }
        FireTargetInterpretation::StoredEffectivePoint => stored_artillery_fire_target(
            map.world_size_px(),
            (context.origin_x, context.origin_y),
            min_px,
            max_px,
            (x, y),
        )
        .map(|target| (target, true))?,
    };
    let inside_field_of_fire =
        artillery_target_inside_field_of_fire(e, target.facing, context.setup_facing);
    Some(ArtilleryPointFireTarget {
        x: target.x,
        y: target.y,
        facing: target.facing,
        inside_field_of_fire,
        in_range,
    })
}

#[derive(Clone, Copy)]
struct LockedArtilleryFireTarget {
    x: f32,
    y: f32,
    facing: f32,
}

fn stored_artillery_fire_target(
    world_size_px: f32,
    origin: (f32, f32),
    min_range_px: f32,
    max_range_px: f32,
    target: (f32, f32),
) -> Option<LockedArtilleryFireTarget> {
    if !point_inside_playable_map(world_size_px, target.0, target.1)
        || !origin.0.is_finite()
        || !origin.1.is_finite()
        || !min_range_px.is_finite()
        || !max_range_px.is_finite()
        || min_range_px < 0.0
        || max_range_px < min_range_px
    {
        return None;
    }
    let dx = target.0 - origin.0;
    let dy = target.1 - origin.1;
    let distance2 = dx * dx + dy * dy;
    if !distance2.is_finite() {
        return None;
    }
    let min2 = min_range_px * min_range_px;
    let max2 = max_range_px * max_range_px;
    let slack = 0.5;
    if distance2 + slack < min2 || distance2 > max2 + slack {
        return None;
    }
    let facing = dy.atan2(dx);
    if !facing.is_finite() {
        return None;
    }
    Some(LockedArtilleryFireTarget {
        x: target.0,
        y: target.1,
        facing,
    })
}

fn point_inside_playable_map(world_size_px: f32, x: f32, y: f32) -> bool {
    if !world_size_px.is_finite() || world_size_px <= 0.0 || !x.is_finite() || !y.is_finite() {
        return false;
    }
    let max = (world_size_px - 1.0).max(0.0);
    x >= 0.0 && y >= 0.0 && x <= max && y <= max
}

fn angle_delta(a: f32, b: f32) -> f32 {
    let mut d = (a - b).rem_euclid(std::f32::consts::TAU);
    if d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    d
}

fn artillery_can_accept_point_fire_command(e: &Entity) -> bool {
    matches!(
        e.weapon_setup(),
        WeaponSetup::Packed
            | WeaponSetup::SettingUp { .. }
            | WeaponSetup::Deployed
            | WeaponSetup::TearingDown { .. }
            | WeaponSetup::TearingDownToRedeploy { .. }
    )
}

fn artillery_can_accept_queued_point_fire_command(e: &Entity) -> bool {
    artillery_can_accept_point_fire_command(e)
        || (matches!(e.weapon_setup(), WeaponSetup::TearingDown { .. })
            && e.move_intent().is_some())
}

fn artillery_point_fire_queue_terminal(e: &Entity) -> bool {
    matches!(
        e.order(),
        Order::ArtilleryPointFire(_) | Order::ArtilleryBlanketFire { .. }
    ) || e.queued_orders().iter().any(|intent| {
        matches!(
            intent,
            crate::game::entity::OrderIntent::PointFire(_)
                | crate::game::entity::OrderIntent::BlanketFire { .. }
        )
    })
}

fn current_artillery_target_context(e: &Entity) -> ArtilleryTargetContext {
    ArtilleryTargetContext {
        origin_x: e.pos_x,
        origin_y: e.pos_y,
        setup_facing: artillery_point_fire_field_center(e),
    }
}

fn queued_artillery_target_context(e: &Entity) -> ArtilleryTargetContext {
    let mut context = current_artillery_target_context(e);
    if let Some((x, y)) = e
        .move_intent()
        .filter(|(x, y)| x.is_finite() && y.is_finite())
    {
        context.origin_x = x;
        context.origin_y = y;
        context.setup_facing = None;
    }
    for intent in e.queued_orders() {
        match intent {
            crate::game::entity::OrderIntent::Move(point)
            | crate::game::entity::OrderIntent::AttackMove(point) => {
                if point.x.is_finite() && point.y.is_finite() {
                    context.origin_x = point.x;
                    context.origin_y = point.y;
                    context.setup_facing = None;
                }
            }
            crate::game::entity::OrderIntent::SetupAntiTankGuns(point) => {
                let facing = (point.y - context.origin_y).atan2(point.x - context.origin_x);
                if facing.is_finite() {
                    context.setup_facing = Some(facing);
                }
            }
            crate::game::entity::OrderIntent::PointFire(_)
            | crate::game::entity::OrderIntent::BlanketFire { .. } => break,
            _ => {}
        }
    }
    context
}

fn artillery_point_fire_field_center(e: &Entity) -> Option<f32> {
    match e.weapon_setup() {
        WeaponSetup::TearingDownToRedeploy { .. } => e.pending_redeploy_facing(),
        WeaponSetup::Packed | WeaponSetup::SettingUp { .. } => e.emplacement_facing(),
        _ => e.emplacement_facing().or_else(|| e.weapon_facing()),
    }
}

fn artillery_target_inside_field_of_fire(
    e: &Entity,
    target_facing: f32,
    planned_facing: Option<f32>,
) -> bool {
    let center = match e.weapon_setup() {
        WeaponSetup::Deployed => artillery_point_fire_field_center(e),
        WeaponSetup::Packed | WeaponSetup::SettingUp { .. } => planned_facing,
        WeaponSetup::TearingDownToRedeploy { .. } => e.pending_redeploy_facing(),
        WeaponSetup::TearingDown { .. } => None,
    };
    center
        .filter(|facing| facing.is_finite())
        .is_some_and(|center| {
            angle_delta(center, target_facing).abs() <= config::ARTILLERY_FIELD_OF_FIRE_RAD * 0.5
        })
}

#[cfg(test)]
mod tests;
