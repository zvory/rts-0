//! Terrain rule seams.
//!
//! Today all passable terrain is open ground. Forests, roads, hills, and their combat/movement
//! effects intentionally land here later instead of being spread through services.

use crate::game::entity::EntityKind;
use crate::protocol::terrain as wire_terrain;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainKind {
    Open,
    // Forest,
    // Road,
    // Hill,
}

impl TerrainKind {
    pub fn from_map_code(code: u8) -> Option<Self> {
        match code {
            wire_terrain::GRASS => Some(TerrainKind::Open),
            wire_terrain::ROCK | wire_terrain::WATER => None,
            _ => None,
        }
    }
}

pub fn movement_allowed(_kind: EntityKind, _terrain: TerrainKind) -> bool {
    true
}

/// Multiplier on incoming damage.
pub fn cover_modifier(_kind: EntityKind, _terrain: TerrainKind) -> f32 {
    1.0
}

/// Multiplier on enemy detection range against this entity.
pub fn concealment_modifier(_kind: EntityKind, _terrain: TerrainKind) -> f32 {
    1.0
}

/// Whether this raw map terrain code blocks line-of-sight for fog and ranged attacks.
/// Stone blocks today; forests can grow into this seam later with partial visibility rules.
pub fn blocks_line_of_sight(code: u8) -> bool {
    code == wire_terrain::ROCK
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::EntityKind;
    use crate::protocol::terrain as wire_terrain;

    #[test]
    fn current_map_codes_only_project_grass_to_open_terrain() {
        assert_eq!(
            TerrainKind::from_map_code(wire_terrain::GRASS),
            Some(TerrainKind::Open)
        );
        assert_eq!(TerrainKind::from_map_code(wire_terrain::ROCK), None);
        assert_eq!(TerrainKind::from_map_code(wire_terrain::WATER), None);
    }

    #[test]
    fn stone_blocks_line_of_sight_but_water_does_not() {
        assert!(!blocks_line_of_sight(wire_terrain::GRASS));
        assert!(blocks_line_of_sight(wire_terrain::ROCK));
        assert!(!blocks_line_of_sight(wire_terrain::WATER));
    }

    #[test]
    fn terrain_stubs_preserve_current_defaults_for_every_kind() {
        for kind in EntityKind::ALL {
            assert!(movement_allowed(kind, TerrainKind::Open));
            assert_eq!(cover_modifier(kind, TerrainKind::Open), 1.0);
            assert_eq!(concealment_modifier(kind, TerrainKind::Open), 1.0);
        }
    }
}
