use std::collections::HashMap;

use crate::config;
use crate::game::entity::EntityKind;
use crate::protocol::{self, Event};
use crate::rules::projection as projection_rules;

use super::{Fog, SmokeCloudStore, TeamRelations};

pub(super) struct LaunchEvent {
    pub owner: u32,
    pub from: u32,
    pub from_pos: (f32, f32),
    pub to_pos: (f32, f32),
}

pub(super) fn emit_launch(
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    teams: &TeamRelations,
    launch: LaunchEvent,
) {
    for player_id in events.keys().copied().collect::<Vec<_>>() {
        if !projection_rules::event_visible_to_team_with_smoke(
            player_id,
            launch.from_pos.0,
            launch.from_pos.1,
            launch.owner,
            fog,
            teams,
            smokes,
        ) {
            continue;
        }
        let endpoint_visible = teams.same_team_or_same_owner(player_id, launch.owner)
            || (projection_rules::team_visible_world(
                player_id,
                launch.to_pos.0,
                launch.to_pos.1,
                fog,
                teams,
            ) && !smokes.point_inside(launch.to_pos.0, launch.to_pos.1));
        let endpoint = if endpoint_visible {
            launch.to_pos
        } else {
            launch.from_pos
        };
        events
            .entry(player_id)
            .or_default()
            .push(Event::PanzerfaustLaunch {
                from: launch.from,
                from_x: launch.from_pos.0,
                from_y: launch.from_pos.1,
                to_x: endpoint.0,
                to_y: endpoint.1,
                delay_ticks: config::PANZERFAUST_TRAVEL_TICKS,
            });
    }
}

pub(super) fn emit_conversion(
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    teams: &TeamRelations,
    owner: u32,
    id: u32,
    pos: (f32, f32),
) {
    for player_id in events.keys().copied().collect::<Vec<_>>() {
        if projection_rules::event_visible_to_team_with_smoke(
            player_id, pos.0, pos.1, owner, fog, teams, smokes,
        ) {
            events
                .entry(player_id)
                .or_default()
                .push(Event::PanzerfaustConversion {
                    id,
                    to_kind: protocol::kind_to_wire(EntityKind::Rifleman).to_string(),
                });
        }
    }
}
