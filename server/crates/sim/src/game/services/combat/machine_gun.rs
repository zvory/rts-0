//! Experimental first-hit MG bursts. Rays resolve immediately; travel is presentation only.
use std::collections::{BTreeMap, HashMap};

use rand::Rng;

use crate::game::entity::{blocks_line_of_sight, EntityKind, EntityStore};
use crate::game::entrenchment_combat;
use crate::game::firing_reveal::{record_firing_reveals_for_victim_team, FiringRevealSource};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services::geometry::{
    building_rect_for_entity, closest_combat_target_point, segment_intersects_rect,
    segment_intersects_unit_body, unit_body_for_entity, RectBody,
};
use crate::game::services::line_of_sight::LineOfSight;
use crate::game::services::spatial::SpatialIndex;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::protocol::Event;
use crate::rules::terrain::TerrainKind;
use crate::rules::{combat as rules, projection};

use super::events::{attack_reveal_for, push_under_attack_notices_for_visible_attack};

pub(super) fn is_machine_gun(kind: rules::WeaponKind) -> bool {
    matches!(
        kind,
        rules::WeaponKind::MachineGunnerMg
            | rules::WeaponKind::ScoutCarMg
            | rules::WeaponKind::TankCoax
    )
}

/// A bounded ray query shared by the three bullets in a burst. Friendly infantry is transparent;
/// friendly tanks and opaque buildings absorb bullets without taking damage.
#[allow(clippy::too_many_arguments)]
fn first_hit(
    map: &Map,
    entities: &EntityStore,
    teams: &TeamRelations,
    candidates: &[u32],
    attacker: u32,
    owner: u32,
    intended: u32,
    start: (f32, f32),
    end: (f32, f32),
) -> Option<(u32, f32)> {
    candidates
        .iter()
        .filter_map(|id| {
            let entity = entities.get(*id)?;
            if *id == attacker || entity.hp == 0 || entity.is_node() || !entity.is_targetable() {
                return None;
            }
            let hard = entity.kind == EntityKind::Tank
                || (entity.is_building() && blocks_line_of_sight(entity.kind));
            if *id != intended && !teams.is_enemy_owner(owner, entity.owner) && !hard {
                return None;
            }
            let hit = if entity.is_building() {
                segment_intersects_rect(start, end, building_rect_for_entity(map, entity)?)
            } else {
                segment_intersects_unit_body(start, end, unit_body_for_entity(entity)?)
            }?;
            Some((*id, hit))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
}

fn clear_endpoint(los: &LineOfSight<'_>, start: (f32, f32), end: (f32, f32)) -> (f32, f32) {
    if los.clear_between_world_points(start, end) {
        return end;
    }
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..12 {
        let mid = (low + high) * 0.5;
        let point = (
            start.0 + (end.0 - start.0) * mid,
            start.1 + (end.1 - start.1) * mid,
        );
        if los.clear_between_world_points(start, point) {
            low = mid;
        } else {
            high = mid;
        }
    }
    (
        start.0 + (end.0 - start.0) * low,
        start.1 + (end.1 - start.1) * low,
    )
}

// Short weapon rays have a bounded tile rectangle. Test tile intersections instead of sampling
// points: a diagonal can cross an arbitrarily short hidden sliver near a tile corner.
fn ray_visible(
    map: &Map,
    fog: &Fog,
    teams: &TeamRelations,
    viewer: u32,
    start: (f32, f32),
    end: (f32, f32),
) -> bool {
    if !map.contains_world_point(start.0, start.1) || !map.contains_world_point(end.0, end.1) {
        return false;
    }
    let (sx, sy) = map.tile_of(start.0, start.1);
    let (ex, ey) = map.tile_of(end.0, end.1);
    let ts = crate::config::TILE_SIZE as f32;
    for y in sy.min(ey)..=sy.max(ey) {
        for x in sx.min(ex)..=sx.max(ex) {
            let rect = RectBody {
                min_x: x as f32 * ts,
                min_y: y as f32 * ts,
                max_x: (x + 1) as f32 * ts,
                max_y: (y + 1) as f32 * ts,
            };
            let center = map.tile_center(x, y);
            if segment_intersects_rect(start, end, rect).is_some()
                && !projection::team_visible_world(viewer, center.0, center.1, fog, teams)
            {
                return false;
            }
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fire(
    map: &Map,
    entities: &mut EntityStore,
    teams: &TeamRelations,
    spatial: &SpatialIndex,
    los: &LineOfSight<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    rng: &mut impl Rng,
    events: &mut HashMap<u32, Vec<Event>>,
    firing_reveals: &mut Vec<FiringRevealSource>,
    attacker: u32,
    intended: u32,
    profile: &rules::WeaponProfile,
    range: f32,
    tick: u32,
) {
    let Some(source) = entities.get(attacker) else {
        return;
    };
    let (owner, start) = (source.owner, (source.pos_x, source.pos_y));
    let Some(target) = entities.get(intended) else {
        return;
    };
    // Match acquisition distance: a building surface can be in range while its center is not.
    let aim = closest_combat_target_point(map, start, target);
    let angle = (aim.1 - start.1).atan2(aim.0 - start.0);
    if !range.is_finite() || range <= 0.0 {
        return;
    }
    // Buildings are indexed by center; pad enough for their attackable footprint.
    let padding = super::projection::max_building_combat_extent_px().max(64.0);
    let candidates: Vec<_> = spatial
        .ids_in_circle_bbox(start.0, start.1, range + padding)
        .collect();
    let mut hits = BTreeMap::<u32, u32>::new();
    let mut rays = Vec::with_capacity(rules::MG_BURST_BULLETS);
    for ray in 0..rules::MG_BURST_BULLETS {
        // The aimed ray always follows the selected target; the other two scatter.
        let spread = if ray == 0 {
            0.0
        } else {
            rng.gen_range(-rules::MG_HALF_SPREAD_RAD..=rules::MG_HALF_SPREAD_RAD)
        };
        let direction = angle + spread;
        let end = clear_endpoint(
            los,
            start,
            (
                start.0 + direction.cos() * range,
                start.1 + direction.sin() * range,
            ),
        );
        let hit = first_hit(
            map,
            entities,
            teams,
            &candidates,
            attacker,
            owner,
            intended,
            start,
            end,
        );
        let (victim, endpoint) = if let Some((id, t)) = hit {
            let victim = entities.get(id);
            if victim.is_some_and(|v| teams.is_enemy_owner(owner, v.owner) || id == intended) {
                *hits.entry(id).or_default() += profile.dmg / 2;
            }
            (
                id,
                (
                    start.0 + (end.0 - start.0) * t,
                    start.1 + (end.1 - start.1) * t,
                ),
            )
        } else {
            (0, end)
        };
        rays.push((victim, endpoint));
    }
    // Aggregate the burst per victim before integer armor/cover rounding. Bodies absorb every
    // intersecting bullet in this simultaneous burst, including lethal overkill.
    let mut reveals_attacker = false;
    for (id, damage) in hits {
        let Some(victim) = entities.get(id) else {
            continue;
        };
        let (victim_owner, pos, kind) = (victim.owner, (victim.pos_x, victim.pos_y), victim.kind);
        let damage = rules::effective_damage_with_facing_for_weapon(
            profile,
            kind,
            damage,
            Some(TerrainKind::Open),
            Some(victim.facing()),
            pos,
            start,
        );
        let damage = entrenchment_combat::reduce_direct_damage(victim, damage);
        // A real hit must survive integer protection rounding, even for a lone half-damage ray.
        let damage = map.damage_after_reduction_tile(pos.0, pos.1, damage).max(1);
        let damaged = entities.get_mut(id).is_some_and(|v| {
            if teams.is_enemy_owner(owner, victim_owner) {
                v.apply_damage_from_entity(damage, owner, attacker, start, tick)
            } else {
                v.apply_damage(damage, None)
            }
        });
        if projection::shot_reveals_attacker(kind) {
            reveals_attacker = true;
            record_firing_reveals_for_victim_team(
                firing_reveals,
                events.keys().copied().collect::<Vec<_>>(),
                fog,
                map,
                teams,
                victim_owner,
                owner,
                attacker,
                start,
                tick,
            );
        }
        if damaged {
            push_under_attack_notices_for_visible_attack(
                events,
                fog,
                teams,
                victim_owner,
                owner,
                start.0,
                start.1,
                pos.0,
                pos.1,
            );
        }
    }
    let source = entities.get(attacker);
    let reveal = reveals_attacker
        .then(|| attack_reveal_for(source))
        .flatten();
    for (viewer, output) in events.iter_mut() {
        // Ground visibility alone does not expose a concealed shooter. Trap clearing and
        // missed bursts must not create a transient reveal when no hit grants one.
        if !teams.same_team_or_same_owner(*viewer, owner)
            && (smokes.point_inside(start.0, start.1)
                || (!reveals_attacker
                    && source.is_some_and(|source| {
                        projection::entity_hidden_by_concealment_from_team(
                            *viewer, source, map, fog, teams,
                        )
                    })))
        {
            continue;
        }
        for &(victim, end) in &rays {
            // Do not expose an incidental hidden victim or its impact position. Require the
            // entire ray to be currently visible, preventing trajectories across hidden gaps.
            if !ray_visible(map, fog, teams, *viewer, start, end) {
                continue;
            }
            // A visible impact can still belong to a hidden body whose edge crosses into
            // visible space. Withholding only its id would disclose that body's position.
            if let Some(v) = entities.get(victim) {
                let visible = teams.same_team_or_same_owner(*viewer, v.owner)
                    || (projection::team_visible_world(*viewer, v.pos_x, v.pos_y, fog, teams)
                        && !smokes.point_inside(v.pos_x, v.pos_y)
                        && !projection::entity_hidden_by_concealment_from_team(
                            *viewer, v, map, fog, teams,
                        ));
                if !visible {
                    continue;
                }
            }
            output.push(Event::Attack {
                from: attacker,
                to: victim,
                reveal: reveal.clone(),
                to_pos: Some([end.0, end.1]),
                weapon_kind: Some(profile.id.stable_id().to_string()),
                shot_origin: Some([start.0, start.1]),
            });
        }
    }
}

#[cfg(test)]
mod tests;
