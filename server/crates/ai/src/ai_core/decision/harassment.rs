use super::geometry::{clamp_to_map, dist2, normalized_direction, squared, tile_center};
use super::raids::{enemy_main_steel_center, group_center};
use super::*;

const HARASSMENT_FLANK_PROGRESS_EPS_TILES: f32 = 3.0;
const HARASSMENT_EVASION_EXTRA_TILES: f32 = 6.0;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct HarassmentPlan {
    pub(super) reserved_units: Vec<u32>,
    pub(super) move_target: Option<(f32, f32)>,
    pub(super) flank_target: Option<(f32, f32)>,
    pub(super) evasion_target: Option<(f32, f32)>,
    pub(super) attack_due: bool,
}

impl HarassmentPlan {
    pub(super) fn inactive() -> Self {
        Self {
            reserved_units: Vec::new(),
            move_target: None,
            flank_target: None,
            evasion_target: None,
            attack_due: false,
        }
    }

    pub(super) fn should_issue(&self) -> bool {
        !self.reserved_units.is_empty()
            && (self.evasion_target.is_some() || (self.attack_due && self.move_target.is_some()))
    }
}

pub(super) fn plan_scout_car_harassment(
    observation: &AiObservation,
    profile: &AiProfile,
    memory: &mut AiDecisionMemory,
    enemy_base: Option<EnemyBaseFact>,
) -> HarassmentPlan {
    let Some(policy) = profile.harassment else {
        return HarassmentPlan::inactive();
    };
    if policy.unit_kind != EntityKind::ScoutCar || policy.group_size == 0 {
        return HarassmentPlan::inactive();
    }

    let reserved_units = select_harassment_units(observation, policy);
    if reserved_units.is_empty() {
        return HarassmentPlan::inactive();
    }

    let evasion_target =
        harassment_evasion_target(observation, policy, &reserved_units, enemy_base);
    let route = enemy_base.map(|enemy_base| scout_car_harassment_route(observation, policy, enemy_base));
    let attack_due = memory.harassment_due_for(profile, observation.tick);
    HarassmentPlan {
        reserved_units,
        move_target: route.map(|route| route.final_target),
        flank_target: route.map(|route| route.flank_target),
        evasion_target,
        attack_due,
    }
}

pub(super) fn issue_scout_car_harassment(
    actions: &mut AiActionContext<'_>,
    memory: &mut AiDecisionMemory,
    profile: &AiProfile,
    observation: &AiObservation,
    plan: &HarassmentPlan,
) -> Option<AiIntent> {
    if !plan.should_issue() {
        return None;
    }
    if let Some((x, y)) = plan.evasion_target {
        let units = actions::move_units(actions, plan.reserved_units.clone(), x, y)?;
        memory.note_harassment_for(profile, observation.tick);
        return Some(AiIntent::Move { units });
    }
    let (x, y) = plan.move_target?;
    let units = if let Some((fx, fy)) = plan.flank_target.filter(|flank| {
        harassment_needs_flank_stage(observation, &plan.reserved_units, *flank)
    }) {
        let units = actions::move_units(actions, plan.reserved_units.clone(), fx, fy)?;
        actions::move_units_with_queue(actions, units.clone(), x, y, true)?;
        units
    } else {
        actions::move_units(actions, plan.reserved_units.clone(), x, y)?
    };
    memory.note_harassment_for(profile, observation.tick);
    Some(AiIntent::Move { units })
}

