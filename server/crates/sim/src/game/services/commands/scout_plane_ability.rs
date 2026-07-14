use std::collections::HashMap;

use crate::game::ability::{self, AbilityKind, AbilityTargetMode};
use crate::game::entity::EntityStore;
use crate::game::map::Map;
use crate::game::services::ability_orders::{self, caster_can_accept_order, tech_requirement_met};
use crate::game::services::scout_plane::{self, ScoutPlaneLaunchError};
use crate::game::PlayerState;
use crate::protocol::{Event, NoticeSeverity};
use crate::rules;

use super::guards::dedupe_cap_units;
use super::{notice, notice_positioned, AbilityUse};

pub(super) fn use_ability(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    faction_id: &str,
    request: AbilityUse,
) {
    let ability = AbilityKind::ScoutPlane;
    let definition = ability::definition(ability);
    let Some(x) = request.x else {
        return;
    };
    let Some(y) = request.y else {
        return;
    };
    if definition.target_mode != AbilityTargetMode::WorldPoint
        || !tech_requirement_met(entities, player, ability)
    {
        return;
    }
    let mut active_blocked = false;
    let caster = dedupe_cap_units(request.units, request.max_units_per_command)
        .into_iter()
        .find(|unit| {
            let unit = *unit;
            if !caster_can_accept_order(entities, player, unit, ability)
                || !ability_orders::caster_allowed_by_faction(entities, faction_id, unit, ability)
            {
                return false;
            }
            if scout_plane::active_scout_plane_for_command_car(entities, player, unit).is_some() {
                active_blocked = true;
                return false;
            }
            true
        });
    let Some(source_command_car) = caster else {
        if active_blocked {
            notice(
                events,
                player,
                "Scout Plane already active for this Command Car",
            );
        }
        return;
    };
    let Some(ps) = players.iter_mut().find(|p| p.id == player) else {
        return;
    };
    if !ps.spend_cost(definition.cost) {
        notice(
            events,
            player,
            rules::economy::resource_shortage_notice_for_cost(ps.steel, ps.oil, definition.cost),
        );
        return;
    }
    match scout_plane::launch_ability(map, entities, player, source_command_car, x, y) {
        Ok(_) => {
            if let Some(caster) = entities.get_mut(source_command_car) {
                caster.start_ability_cooldown(ability, definition.cooldown_ticks);
            }
            notice_positioned(events, player, "Scout Plane", NoticeSeverity::Info, x, y);
        }
        Err(ScoutPlaneLaunchError::Active) => {
            ps.refund_cost(definition.cost);
            notice(
                events,
                player,
                "Scout Plane already active for this Command Car",
            );
        }
        Err(ScoutPlaneLaunchError::NoCityCentre) => {
            ps.refund_cost(definition.cost);
            notice(events, player, "Requires City Centre");
        }
    }
}
