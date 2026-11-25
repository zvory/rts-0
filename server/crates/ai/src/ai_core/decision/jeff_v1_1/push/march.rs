//! The push's march orders: whether it drives into a base (`goin`), its fighting ranks
//! (`wideline`), the Tank convoys that join it (`tankgroup`), its legs and holds, and the anchors
//! it steps between.

use super::*;

/// Whether the push drives straight into an enemy base (`goin`).
pub(in crate::ai_core::decision) fn going_in(memory: &AiDecisionMemory) -> bool {
    v1_1(memory) && memory.jeff_v1_1.push.going_in
}

/// `wideline`: the push's fighting ranks (not its travel column) are three quarters of its Tanks
/// wide, 2 tiles apart, not six abreast 1.5 tiles apart: sixteen Tanks stood three deep, and
/// moving Tanks reach 5 tiles, so only the front rank fired. Returns the rank width and spacing.
pub(in crate::ai_core::decision) fn wide_ranks(
    memory: &AiDecisionMemory,
    tanks: usize,
) -> Option<(usize, f32)> {
    (v1_1(memory) && !super::off("wideline"))
        .then(|| ((tanks * 3).div_ceil(4).max(6), WIDE_SPACING_TILES))
}

/// `tankgroup`: while the push is out, free Tanks leave home only `CONVOY_TANKS` or more at once,
/// beyond the `keep_home` the home keeps, and drive together to the push, joining it once near it.
/// Returns the convoy, which other systems leave be. On Classic against Pressure AI 12 Tanks
/// waited at the main's exit at 8:00 beside a 2-Tank push.
pub(in crate::ai_core::decision) fn convoy(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    held: &std::collections::BTreeSet<u32>,
    keep_home: usize,
) -> std::collections::BTreeSet<u32> {
    let launched = memory.containment.wave_launched && !memory.containment.recovery_active;
    let tanks: Vec<u32> = memory.containment.active_tanks.iter().copied().collect();
    let center = super::super::frontal::group_center(observation, &tanks);
    let Some(center) = center.filter(|_| launched && v1_1(memory) && !super::off("tankgroup"))
    else {
        memory.jeff_v1_1.push.convoy.clear();
        return Default::default();
    };
    let ts = observation.map.tile_size as f32;
    let alive: std::collections::BTreeSet<u32> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && unit.hp > 0)
        .map(|unit| unit.id)
        .collect();
    memory.jeff_v1_1.push.convoy.retain(|id| alive.contains(id));
    // Those that reached the push join it.
    let arrived: Vec<u32> = observation
        .owned
        .iter()
        .filter(|unit| memory.jeff_v1_1.push.convoy.contains(&unit.id))
        .filter(|unit| dist2(unit.x, unit.y, center.0, center.1) <= squared(CONVOY_JOIN_TILES * ts))
        .map(|unit| unit.id)
        .collect();
    for id in &arrived {
        memory.jeff_v1_1.push.convoy.remove(id);
        memory.containment.active_tanks.insert(*id);
    }
    let reserved = memory.squad_reserved();
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let mut free: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| {
            unit.kind == EntityKind::Tank
                && unit.is_complete
                && unit.hp > 0
                && unit.free_for_combat
                && Some(unit.id) != memory.home_defensive_tank
                && !memory.containment.active_tanks.contains(&unit.id)
                && !memory.containment.stuck_tanks.contains_key(&unit.id)
                && !memory.later_bases.guards.contains(&unit.id)
                && !memory.third_pressure.is_reserved(unit.id)
                && !memory.jeff_v1_1.push.convoy.contains(&unit.id)
                && !reserved.contains(&unit.id)
                && !held.contains(&unit.id)
        })
        .collect();
    // The home keeps the nearest to the main.
    free.sort_by(|a, b| {
        dist2(a.x, a.y, home.0, home.1)
            .total_cmp(&dist2(b.x, b.y, home.0, home.1))
            .then(a.id.cmp(&b.id))
    });
    let spare: Vec<u32> = free.iter().skip(keep_home).map(|unit| unit.id).collect();
    let fresh = memory.jeff_v1_1.push.convoy.is_empty() && spare.len() >= CONVOY_TANKS;
    if spare.len() >= CONVOY_TANKS {
        memory.jeff_v1_1.push.convoy.extend(spare);
        memory.jeff_v1_1.push.convoy_ordered = 0;
    }
    let tick = observation.tick;
    let members: Vec<u32> = memory.jeff_v1_1.push.convoy.iter().copied().collect();
    let due = !members.is_empty()
        && (memory.jeff_v1_1.push.convoy_ordered == 0
            || tick.saturating_sub(memory.jeff_v1_1.push.convoy_ordered) >= CONVOY_REORDER_TICKS);
    // `convoyway`: on Schone Tage's southern way each Tank goes by the way from where it is.
    let own_way: Vec<(u32, (f32, f32))> = observation
        .owned
        .iter()
        .filter(|unit| members.contains(&unit.id))
        .filter_map(|unit| {
            super::south_route::convoy_member_waypoint(
                observation,
                memory,
                (unit.x, unit.y),
                center,
            )
            .map(|target| (unit.id, target))
        })
        .collect();
    if due && !own_way.is_empty() {
        for (id, target) in own_way {
            actions::attack_move_units(actions, [id], target.0, target.1);
        }
        memory.jeff_v1_1.push.convoy_ordered = tick.max(1);
    } else if due {
        // `southroute`: on Schone Tage along the southern way up to where the push is.
        let target = super::super::frontal::group_center(observation, &members)
            .map(|at| super::south_route::convoy_waypoint(observation, memory, at, center, fresh))
            .unwrap_or(center);
        actions::attack_move_units(actions, members.iter().copied(), target.0, target.1);
        memory.jeff_v1_1.push.convoy_ordered = tick.max(1);
    }
    memory.jeff_v1_1.push.convoy.clone()
}

