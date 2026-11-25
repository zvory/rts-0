use super::guards::unit_has_committed_barrage;
use super::*;

pub(super) fn artillery_fire_mode_for(ability: AbilityKind) -> Option<ArtilleryFireMode> {
    match ability {
        AbilityKind::PointFire => Some(ArtilleryFireMode::Point),
        AbilityKind::BlanketFire => Some(ArtilleryFireMode::Blanket),
        _ => None,
    }
}

pub(super) fn immediate_unit_can_replace(entities: &EntityStore, player: u32, unit: u32) -> bool {
    unit_can_accept_player_command(entities, player, unit)
        && !unit_has_committed_barrage(entities, unit)
}

pub(super) fn gather_node_valid(entities: &EntityStore, _player: u32, node: u32) -> bool {
    world_query::live_steel_node(entities, node)
        && entities.resource_extractor_for_node(node).is_none()
}

pub(super) fn choose_smoke_caster(
    map: &Map,
    entities: &EntityStore,
    ability: AbilityKind,
    eligible: &[u32],
    x: f32,
    y: f32,
) -> Option<u32> {
    let mut furthest_in_range: Option<(u32, f32)> = None;
    let mut closest: Option<(u32, f32)> = None;
    for id in eligible {
        let Some(e) = entities.get(*id) else {
            continue;
        };
        let d2 = dist2(e.pos_x(), e.pos_y(), x, y);
        if closest.is_none_or(|(_, best)| d2 < best) {
            closest = Some((*id, d2));
        }
        if ability_orders::caster_in_range(map, entities, *id, ability, x, y)
            && furthest_in_range.is_none_or(|(_, best)| d2 > best)
        {
            furthest_in_range = Some((*id, d2));
        }
    }
    furthest_in_range.or(closest).map(|(id, _)| id)
}

pub(super) fn clear_queued_orders(entities: &mut EntityStore, ids: &[u32]) {
    for id in ids {
        if let Some(e) = entities.get_mut(*id) {
            e.clear_queued_orders();
        }
    }
}

pub(super) fn clear_staged_anti_tank_gun_setup(entities: &mut EntityStore, ids: &[u32]) {
    for id in ids {
        let Some(e) = entities.get_mut(*id) else {
            continue;
        };
        if e.kind == EntityKind::AntiTankGun {
            e.set_emplacement_facing(None);
            e.set_pending_redeploy_facing(None);
        }
    }
}

// Dash applies to every selected caster. Singleton requests retain the normal
// planner guards without its single-caster world-ability arbitration.
pub(super) fn ability_requests(
    units: Vec<u32>,
    queued: bool,
    ability: AbilityKind,
    target: planner::AbilityTarget,
    max_units_per_command: usize,
) -> Vec<planner::OrderRequest> {
    let unit_groups = if ability.is_dash() {
        dedupe_cap_units(units, max_units_per_command)
            .into_iter()
            .map(|unit| vec![unit])
            .collect()
    } else {
        vec![units]
    };
    unit_groups
        .into_iter()
        .map(|units| planner::OrderRequest {
            units,
            mode: issue_mode(queued),
            order: planner::RequestedOrder::UseAbility {
                ability: ability_to_planner(ability),
                target,
            },
        })
        .collect()
}

pub(super) fn sanitize_formation_points(
    map: &Map,
    points: Vec<(f32, f32)>,
) -> Option<Vec<(f32, f32)>> {
    if points.len() < 2 || points.len() > protocol::MAX_FORMATION_POINTS {
        return None;
    }
    let max_x = map.world_width_px() - 1.0;
    let max_y = map.world_height_px() - 1.0;
    let min_distance_sq = MIN_FORMATION_POINT_DISTANCE_PX * MIN_FORMATION_POINT_DISTANCE_PX;
    let mut sanitized = Vec::with_capacity(points.len());
    for (x, y) in points {
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        let point = (x.clamp(0.0, max_x), y.clamp(0.0, max_y));
        if sanitized.last().is_some_and(|last: &(f32, f32)| {
            let dx = point.0 - last.0;
            let dy = point.1 - last.1;
            dx * dx + dy * dy < min_distance_sq
        }) {
            continue;
        }
        sanitized.push(point);
    }
    (sanitized.len() >= 2).then_some(sanitized)
}
