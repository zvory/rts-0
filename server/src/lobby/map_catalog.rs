use rts_sim::game::map::Map;

use super::MAX_PLAYERS;

pub(super) fn selectable_map(name: &str) -> Option<(String, usize)> {
    Map::list_available()
        .into_iter()
        .find(|entry| entry.name == name)
        .map(|entry| {
            let cap = (entry.max_players as usize).clamp(1, MAX_PLAYERS);
            (entry.name, cap)
        })
}

pub(super) fn active_slot_cap(map_name: &str) -> usize {
    selectable_map(map_name).map_or(MAX_PLAYERS, |(_, cap)| cap)
}
