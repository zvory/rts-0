use super::geometry::{clamp_to_map, dist2, squared, tile_center};
use super::raids::{enemy_main_steel_center, group_center};
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct HarassmentPlan {
    pub(super) reserved_units: Vec<u32>,
    pub(super) move_target: Option<(f32, f32)>,
    pub(super) visible_threat: Option<u32>,
    pub(super) attack_due: bool,
}

impl HarassmentPlan {
    pub(super) fn inactive() -> Self {
        Self {
            reserved_units: Vec::new(),
            move_target: None,
            visible_threat: None,
            attack_due: false,
        }
    }

    pub(super) fn should_issue(&self) -> bool {
        !self.reserved_units.is_empty()
            && (self.visible_threat.is_some() || (self.attack_due && self.move_target.is_some()))
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

    let visible_threat = visible_threat_for_harassment(observation, policy, &reserved_units);
    let move_target = enemy_base
        .map(|enemy_base| scout_car_harassment_move_target(observation, policy, enemy_base));
    let attack_due = memory.harassment_due_for(profile, observation.tick);
    HarassmentPlan {
        reserved_units,
        move_target,
        visible_threat,
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
    if let Some(target) = plan.visible_threat {
        let units = actions::attack_units(actions, plan.reserved_units.clone(), target)?;
        memory.note_harassment_for(profile, observation.tick);
        return Some(AiIntent::Attack { units });
    }
    let (x, y) = plan.move_target?;
    let units = actions::move_units(actions, plan.reserved_units.clone(), x, y)?;
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
    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    let resource_center =
        enemy_main_steel_center(observation, enemy_base).unwrap_or((enemy_base.x, enemy_base.y));
    let dx = enemy_base.x - own_base.0;
    let dy = enemy_base.y - own_base.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= f32::EPSILON || !len.is_finite() {
        return resource_center;
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
    clamp_to_map(
        (
            resource_center.0
                + forward.0 * policy.back_offset_tiles * tile_size
                + perp.0 * side_sign * policy.side_offset_tiles * tile_size,
            resource_center.1
                + forward.1 * policy.back_offset_tiles * tile_size
                + perp.1 * side_sign * policy.side_offset_tiles * tile_size,
        ),
        observation.map,
    )
}

fn visible_threat_for_harassment(
    observation: &AiObservation,
    policy: crate::ai_core::profiles::HarassmentPolicy,
    unit_ids: &[u32],
) -> Option<u32> {
    let center = group_center(observation, unit_ids)?;
    let radius2 = squared(policy.visible_threat_radius_tiles * observation.map.tile_size as f32);
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .map(|enemy| {
            (
                enemy.id,
                harassment_threat_priority(enemy.kind),
                dist2(center.0, center.1, enemy.x, enemy.y),
            )
        })
        .filter(|(_, _, distance2)| *distance2 <= radius2)
        .min_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.total_cmp(&right.2))
                .then_with(|| left.0.cmp(&right.0))
        })
        .map(|(id, _, _)| id)
}

fn harassment_threat_priority(kind: EntityKind) -> u8 {
    match kind {
        EntityKind::Tank => 0,
        EntityKind::AntiTankGun | EntityKind::MachineGunner => 1,
        EntityKind::ScoutCar | EntityKind::Rifleman => 2,
        _ => 3,
    }
}
