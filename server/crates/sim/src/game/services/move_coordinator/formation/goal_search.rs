use super::{is_free_goal, preferred_gap_tiles, FormationAssignment, FormationUnit, Occupancy};
use crate::game::map::Map;

/// Keep compact vehicle spacing strict while looking for the nearest local fallback tile. Reachable
/// candidates are preferred; a free fallback can still let normal path processing report failure.
pub(super) fn find_unique_tile_near<F>(
    map: &Map,
    occ: &Occupancy,
    unit: &FormationUnit,
    anchor: (u32, u32),
    assigned: &[FormationAssignment],
    is_goal_reachable: &mut F,
) -> Option<(u32, u32)>
where
    F: FnMut(&FormationUnit, (u32, u32)) -> bool,
{
    let require_spacing = preferred_gap_tiles(unit.kind) > 0
        || assigned
            .iter()
            .any(|assignment| preferred_gap_tiles(assignment.kind) > 0);
    find_tile_near(anchor, |tile| {
        is_free_goal(map, occ, unit, tile, assigned, require_spacing)
            && is_goal_reachable(unit, tile)
    })
    .or_else(|| {
        find_tile_near(anchor, |tile| {
            is_free_goal(map, occ, unit, tile, assigned, require_spacing)
        })
    })
}

fn find_tile_near<V>(anchor: (u32, u32), mut is_candidate_valid: V) -> Option<(u32, u32)>
where
    V: FnMut((u32, u32)) -> bool,
{
    if is_candidate_valid(anchor) {
        return Some(anchor);
    }
    for radius in 1i32..=6 {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dy.abs()) != radius {
                    continue;
                }
                let tx = anchor.0 as i32 + dx;
                let ty = anchor.1 as i32 + dy;
                if tx < 0 || ty < 0 {
                    continue;
                }
                let tile = (tx as u32, ty as u32);
                if is_candidate_valid(tile) {
                    return Some(tile);
                }
            }
        }
    }
    None
}
