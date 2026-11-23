use super::standability::unit_static_standable_with_radius;
use super::Occupancy;
use crate::config;
use crate::game::entity::EntityStore;
use crate::game::map::Map;

/// Commit growth only at a legal body position, using the current post-production topology.
/// A bounded local adjustment clears adjacent walls without teleporting through tile blockers.
/// If the body cannot fit nearby, leave insight and HP unchanged and retry next tick.
pub(crate) fn advance_dao_masters(
    map: &Map,
    entities: &mut EntityStore,
    eligible_owner: impl Fn(u32) -> bool,
    occ: &Occupancy,
) {
    for id in entities.ids() {
        let Some(unit) = entities.get(id) else {
            continue;
        };
        if unit.hp == 0
            || unit.dao_master
            || unit.dao().is_none()
            || unit.insight < config::DISCIPLE_EVOLVED_INSIGHT_CAP
            || !eligible_owner(unit.owner)
        {
            continue;
        }
        let radius = unit.radius() * config::DAO_MASTER_BODY_SCALE;
        let legal = |x, y| {
            unit_static_standable_with_radius(
                map,
                occ,
                unit.kind,
                x,
                y,
                unit.facing(),
                Some(radius),
            )
        };
        let origin = (unit.pos_x(), unit.pos_y());
        let mut destination = legal(origin.0, origin.1).then_some(origin);
        let max_shift = (radius - unit.radius()) * 2.0 + 1.0;
        // Fixed directions and half-pixel steps make the nearest bounded search deterministic.
        let diagonal = std::f32::consts::FRAC_1_SQRT_2;
        let directions = [
            (1.0, 0.0),
            (-1.0, 0.0),
            (0.0, 1.0),
            (0.0, -1.0),
            (diagonal, diagonal),
            (-diagonal, diagonal),
            (diagonal, -diagonal),
            (-diagonal, -diagonal),
        ];
        for step in 1..=32 {
            if destination.is_some() {
                break;
            }
            let distance = step as f32 * 0.5;
            if distance > max_shift {
                break;
            }
            destination = directions
                .iter()
                .map(|(dx, dy)| (origin.0 + dx * distance, origin.1 + dy * distance))
                .find(|(x, y)| legal(*x, *y));
        }
        if let (Some((x, y)), Some(unit)) = (destination, entities.get_mut(id)) {
            unit.advance_dao_master();
            unit.set_position(x, y);
        }
    }
}
