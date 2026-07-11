use crate::config;
use crate::game::fog::Fog;
use crate::game::teams::TeamRelations;
use crate::rules::projection;

const MEDIAN_TO_MAX_SCATTER_RADIUS: f32 = 2.0;

pub(crate) fn predicted_mortar_impact(
    fog: &Fog,
    teams: &TeamRelations,
    owner: u32,
    attacker: u32,
    x: f32,
    y: f32,
    tick: u32,
) -> (f32, f32) {
    if !x.is_finite() || !y.is_finite() {
        return (x, y);
    }
    let median_tiles = if projection::team_visible_world(owner, x, y, fog, teams) {
        config::MORTAR_VISIBLE_MEDIAN_SCATTER_TILES
    } else {
        config::MORTAR_BLIND_MEDIAN_SCATTER_TILES
    };
    let max_radius = median_tiles.max(0.0)
        * MEDIAN_TO_MAX_SCATTER_RADIUS
        * config::TILE_SIZE as f32;
    if max_radius <= f32::EPSILON || !max_radius.is_finite() {
        return (x, y);
    }
    let seed = mortar_scatter_seed(owner, attacker, tick, x, y);
    let angle = unit_float_from_seed(seed) * std::f32::consts::TAU;
    let radius = unit_float_from_seed(seed.rotate_left(29)) * max_radius;
    (x + angle.cos() * radius, y + angle.sin() * radius)
}

fn mortar_scatter_seed(owner: u32, attacker: u32, tick: u32, x: f32, y: f32) -> u64 {
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    seed = mix_u64(seed ^ owner as u64);
    seed = mix_u64(seed ^ ((attacker as u64) << 17));
    seed = mix_u64(seed ^ ((tick as u64) << 31));
    seed = mix_u64(seed ^ x.to_bits() as u64);
    mix_u64(seed ^ ((y.to_bits() as u64) << 1))
}

fn mix_u64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn unit_float_from_seed(seed: u64) -> f32 {
    const MASK_24: u64 = (1 << 24) - 1;
    (seed & MASK_24) as f32 / MASK_24 as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::EntityStore;
    use crate::game::fog::LingeringSightSource;
    use crate::game::map::Map;
    use crate::game::services::dist2;
    use crate::protocol::terrain;

    fn open_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![(4, 4), (size - 5, size - 5)],
            base_sites: Vec::new(),
        }
    }

    fn visible_team_fog(map: &Map, entities: &EntityStore) -> Fog {
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2, 3], entities, map);
        fog
    }

    #[test]
    fn mortar_scatter_uses_visibility_tier_for_impact_point() {
        let map = open_map(30);
        let entities = EntityStore::new();
        let mut fog = visible_team_fog(&map, &entities);
        let source =
            LingeringSightSource::new(1, 160.0, 160.0, 1, 99).expect("source should be valid");
        fog.stamp_lingering_sources(&[source], &map, &entities);
        let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
        let target = (160.0, 160.0);

        let visible_impact =
            predicted_mortar_impact(&fog, &teams, 1, 99, target.0, target.1, 10);
        let visible_offset =
            dist2(target.0, target.1, visible_impact.0, visible_impact.1).sqrt();
        assert!(
            visible_offset
                <= config::MORTAR_VISIBLE_MEDIAN_SCATTER_TILES
                    * MEDIAN_TO_MAX_SCATTER_RADIUS
                    * config::TILE_SIZE as f32
                    + 0.001,
            "visible mortar scatter should use the one-tile median tier, got {visible_offset:.2}px"
        );

        let blind_target = (640.0, 640.0);
        let blind_impact =
            predicted_mortar_impact(&fog, &teams, 1, 99, blind_target.0, blind_target.1, 10);
        let blind_offset =
            dist2(blind_target.0, blind_target.1, blind_impact.0, blind_impact.1).sqrt();
        assert!(
            blind_offset
                <= config::MORTAR_BLIND_MEDIAN_SCATTER_TILES
                    * MEDIAN_TO_MAX_SCATTER_RADIUS
                    * config::TILE_SIZE as f32
                    + 0.001,
            "blind mortar scatter should use the four-tile median tier, got {blind_offset:.2}px"
        );
    }
}
