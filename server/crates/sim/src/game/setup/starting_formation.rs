use super::*;

/// Aim the opening defense at the nearest opposing main base. Stable player order breaks equal
/// distance ties, so free-for-all starts remain deterministic.
pub(super) fn defensive_target_for_player(
    map: &Map,
    players: &[PlayerInit],
    player_index: usize,
    start: (u32, u32),
) -> (f32, f32) {
    let player = players.get(player_index);
    let target_start = player.and_then(|player| {
        let team_id = super::super::teams::normalize_team_id(player.id, player.team_id);
        players
            .iter()
            .enumerate()
            .filter(|(index, candidate)| {
                *index != player_index
                    && super::super::teams::normalize_team_id(candidate.id, candidate.team_id)
                        != team_id
            })
            .filter_map(|(index, _)| map.starts.get(index).copied().map(|tile| (index, tile)))
            .min_by_key(|(index, (x, y))| {
                let dx = i64::from(*x) - i64::from(start.0);
                let dy = i64::from(*y) - i64::from(start.1);
                (dx * dx + dy * dy, *index)
            })
            .map(|(_, tile)| tile)
    });

    target_start
        .map(|tile| map.tile_center(tile.0, tile.1))
        .unwrap_or((map.world_width_px() * 0.5, map.world_height_px() * 0.5))
}
