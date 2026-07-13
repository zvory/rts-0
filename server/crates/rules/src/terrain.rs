//! Terrain rule seams.
//!
//! Terrain effects stay here instead of being spread through simulation services.

use crate::EntityKind;

pub const MAP_TERRAIN_GRASS: u8 = 0;
pub const MAP_TERRAIN_ROCK: u8 = 1;
pub const MAP_TERRAIN_WATER: u8 = 2;
pub const MAP_TERRAIN_ROAD_BARE: u8 = 3;
pub const MAP_TERRAIN_ROAD_HORIZONTAL: u8 = 4;
pub const MAP_TERRAIN_ROAD_VERTICAL: u8 = 5;
pub const MAP_TERRAIN_ROAD_DIAGONAL_NW_SE: u8 = 6;
pub const MAP_TERRAIN_ROAD_DIAGONAL_NE_SW: u8 = 7;

pub const ROAD_MOVEMENT_SPEED_MULTIPLIER: f32 = 1.4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainKind {
    Open,
    Road,
    // Forest,
    // Hill,
}

impl TerrainKind {
    pub fn from_map_code(code: u8) -> Option<Self> {
        match code {
            MAP_TERRAIN_GRASS => Some(TerrainKind::Open),
            MAP_TERRAIN_ROAD_BARE
            | MAP_TERRAIN_ROAD_HORIZONTAL
            | MAP_TERRAIN_ROAD_VERTICAL
            | MAP_TERRAIN_ROAD_DIAGONAL_NW_SE
            | MAP_TERRAIN_ROAD_DIAGONAL_NE_SW => Some(TerrainKind::Road),
            MAP_TERRAIN_ROCK | MAP_TERRAIN_WATER => None,
            _ => None,
        }
    }
}

pub fn is_passable_map_code(code: u8) -> bool {
    TerrainKind::from_map_code(code).is_some()
}

pub fn movement_allowed(_kind: EntityKind, _terrain: TerrainKind) -> bool {
    true
}

/// Multiplier on the unit's movement budget while its center is on this terrain.
pub fn movement_speed_multiplier(_kind: EntityKind, terrain: TerrainKind) -> f32 {
    match terrain {
        TerrainKind::Open => 1.0,
        TerrainKind::Road => ROAD_MOVEMENT_SPEED_MULTIPLIER,
    }
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
    code == MAP_TERRAIN_ROCK
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn passable_map_codes_project_to_their_terrain_kind() {
        assert_eq!(
            TerrainKind::from_map_code(MAP_TERRAIN_GRASS),
            Some(TerrainKind::Open)
        );
        assert_eq!(
            TerrainKind::from_map_code(MAP_TERRAIN_ROAD_BARE),
            Some(TerrainKind::Road)
        );
        assert_eq!(
            TerrainKind::from_map_code(MAP_TERRAIN_ROAD_HORIZONTAL),
            Some(TerrainKind::Road)
        );
        assert_eq!(
            TerrainKind::from_map_code(MAP_TERRAIN_ROAD_VERTICAL),
            Some(TerrainKind::Road)
        );
        assert_eq!(
            TerrainKind::from_map_code(MAP_TERRAIN_ROAD_DIAGONAL_NW_SE),
            Some(TerrainKind::Road)
        );
        assert_eq!(
            TerrainKind::from_map_code(MAP_TERRAIN_ROAD_DIAGONAL_NE_SW),
            Some(TerrainKind::Road)
        );
        assert_eq!(TerrainKind::from_map_code(MAP_TERRAIN_ROCK), None);
        assert_eq!(TerrainKind::from_map_code(MAP_TERRAIN_WATER), None);
        assert!(is_passable_map_code(MAP_TERRAIN_GRASS));
        assert!(is_passable_map_code(MAP_TERRAIN_ROAD_BARE));
        assert!(is_passable_map_code(MAP_TERRAIN_ROAD_HORIZONTAL));
        assert!(is_passable_map_code(MAP_TERRAIN_ROAD_VERTICAL));
        assert!(is_passable_map_code(MAP_TERRAIN_ROAD_DIAGONAL_NW_SE));
        assert!(is_passable_map_code(MAP_TERRAIN_ROAD_DIAGONAL_NE_SW));
        assert!(!is_passable_map_code(MAP_TERRAIN_ROCK));
        assert!(!is_passable_map_code(MAP_TERRAIN_WATER));
    }

    #[test]
    fn stone_blocks_line_of_sight_but_water_does_not() {
        assert!(!blocks_line_of_sight(MAP_TERRAIN_GRASS));
        assert!(!blocks_line_of_sight(MAP_TERRAIN_ROAD_BARE));
        assert!(!blocks_line_of_sight(MAP_TERRAIN_ROAD_HORIZONTAL));
        assert!(!blocks_line_of_sight(MAP_TERRAIN_ROAD_VERTICAL));
        assert!(!blocks_line_of_sight(MAP_TERRAIN_ROAD_DIAGONAL_NW_SE));
        assert!(!blocks_line_of_sight(MAP_TERRAIN_ROAD_DIAGONAL_NE_SW));
        assert!(blocks_line_of_sight(MAP_TERRAIN_ROCK));
        assert!(!blocks_line_of_sight(MAP_TERRAIN_WATER));
    }

    #[test]
    fn roads_only_change_movement_speed_for_every_kind() {
        for kind in EntityKind::ALL {
            assert!(movement_allowed(kind, TerrainKind::Open));
            assert!(movement_allowed(kind, TerrainKind::Road));
            assert_eq!(movement_speed_multiplier(kind, TerrainKind::Open), 1.0);
            assert_eq!(
                movement_speed_multiplier(kind, TerrainKind::Road),
                ROAD_MOVEMENT_SPEED_MULTIPLIER
            );
            assert_eq!(cover_modifier(kind, TerrainKind::Open), 1.0);
            assert_eq!(cover_modifier(kind, TerrainKind::Road), 1.0);
            assert_eq!(concealment_modifier(kind, TerrainKind::Open), 1.0);
            assert_eq!(concealment_modifier(kind, TerrainKind::Road), 1.0);
        }
    }
}
