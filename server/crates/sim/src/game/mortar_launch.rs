use std::collections::HashMap;

use crate::config;
use crate::game::teams::TeamRelations;
use crate::protocol::Event;

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_launch(
    events: &mut HashMap<u32, Vec<Event>>,
    teams: &TeamRelations,
    owner: u32,
    attacker: u32,
    from_x: f32,
    from_y: f32,
    to_x: f32,
    to_y: f32,
    delay_ticks: u32,
    rocket: bool,
) {
    let player_ids: Vec<u32> = events.keys().copied().collect();
    for pid in player_ids {
        if !teams.same_team_or_same_owner(pid, owner) {
            continue;
        }
        events.entry(pid).or_default().push(Event::MortarLaunch {
            from: attacker,
            from_x,
            from_y,
            to_x,
            to_y,
            radius_tiles: config::MORTAR_OUTER_RADIUS_TILES,
            delay_ticks,
            rocket,
        });
    }
}
