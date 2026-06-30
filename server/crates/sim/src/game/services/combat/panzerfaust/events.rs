use std::collections::HashMap;

use crate::config;
use crate::game::entity::EntityKind;
use crate::protocol::{self, Event};
use crate::rules::projection as projection_rules;

use super::{Fog, SmokeCloudStore, TeamRelations};

pub(super) fn emit_launch(
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    teams: &TeamRelations,
    owner: u32,
    from: u32,
    from_pos: (f32, f32),
    to_pos: (f32, f32),
) {
    for player_id in events.keys().copied().collect::<Vec<_>>() {
        if !projection_rules::event_visible_to_team_with_smoke(
            player_id, from_pos.0, from_pos.1, owner, fog, teams, smokes,
        ) {
            continue;
        }
        let endpoint_visible = teams.same_team_or_same_owner(player_id, owner)
            || (projection_rules::team_visible_world(player_id, to_pos.0, to_pos.1, fog, teams)
                && !smokes.point_inside(to_pos.0, to_pos.1));
        let endpoint = if endpoint_visible { to_pos } else { from_pos };
        events
            .entry(player_id)
            .or_default()
            .push(Event::PanzerfaustLaunch {
                from,
                from_x: from_pos.0,
                from_y: from_pos.1,
                to_x: endpoint.0,
                to_y: endpoint.1,
                delay_ticks: config::PANZERFAUST_TRAVEL_TICKS,
            });
    }
}

pub(super) fn emit_impact(
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    teams: &TeamRelations,
    owner: u32,
    impact: (f32, f32),
) {
    for player_id in events.keys().copied().collect::<Vec<_>>() {
        if projection_rules::event_visible_to_team_with_smoke(
            player_id, impact.0, impact.1, owner, fog, teams, smokes,
        ) {
            events
                .entry(player_id)
                .or_default()
                .push(Event::PanzerfaustImpact {
                    x: impact.0,
                    y: impact.1,
                });
        }
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
