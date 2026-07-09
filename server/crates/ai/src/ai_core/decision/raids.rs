use super::geometry::{dist2, squared, tile_center};
use super::resources::forward_steel_cluster_center;
use super::*;

pub(super) const RIFLE_RAID_DEEPEN_TILES: f32 = 7.0;

pub(super) const RIFLE_RAID_STEEL_LINE_RADIUS_TILES: f32 = 4.0;

pub(super) const RIFLE_RAID_RESUME_HOME_CLEARANCE_TILES: f32 = 12.0;

pub(super) const RIFLE_RAID_RESUME_REISSUE_EPS_TILES: f32 = 1.0;

pub(super) fn is_rifle_raid_policy(attack: AttackPolicy) -> bool {
    matches!(attack.unit_kinds, [EntityKind::Rifleman])
        && attack.required_unit.is_none()
        && attack.first_attack_size <= 1
        && attack.wave_growth == 0
}

pub(super) fn select_rifle_raid_units(observation: &AiObservation) -> Vec<u32> {
    let mut units: Vec<u32> = observation
        .owned
        .iter()
        .filter(|entity| entity.kind == EntityKind::Rifleman && entity.is_complete)
        .filter(|entity| {
            entity.free_for_combat
                || matches!(entity.state, AiEntityState::Move | AiEntityState::Attack)
        })
        .map(|entity| entity.id)
        .collect();
    units.sort_unstable();
    units
}

pub(super) fn rifle_raid_units_to_resume(
    observation: &AiObservation,
    raid_units: &[u32],
    move_target: (f32, f32),
) -> Vec<u32> {
    let raid_ids: BTreeSet<u32> = raid_units.iter().copied().collect();
    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    let home_clearance_px =
        RIFLE_RAID_RESUME_HOME_CLEARANCE_TILES * observation.map.tile_size as f32;
    let reissue_eps2 =
        squared(RIFLE_RAID_RESUME_REISSUE_EPS_TILES * observation.map.tile_size as f32);
    let mut units: Vec<u32> = observation
        .owned
        .iter()
        .filter(|entity| raid_ids.contains(&entity.id))
        .filter(|entity| match entity.state {
            AiEntityState::Move | AiEntityState::Attack => true,
            AiEntityState::Idle => {
                entity.free_for_combat
                    && projected_raid_progress_px(own_base, move_target, (entity.x, entity.y))
                        > home_clearance_px
                    && dist2(entity.x, entity.y, move_target.0, move_target.1) > reissue_eps2
            }
            _ => false,
        })
        .map(|entity| entity.id)
        .collect();
    units.sort_unstable();
    units
}

pub(super) fn projected_raid_progress_px(
    from: (f32, f32),
    to: (f32, f32),
    point: (f32, f32),
) -> f32 {
    let vx = to.0 - from.0;
    let vy = to.1 - from.1;
    let len = (vx * vx + vy * vy).sqrt();
    if len <= f32::EPSILON || !len.is_finite() {
        return 0.0;
    }
    let px = point.0 - from.0;
    let py = point.1 - from.1;
    (px * vx + py * vy) / len
}

pub(super) fn rifle_raid_unit_target(
    observation: &AiObservation,
    raid_units: &[u32],
    excluded_targets: &BTreeSet<u32>,
) -> Option<u32> {
    let center = group_center(observation, raid_units)?;
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| !excluded_targets.contains(&enemy.id))
        .filter(|enemy| enemy.kind.is_unit())
        .map(|enemy| {
            (
                enemy.id,
                rifle_raid_unit_priority(enemy.kind),
                dist2(center.0, center.1, enemy.x, enemy.y),
            )
        })
        .min_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.total_cmp(&right.2))
                .then_with(|| left.0.cmp(&right.0))
        })
        .map(|(id, _, _)| id)
}

pub(super) fn rifle_raid_unit_priority(kind: EntityKind) -> u8 {
    match kind {
        EntityKind::Worker => 0,
        EntityKind::Rifleman | EntityKind::MachineGunner | EntityKind::AntiTankGun => 1,
        EntityKind::Tank => 2,
        _ => 3,
    }
}

pub(super) fn rifle_raid_building_fallback_target(
    observation: &AiObservation,
    raid_units: &[u32],
    excluded_targets: &BTreeSet<u32>,
    enemy_base: EnemyBaseFact,
) -> Option<u32> {
    let raid_ids: BTreeSet<u32> = raid_units.iter().copied().collect();
    let fallback_center =
        enemy_main_steel_center(observation, enemy_base).unwrap_or((enemy_base.x, enemy_base.y));
    let radius_px = RIFLE_RAID_STEEL_LINE_RADIUS_TILES * observation.map.tile_size as f32;
    let radius2 = squared(radius_px);
    let raider_ready_to_burn_buildings = observation.owned.iter().any(|entity| {
        raid_ids.contains(&entity.id)
            && !matches!(entity.state, AiEntityState::Move)
            && dist2(entity.x, entity.y, fallback_center.0, fallback_center.1) <= radius2
    });
    if !raider_ready_to_burn_buildings {
        return None;
    }

    let center = group_center(observation, raid_units)?;
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| !excluded_targets.contains(&enemy.id))
        .filter(|enemy| enemy.kind.is_building())
        .map(|enemy| (enemy.id, dist2(center.0, center.1, enemy.x, enemy.y)))
        .min_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.0.cmp(&right.0))
        })
        .map(|(id, _)| id)
}

pub(super) fn enemy_main_steel_center(
    observation: &AiObservation,
    enemy_base: EnemyBaseFact,
) -> Option<(f32, f32)> {
    let tile_size = observation.map.tile_size as f32;
    let search_radius_px = (config::CC_RESOURCE_MAX_DIST_TILES + 0.5) * tile_size;
    let search_radius2 = squared(search_radius_px);
    forward_steel_cluster_center(
        observation.resources.iter().filter(|resource| {
            dist2(resource.x, resource.y, enemy_base.x, enemy_base.y) <= search_radius2
        }),
        (enemy_base.x, enemy_base.y),
        observation.map,
    )
}

pub(super) fn rifle_raid_move_target(
    observation: &AiObservation,
    enemy_base: EnemyBaseFact,
) -> (f32, f32) {
    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    let dx = enemy_base.x - own_base.0;
    let dy = enemy_base.y - own_base.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= f32::EPSILON || !len.is_finite() {
        return (enemy_base.x, enemy_base.y);
    }
    let deepen_px = RIFLE_RAID_DEEPEN_TILES * observation.map.tile_size as f32;
    let max = observation.map.width as f32 * observation.map.tile_size as f32 - 0.01;
    (
        (enemy_base.x + dx / len * deepen_px).clamp(0.0, max),
        (enemy_base.y + dy / len * deepen_px).clamp(0.0, max),
    )
}

pub(super) fn group_center(observation: &AiObservation, unit_ids: &[u32]) -> Option<(f32, f32)> {
    let ids: BTreeSet<u32> = unit_ids.iter().copied().collect();
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut count = 0usize;
    for entity in observation
        .owned
        .iter()
        .filter(|entity| ids.contains(&entity.id))
    {
        sum_x += entity.x;
        sum_y += entity.y;
        count += 1;
    }
    (count > 0).then(|| (sum_x / count as f32, sum_y / count as f32))
}
