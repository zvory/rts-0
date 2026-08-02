use super::{fnv_bytes, fnv_usize};
use crate::config;
use crate::protocol::{validate_map_doodads, MapDoodad};

pub(crate) fn canonicalize(
    size: u32,
    mut doodads: Vec<MapDoodad>,
) -> Result<Vec<MapDoodad>, String> {
    doodads.sort_unstable_by_key(|doodad| doodad.id);
    let world_size_px = size
        .checked_mul(config::TILE_SIZE)
        .ok_or_else(|| "map world-pixel size overflows u32".to_string())?;
    validate_map_doodads(&doodads, world_size_px)?;
    Ok(doodads)
}

pub(super) fn hash_materialized(mut hash: u64, doodads: &[MapDoodad]) -> u64 {
    // Preserve the materialized hash of legacy maps with no static decorations. Absence and an
    // explicit empty schema-v5 doodad list are the same materialized map.
    if doodads.is_empty() {
        return hash;
    }
    hash = fnv_usize(hash, doodads.len());
    for doodad in doodads {
        hash = fnv_bytes(hash, &doodad.id.to_le_bytes());
        hash = fnv_usize(hash, doodad.type_id.len());
        hash = fnv_bytes(hash, doodad.type_id.as_bytes());
        hash = fnv_bytes(hash, &doodad.x.to_le_bytes());
        hash = fnv_bytes(hash, &doodad.y.to_le_bytes());
        match &doodad.color {
            Some(color) => {
                hash = fnv_bytes(hash, &[1]);
                hash = fnv_usize(hash, color.len());
                hash = fnv_bytes(hash, color.as_bytes());
            }
            None => hash = fnv_bytes(hash, &[0]),
        }
    }
    hash
}
