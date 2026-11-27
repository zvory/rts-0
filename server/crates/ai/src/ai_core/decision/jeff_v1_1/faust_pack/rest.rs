//! Where the Panzerfaust pack rests: behind Jeff's biggest Tank group, or behind the push's
//! Riflemen under `pfpush`, and each Panzerfaust's slot there (see the parent module docs).

use super::*;

/// Where the pack rests and which way it faces: behind Jeff's biggest group of Tanks (Tanks within
/// `GROUP_TILES` of one another), facing the nearest enemy Tank in sight or away from home; else
/// ahead of the Depot nearest the enemy. `last`: the group's centre at the last decision, which
/// keeps the pack with its group until another holds `SWITCH_MARGIN` more Tanks.
pub(super) fn rest_place(
    observation: &AiObservation,
    last: &mut Option<(i32, i32)>,
) -> Option<((f32, f32), (f32, f32))> {
    let ts = observation.map.tile_size as f32;
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let enemy = enemy_start(observation);
    let tanks: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
        .map(|unit| (unit.x, unit.y))
        .collect();
    // Groups: Tanks linked by chains of Tanks within `GROUP_TILES`.
    let mut group: Vec<usize> = (0..tanks.len()).collect();
    let reach = squared(GROUP_TILES * ts);
    for a in 0..tanks.len() {
        for b in (a + 1)..tanks.len() {
            if dist2(tanks[a].0, tanks[a].1, tanks[b].0, tanks[b].1) <= reach {
                let (from, to) = (group[b], group[a]);
                if from != to {
                    for g in group.iter_mut() {
                        if *g == from {
                            *g = to;
                        }
                    }
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, (usize, f32, f32)> = BTreeMap::new();
    for (index, at) in tanks.iter().enumerate() {
        let entry = groups.entry(group[index]).or_insert((0, 0.0, 0.0));
        entry.0 += 1;
        entry.1 += at.0;
        entry.2 += at.1;
    }
    let centres: Vec<(usize, (f32, f32))> = groups
        .values()
        .map(|(count, x, y)| (*count, (x / *count as f32, y / *count as f32)))
        .collect();
    let best = centres
        .iter()
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1 .0.total_cmp(&a.1 .0)))
        .copied();
    let previous = last.and_then(|at| {
        centres
            .iter()
            .filter(|(_, centre)| {
                dist2(centre.0, centre.1, at.0 as f32, at.1 as f32) <= squared(FOLLOW_TILES * ts)
            })
            .min_by(|a, b| {
                dist2(a.1 .0, a.1 .1, at.0 as f32, at.1 as f32).total_cmp(&dist2(
                    b.1 .0,
                    b.1 .1,
                    at.0 as f32,
                    at.1 as f32,
                ))
            })
            .copied()
    });
    let chosen = match (previous, best) {
        (Some(previous), Some(best)) if best.0 < previous.0 + SWITCH_MARGIN => Some(previous),
        (_, best) => best,
    };
    *last = chosen.map(|(_, centre)| (centre.0 as i32, centre.1 as i32));
    if let Some((_, center)) = chosen {
        let facing = observation
            .visible_enemies
            .iter()
            .filter(|enemy| enemy.kind == EntityKind::Tank && enemy.hp > 0)
            .map(|enemy| (enemy.x, enemy.y))
            .filter(|at| dist2(at.0, at.1, center.0, center.1) <= squared(FACE_TILES * ts))
            .min_by(|a, b| {
                dist2(a.0, a.1, center.0, center.1).total_cmp(&dist2(b.0, b.1, center.0, center.1))
            });
        let forward = facing
            .and_then(|at| unit_vector(center, at))
            .or_else(|| unit_vector(home, center))
            .or_else(|| enemy.and_then(|enemy| unit_vector(center, enemy)))
            .unwrap_or((1.0, 0.0));
        return Some((center, forward));
    }
    let enemy = enemy?;
    let depot = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.is_complete && unit.hp > 0)
        .min_by(|a, b| {
            dist2(a.x, a.y, enemy.0, enemy.1).total_cmp(&dist2(b.x, b.y, enemy.0, enemy.1))
        })?;
    let forward = unit_vector((depot.x, depot.y), enemy)?;
    // The slots start `BACK_TILES` behind the centre.
    let ahead = (DEPOT_AHEAD_TILES + BACK_TILES) * ts;
    Some((
        (depot.x + forward.0 * ahead, depot.y + forward.1 * ahead),
        forward,
    ))
}

/// `pfpush`: where the pack rests while Jeff's push is out: behind the centre of the push's
/// Riflemen (its Tanks with fewer than two Riflemen left), facing the nearest enemy Tank in sight,
/// else the push's next waypoint, else away from home. `None` with no push out.
pub(super) fn push_rest(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
) -> Option<((f32, f32), (f32, f32))> {
    let push = &memory.containment;
    if !push.wave_launched || push.active_tanks.is_empty() {
        return None;
    }
    let ts = observation.map.tile_size as f32;
    let centre_of = |ids: &BTreeSet<u32>| {
        let at: Vec<(f32, f32)> = observation
            .owned
            .iter()
            .filter(|unit| ids.contains(&unit.id) && unit.is_complete && unit.hp > 0)
            .map(|unit| (unit.x, unit.y))
            .collect();
        let n = at.len();
        (n > 0).then(|| {
            let (x, y) = at
                .iter()
                .fold((0.0_f32, 0.0_f32), |sum, at| (sum.0 + at.0, sum.1 + at.1));
            (n, (x / n as f32, y / n as f32))
        })
    };
    let (_, tanks) = centre_of(&push.active_tanks)?;
    let center = match centre_of(&push.active_riflemen) {
        Some((n, at)) if n >= 2 => at,
        _ => tanks,
    };
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let facing = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::Tank && enemy.hp > 0)
        .map(|enemy| (enemy.x, enemy.y))
        .filter(|at| dist2(at.0, at.1, center.0, center.1) <= squared(FACE_TILES * ts))
        .min_by(|a, b| {
            dist2(a.0, a.1, center.0, center.1).total_cmp(&dist2(b.0, b.1, center.0, center.1))
        });
    let forward = facing
        .and_then(|at| unit_vector(center, at))
        .or_else(|| {
            push.march_waypoint
                .and_then(|at| unit_vector(center, (at.0 as f32, at.1 as f32)))
        })
        .or_else(|| unit_vector(home, center))
        .unwrap_or((1.0, 0.0));
    Some((center, forward))
}

/// Slot `index` of a pack resting behind `center` facing `forward`.
pub(super) fn slot_point(
    observation: &AiObservation,
    (center, forward): ((f32, f32), (f32, f32)),
    index: usize,
) -> (f32, f32) {
    let ts = observation.map.tile_size as f32;
    let side = (-forward.1, forward.0);
    let row = (index / SLOT_ROW) as f32;
    let column = (index % SLOT_ROW) as f32 - (SLOT_ROW as f32 - 1.0) / 2.0;
    let back = (BACK_TILES + row * SLOT_SPACING_TILES) * ts;
    let across = column * SLOT_SPACING_TILES * ts;
    clamp_to_map(
        (
            center.0 - forward.0 * back + side.0 * across,
            center.1 - forward.1 * back + side.1 * across,
        ),
        observation.map,
    )
}