/// The fewest Tanks that march in legs: V1.1 takes a side lane with two.
pub(in crate::ai_core::decision) fn legs_min_tanks(
    memory: &AiDecisionMemory,
    default: usize,
) -> usize {
    if v1_1(memory)
        && memory
            .jeff_v1_1
            .push
            .target
            .is_some_and(|target| target.side.is_some())
    {
        2
    } else {
        default
    }
}

/// Whether V1.1's push holds short of its target, at an anchor or a strike, where the march
/// watchdog must not drive it on.
pub(in crate::ai_core::decision) fn holding_short(memory: &AiDecisionMemory) -> bool {
    v1_1(memory) && memory.jeff_v1_1.push.short
}

/// The anchors along the enemy's route toward `last`, nearest Jeff first, each moved onto a map
/// choke near it.
pub(super) fn anchors(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    own_base: (f32, f32),
    last: (f32, f32),
) -> Vec<(f32, f32)> {
    let ts = observation.map.tile_size as f32;
    let enemy = enemy_player(observation);
    let route = enemy
        .and_then(|enemy| map_analysis.and_then(|analysis| analysis.base_route_tiles(enemy.id)));
    let chokes: Vec<(f32, f32)> = map_analysis
        .map(|analysis| {
            analysis
                .chokes()
                .iter()
                .map(|choke| {
                    tile_center(
                        (choke.center_tile.x, choke.center_tile.y),
                        observation.map.tile_size,
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let limit = dist2(last.0, last.1, own_base.0, own_base.1);
    ANCHOR_SHARES
        .iter()
        .map(|share| match route.filter(|route| !route.is_empty()) {
            Some(route) => {
                let index = ((route.len() - 1) as f32 * share).round() as usize;
                tile_center((route[index].x, route[index].y), observation.map.tile_size)
            }
            None => (
                own_base.0 + (last.0 - own_base.0) * share,
                own_base.1 + (last.1 - own_base.1) * share,
            ),
        })
        .map(|at| {
            chokes
                .iter()
                .filter(|choke| {
                    dist2(choke.0, choke.1, at.0, at.1) <= squared(CHOKE_SNAP_TILES * ts)
                })
                .min_by(|a, b| dist2(a.0, a.1, at.0, at.1).total_cmp(&dist2(b.0, b.1, at.0, at.1)))
                .copied()
                .unwrap_or(at)
        })
        .filter(|at| dist2(at.0, at.1, own_base.0, own_base.1) < limit)
        .collect()
}
