use super::super::{BASE_PROTECTION_RADIUS_TILES, BASE_SITE_PROTECTION_RADIUS_TILES};
use std::collections::HashSet;

pub(super) fn validate_base_clearance(
    width: u32,
    height: u32,
    terrain_grid: &[u8],
    unwalkable_tiles: &[(u32, u32)],
    start_locations: &[(u32, u32)],
    base_sites: &[(u32, u32)],
) -> Result<(), String> {
    let starts: HashSet<_> = start_locations.iter().copied().collect();
    for &(sx, sy) in base_sites {
        let radius = if starts.contains(&(sx, sy)) {
            BASE_PROTECTION_RADIUS_TILES
        } else {
            BASE_SITE_PROTECTION_RADIUS_TILES
        };
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let tx = sx as i32 + dx;
                let ty = sy as i32 + dy;
                if tx < 0 || ty < 0 || tx >= width as i32 || ty >= height as i32 {
                    return Err(format!(
                        "base site ({sx},{sy}) is too close to the map edge"
                    ));
                }
                let idx = (ty as u32 * width + tx as u32) as usize;
                if unwalkable_tiles
                    .binary_search(&(tx as u32, ty as u32))
                    .is_ok()
                    || !crate::rules::terrain::is_passable_map_code(terrain_grid[idx])
                {
                    return Err(format!(
                        "base site ({sx},{sy}) has impassable terrain in its protected area at ({tx},{ty})"
                    ));
                }
            }
        }
    }
    Ok(())
}
