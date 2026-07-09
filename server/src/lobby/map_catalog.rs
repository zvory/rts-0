use rts_sim::game::map::Map;

use super::MAX_PLAYERS;

#[derive(Debug, Clone, Copy)]
pub(super) struct ActiveSlotBounds {
    pub(super) min: usize,
    pub(super) max: usize,
}

pub(super) fn selectable_map(name: &str) -> Option<(String, ActiveSlotBounds)> {
    Map::list_available()
        .into_iter()
        .find(|entry| entry.name == name)
        .map(|entry| {
            let max = (entry.max_players as usize).clamp(1, MAX_PLAYERS);
            let min = (entry.min_players as usize).clamp(1, max);
            (entry.name, ActiveSlotBounds { min, max })
        })
}

pub(super) fn active_slot_bounds(map_name: &str) -> ActiveSlotBounds {
    selectable_map(map_name)
        .map(|(_, bounds)| bounds)
        .unwrap_or(ActiveSlotBounds {
            min: 1,
            max: MAX_PLAYERS,
        })
}

pub(super) fn active_slot_cap(map_name: &str) -> usize {
    active_slot_bounds(map_name).max
}