pub(super) fn select_harassment_units(
    observation: &AiObservation,
    policy: crate::ai_core::profiles::HarassmentPolicy,
) -> Vec<u32> {
    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    let mut candidates: Vec<(u8, f32, u32)> = observation
        .owned
        .iter()
        .filter(|entity| entity.kind == policy.unit_kind && entity.is_complete)
        .filter(|entity| {
            entity.free_for_combat
                || matches!(entity.state, AiEntityState::Move | AiEntityState::Attack)
        })
        .map(|entity| {
            let committed = matches!(entity.state, AiEntityState::Move | AiEntityState::Attack);
            (
                if committed { 0 } else { 1 },
                dist2(entity.x, entity.y, own_base.0, own_base.1),
                entity.id,
            )
        })
        .collect();
    candidates.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| right.1.total_cmp(&left.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    candidates
        .into_iter()
        .take(policy.group_size)
        .map(|(_, _, id)| id)
        .collect()
}

pub(super) fn scout_car_harassment_move_target(
    observation: &AiObservation,
    policy: crate::ai_core::profiles::HarassmentPolicy,
    enemy_base: EnemyBaseFact,
) -> (f32, f32) {
    scout_car_harassment_route(observation, policy, enemy_base).final_target
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ScoutCarHarassmentRoute {
    flank_target: (f32, f32),
    final_target: (f32, f32),
}

fn scout_car_harassment_route(
    observation: &AiObservation,
    policy: crate::ai_core::profiles::HarassmentPolicy,
    enemy_base: EnemyBaseFact,
) -> ScoutCarHarassmentRoute {
    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    let resource_center =
        enemy_main_steel_center(observation, enemy_base).unwrap_or((enemy_base.x, enemy_base.y));
    let dx = enemy_base.x - own_base.0;
    let dy = enemy_base.y - own_base.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= f32::EPSILON || !len.is_finite() {
        return ScoutCarHarassmentRoute {
            flank_target: resource_center,
            final_target: resource_center,
        };
    }
    let forward = (dx / len, dy / len);
    let perp = (-forward.1, forward.0);
    let map_center = (
        observation.map.width as f32 * observation.map.tile_size as f32 * 0.5,
        observation.map.height as f32 * observation.map.tile_size as f32 * 0.5,
    );
    let side_sign = if dist2(
        resource_center.0 + perp.0,
        resource_center.1 + perp.1,
        map_center.0,
        map_center.1,
    ) >= dist2(
        resource_center.0 - perp.0,
        resource_center.1 - perp.1,
        map_center.0,
        map_center.1,
    ) {
        1.0
    } else {
        -1.0
    };
    let tile_size = observation.map.tile_size as f32;
    let side_offset = policy.side_offset_tiles * tile_size;
    let final_target = clamp_to_map(
        (
            resource_center.0
                + forward.0 * policy.back_offset_tiles * tile_size
                + perp.0 * side_sign * side_offset,
            resource_center.1
                + forward.1 * policy.back_offset_tiles * tile_size
                + perp.1 * side_sign * side_offset,
        ),
        observation.map,
    );
    let midpoint = ((own_base.0 + enemy_base.x) * 0.5, (own_base.1 + enemy_base.y) * 0.5);
    let flank_target = clamp_to_map(
        (
            midpoint.0 + perp.0 * side_sign * side_offset * 1.75,
            midpoint.1 + perp.1 * side_sign * side_offset * 1.75,
        ),
        observation.map,
    );
    ScoutCarHarassmentRoute {
        flank_target,
        final_target,
    }
}

fn harassment_evasion_target(
    observation: &AiObservation,
    policy: crate::ai_core::profiles::HarassmentPolicy,
    unit_ids: &[u32],
    enemy_base: Option<EnemyBaseFact>,
) -> Option<(f32, f32)> {
    let center = group_center(observation, unit_ids)?;
    let radius2 = squared(policy.visible_threat_radius_tiles * observation.map.tile_size as f32);
    let (_, _, _, threat_x, threat_y) = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .map(|enemy| {
            (
                enemy.id,
                harassment_threat_priority(enemy.kind),
                dist2(center.0, center.1, enemy.x, enemy.y),
                enemy.x,
                enemy.y,
            )
        })
        .filter(|(_, _, distance2, _, _)| *distance2 <= radius2)
        .min_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.total_cmp(&right.2))
                .then_with(|| left.0.cmp(&right.0))
        })?;
    let fallback = enemy_base
        .and_then(|base| normalized_direction((base.x, base.y), center))
        .unwrap_or((1.0, 0.0));
    let away =
        normalized_direction((threat_x, threat_y), center).unwrap_or(fallback);
    let distance =
        (policy.visible_threat_radius_tiles + HARASSMENT_EVASION_EXTRA_TILES)
            * observation.map.tile_size as f32;
    Some(clamp_to_map(
        (center.0 + away.0 * distance, center.1 + away.1 * distance),
        observation.map,
    ))
}

fn harassment_needs_flank_stage(
    observation: &AiObservation,
    unit_ids: &[u32],
    flank_target: (f32, f32),
) -> bool {
    let Some(center) = group_center(observation, unit_ids) else {
        return false;
    };
    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    let Some(route_dir) = normalized_direction(own_base, flank_target) else {
        return false;
    };
    let total_dx = flank_target.0 - own_base.0;
    let total_dy = flank_target.1 - own_base.1;
    let total_progress = (total_dx * total_dx + total_dy * total_dy).sqrt();
    let progress =
        (center.0 - own_base.0) * route_dir.0 + (center.1 - own_base.1) * route_dir.1;
    let eps = HARASSMENT_FLANK_PROGRESS_EPS_TILES * observation.map.tile_size as f32;
    progress + eps < total_progress
}

fn harassment_threat_priority(kind: EntityKind) -> u8 {
    match kind {
        EntityKind::Tank => 0,
        EntityKind::AntiTankGun | EntityKind::MachineGunner => 1,
        EntityKind::ScoutCar | EntityKind::Rifleman => 2,
        _ => 3,
    }
}
