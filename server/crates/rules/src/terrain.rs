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
pub const MAP_TERRAIN_GRAVEL_A: u8 = 8;
pub const MAP_TERRAIN_GRAVEL_B: u8 = 9;
pub const MAP_TERRAIN_GRAVEL_C: u8 = 10;
pub const MAP_TERRAIN_DIRT_A: u8 = 11;
pub const MAP_TERRAIN_DIRT_B: u8 = 12;
pub const MAP_TERRAIN_DIRT_C: u8 = 13;
pub const MAP_TERRAIN_MUD_A: u8 = 14;
pub const MAP_TERRAIN_MUD_B: u8 = 15;
pub const MAP_TERRAIN_MUD_C: u8 = 16;
pub const MAP_TERRAIN_FROSTED_GROUND: u8 = 17;

pub const ROAD_MOVEMENT_SPEED_MULTIPLIER: f32 = 1.5;
pub const SLOW_MOVEMENT_TILE_SPEED_MULTIPLIER: f32 = 0.75;
pub const UPHILL_MOVEMENT_SPEED_MULTIPLIER: f32 = 0.70;
pub const DOWNHILL_MOVEMENT_SPEED_MULTIPLIER: f32 = 1.30;
/// Maximum ordinary fog-of-war sight granted by authored elevation.
pub const MAX_ELEVATION_SIGHT_BONUS_TILES: u32 = 4;
/// Body-edge distance at which an ordinary unit detects a concealed hostile unit.
pub const CONCEALMENT_CLOSE_DETECTION_RANGE_TILES: f32 = 2.0;
/// Maximum number of concealment tiles an ordinary fog-of-war sight ray may enter.
pub const CONCEALMENT_SIGHT_DEPTH_TILES: u32 = 3;
/// Duration that an entity remains detected after close contact ends.
pub const CONCEALMENT_DETECTION_PERSIST_TICKS: u32 = crate::balance::TICK_HZ;
const DAMAGE_REDUCTION_TILE_DAMAGE_NUMERATOR: u32 = 3;
const DAMAGE_REDUCTION_TILE_DAMAGE_DENOMINATOR: u32 = 4;
pub const DAMAGE_REDUCTION_TILE_DAMAGE_MULTIPLIER: f32 =
    DAMAGE_REDUCTION_TILE_DAMAGE_NUMERATOR as f32 / DAMAGE_REDUCTION_TILE_DAMAGE_DENOMINATOR as f32;

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
            MAP_TERRAIN_GRASS
            | MAP_TERRAIN_GRAVEL_A
            | MAP_TERRAIN_GRAVEL_B
            | MAP_TERRAIN_GRAVEL_C
            | MAP_TERRAIN_DIRT_A
            | MAP_TERRAIN_DIRT_B
            | MAP_TERRAIN_DIRT_C
            | MAP_TERRAIN_MUD_A
            | MAP_TERRAIN_MUD_B
            | MAP_TERRAIN_MUD_C
            | MAP_TERRAIN_FROSTED_GROUND => Some(TerrainKind::Open),
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

/// Multiplier on movement while the unit center occupies an authored slow-movement tile.
pub fn slow_movement_tile_multiplier(active: bool) -> f32 {
    if active {
        SLOW_MOVEMENT_TILE_SPEED_MULTIPLIER
    } else {
        1.0
    }
}

/// Binary multiplier on movement toward a locally sampled elevation.
/// The size of the elevation difference is intentionally irrelevant.
pub fn elevation_movement_speed_multiplier(current: u8, ahead: u8) -> f32 {
    if ahead > current {
        UPHILL_MOVEMENT_SPEED_MULTIPLIER
    } else if ahead < current {
        DOWNHILL_MOVEMENT_SPEED_MULTIPLIER
    } else {
        1.0
    }
}

/// Extra ordinary sight granted by the observer's absolute authored elevation.
///
/// Two elevation levels grant one tile, capped so high plateaus remain a modest positional
/// advantage. Low ground never reduces the entity's base sight.
pub fn elevation_sight_bonus_tiles(elevation: u8) -> u32 {
    (u32::from(elevation) / 2).min(MAX_ELEVATION_SIGHT_BONUS_TILES)
}

/// Reduce incoming damage by 25% while the target center occupies an authored reduction tile.
/// Integer damage rounds up so a non-zero hit always remains meaningful.
pub fn damage_after_reduction_tile(damage: u32, active: bool) -> u32 {
    if active {
        let scaled = u64::from(damage) * u64::from(DAMAGE_REDUCTION_TILE_DAMAGE_NUMERATOR);
        ((scaled + u64::from(DAMAGE_REDUCTION_TILE_DAMAGE_DENOMINATOR - 1))
            / u64::from(DAMAGE_REDUCTION_TILE_DAMAGE_DENOMINATOR)) as u32
    } else {
        damage
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
        let open_codes = [
            MAP_TERRAIN_GRASS,
            MAP_TERRAIN_GRAVEL_A,
            MAP_TERRAIN_GRAVEL_B,
            MAP_TERRAIN_GRAVEL_C,
            MAP_TERRAIN_DIRT_A,
            MAP_TERRAIN_DIRT_B,
            MAP_TERRAIN_DIRT_C,
            MAP_TERRAIN_MUD_A,
            MAP_TERRAIN_MUD_B,
            MAP_TERRAIN_MUD_C,
            MAP_TERRAIN_FROSTED_GROUND,
        ];
        for code in open_codes {
            assert_eq!(TerrainKind::from_map_code(code), Some(TerrainKind::Open));
            assert!(is_passable_map_code(code));
            assert!(!blocks_line_of_sight(code));
        }
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

    #[test]
    fn authored_overlay_multipliers_are_exact_and_nonzero_damage_stays_nonzero() {
        assert_eq!(slow_movement_tile_multiplier(false), 1.0);
        assert_eq!(
            slow_movement_tile_multiplier(true),
            SLOW_MOVEMENT_TILE_SPEED_MULTIPLIER
        );
        assert_eq!(damage_after_reduction_tile(100, true), 75);
        assert_eq!(damage_after_reduction_tile(99, true), 75);
        assert_eq!(damage_after_reduction_tile(1, true), 1);
        assert_eq!(damage_after_reduction_tile(0, true), 0);
        assert_eq!(damage_after_reduction_tile(99, false), 99);
    }

    #[test]
    fn elevation_movement_is_directional_and_independent_of_grade_size() {
        assert_eq!(elevation_movement_speed_multiplier(4, 4), 1.0);
        assert_eq!(elevation_movement_speed_multiplier(4, 5), 0.70);
        assert_eq!(elevation_movement_speed_multiplier(0, 9), 0.70);
        assert_eq!(elevation_movement_speed_multiplier(5, 4), 1.30);
        assert_eq!(elevation_movement_speed_multiplier(9, 0), 1.30);
    }

    #[test]
    fn elevation_sight_bonus_steps_every_two_levels_and_caps_at_four_tiles() {
        let expected = [0, 0, 1, 1, 2, 2, 3, 3, 4, 4];
        for (elevation, expected_bonus) in expected.into_iter().enumerate() {
            assert_eq!(elevation_sight_bonus_tiles(elevation as u8), expected_bonus);
        }
        assert_eq!(elevation_sight_bonus_tiles(u8::MAX), 4);
    }
}
