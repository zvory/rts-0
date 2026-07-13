use crate::command_budget::{BASE_COMMAND_SUPPLY_CAP, COMMAND_CAR_SUPPLY_CAP_BONUS};
use crate::config;
use crate::game::ability::{self, AbilityEffectHook, AbilityKind, AbilityTargetMode};
use crate::game::ability_runtime::AbilityRuntime;
use crate::game::artillery::ArtilleryShellStore;
use crate::game::command::SimCommand;
use crate::game::commands::{CommandAdmission, PendingCommand};
use crate::game::entity::{
    EntityKind, EntityStore, Order, OrderIntent, ProdItem, RallyIntent, ResearchItem, WeaponSetup,
};
use crate::game::firing_reveal::{
    record_global_firing_reveals_for_enemy_players, FiringRevealSource,
};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::mortar::MortarShellStore;
use crate::game::services::ability_orders::{
    self, caster_can_accept_order, launch_self_ability, launch_world_ability,
    order_or_launch_world_ability, tech_requirement_met,
};
use crate::game::services::construction::resumable_site_for_build_intent;
use crate::game::services::dist2;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::movement::angle_delta;
use crate::game::services::order_execution::targeting::{
    artillery_point_fire_target, queued_artillery_point_fire_target,
    stored_artillery_point_fire_target, ArtilleryPointFireAcceptance,
};
use crate::game::services::order_execution::{
    begin_artillery_teardown_for_movement, execute_anti_tank_gun_setup,
    start_artillery_fire_command_order, ArtilleryFireMode, FutureOrderMode,
};
use crate::game::services::order_planner as planner;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::standability;
use crate::game::services::world_query::{self, owns_unit};
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::game::upgrade::{self, UpgradeKind};
use crate::game::PlayerState;
use crate::protocol::{self, AttackReveal, Event, NoticeSeverity};
use crate::rules::{self, combat::WeaponKind};
use std::collections::HashMap;
/// Max submitted unit ids inspected per multi-unit command. Caps the per-id work a single command
/// can force, so a repeated/huge id list can't be used to stall the tick loop.
const MAX_UNITS_PER_COMMAND: usize = 256;
const LAB_MAX_UNITS_PER_COMMAND: usize = 4096;
const MAX_RALLY_STAGES: usize = 4;
mod artillery_scatter;
mod guards;
mod planner_facts;
mod production_repeat;
mod scout_plane_ability;
#[cfg(test)]
use self::artillery_scatter::artillery_error_tiles;
use self::artillery_scatter::{artillery_blanket_point, artillery_scattered_point};
use self::guards::{
    dedupe_cap_units, is_constructing, player_is_ai, rally_intent_for_map,
    unit_can_accept_ground_command, unit_can_accept_player_command,
};
use self::planner_facts::{
    entity_order_intent_from_planner, planner_config, planner_facts, AbilityFactInput,
};
struct CommandExecutionContext<'a, 'pathing> {
    map: &'a Map,
    entities: &'a mut EntityStore,
    spatial: &'a SpatialIndex,
    coordinator: &'a mut MoveCoordinator<'pathing>,
    fog: &'a Fog,
    smokes: &'a mut SmokeCloudStore,
    ability_runtime: &'a mut AbilityRuntime,
    mortar_shells: &'a mut MortarShellStore,
    artillery_shells: &'a mut ArtilleryShellStore,
    firing_reveals: &'a mut Vec<FiringRevealSource>,
    events: &'a mut HashMap<u32, Vec<Event>>,
    teams: TeamRelations,
    tick: u32,
}
#[derive(Clone, Copy)]
struct CommandAdmissionPolicy {
    enforce_budget: bool,
    max_units_per_command: usize,
}
/// Drain + apply queued commands (validate ownership / cost / supply / tech / placement).
#[allow(clippy::too_many_arguments)]
pub(in crate::game) fn apply_commands(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    fog: &Fog,
    smokes: &mut SmokeCloudStore,
    ability_runtime: &mut AbilityRuntime,
    mortar_shells: &mut MortarShellStore,
    artillery_shells: &mut ArtilleryShellStore,
    firing_reveals: &mut Vec<FiringRevealSource>,
    pending: Vec<PendingCommand>,
    events: &mut HashMap<u32, Vec<Event>>,
    tick: u32,
) {
    let teams = TeamRelations::from_player_teams(players.iter().map(|p| (p.id, p.team_id)));
    macro_rules! command_context {
        () => {
            CommandExecutionContext {
                map,
                entities,
                spatial,
                coordinator,
                fog,
                smokes,
                ability_runtime,
                mortar_shells,
                artillery_shells,
                firing_reveals,
                events,
                teams: teams.clone(),
                tick,
            }
        };
    }
    macro_rules! apply_planned {
        ($player:expr, $facts:expr, $request:expr, $admission:expr) => {{
            let facts = $facts;
            let mut ctx = command_context!();
            planned_actions::execute(
                &mut ctx,
                players,
                $player,
                &facts,
                $request,
                $admission.max_units_per_command,
            );
        }};
    }
    macro_rules! admission_facts {
        ($player:expr, $faction_id:expr, $admission:expr, $units:expr, $ability:expr) => {
            planner_facts(
                entities,
                $player,
                $faction_id,
                &$units,
                $ability,
                $admission.max_units_per_command,
            )
        };
    }
    for pending_command in pending {
        let player = pending_command.player;
        let cmd = pending_command.command;
        let faction_id = faction_id_for(
            players.iter().map(|p| (p.id, p.faction_id.as_str())),
            player,
        );
        let command_admission = command_admission_for(
            pending_command.admission,
            player_is_ai(
                players
                    .iter()
                    .map(|candidate| (candidate.id, candidate.is_ai)),
                player,
            ),
        );
        match cmd {
            SimCommand::Move {
                units,
                x,
                y,
                queued,
            } => {
                let Some(units) =
                    validate_command_units(entities, events, player, units, command_admission)
                else {
                    continue;
                };
                let request = planner::OrderRequest {
                    units: units.clone(),
                    mode: issue_mode(queued),
                    order: planner::RequestedOrder::Move {
                        to: planner::Point::new(x, y),
                    },
                };
                apply_planned!(
                    player,
                    admission_facts!(player, &faction_id, command_admission, units, None),
                    &request,
                    command_admission
                );
            }
            SimCommand::AttackMove {
                units,
                x,
                y,
                queued,
            } => {
                let Some(units) =
                    validate_command_units(entities, events, player, units, command_admission)
                else {
                    continue;
                };
                let request = planner::OrderRequest {
                    units: units.clone(),
                    mode: issue_mode(queued),
                    order: planner::RequestedOrder::AttackMove {
                        to: planner::Point::new(x, y),
                    },
                };
                apply_planned!(
                    player,
                    admission_facts!(player, &faction_id, command_admission, units, None),
                    &request,
                    command_admission
                );
            }
            SimCommand::Attack {
                units,
                target,
                queued,
            } => {
                let Some(units) =
                    validate_command_units(entities, events, player, units, command_admission)
                else {
                    continue;
                };
                let target_valid =
                    attack_target_valid(entities, &teams, fog, smokes, player, &units, target);
                let request = planner::OrderRequest {
                    units: units.clone(),
                    mode: issue_mode(queued),
                    order: planner::RequestedOrder::AttackTarget {
                        target,
                        target_valid,
                    },
                };
                apply_planned!(
                    player,
                    admission_facts!(player, &faction_id, command_admission, units, None),
                    &request,
                    command_admission
                );
            }
            SimCommand::Deconstruct {
                units,
                target,
                queued,
            } => {
                let Some(units) =
                    validate_command_units(entities, events, player, units, command_admission)
                else {
                    continue;
                };
                let target_valid =
                    deconstruct_target_valid(entities, &teams, fog, smokes, player, &units, target);
                let target_point = entities
                    .get(target)
                    .map(|target| planner::Point::new(target.pos_x, target.pos_y))
                    .unwrap_or_else(|| planner::Point::new(0.0, 0.0));
                let request = planner::OrderRequest {
                    units: units.clone(),
                    mode: issue_mode(queued),
                    order: planner::RequestedOrder::Deconstruct {
                        target,
                        target_point,
                        target_valid,
                    },
                };
                apply_planned!(
                    player,
                    admission_facts!(player, &faction_id, command_admission, units, None),
                    &request,
                    command_admission
                );
            }
            SimCommand::SetupAntiTankGuns {
                units,
                x,
                y,
                queued,
            } => {
                let Some(units) =
                    validate_command_units(entities, events, player, units, command_admission)
                else {
                    continue;
                };
                let request = planner::OrderRequest {
                    units: units.clone(),
                    mode: issue_mode(queued),
                    order: planner::RequestedOrder::SetupAntiTankGuns {
                        face_toward: planner::Point::new(x, y),
                    },
                };
                let facts = admission_facts!(player, &faction_id, command_admission, units, None);
                apply_planned!(player, facts, &request, command_admission);
            }
            SimCommand::TearDownAntiTankGuns { units } => {
                let Some(units) =
                    validate_command_units(entities, events, player, units, command_admission)
                else {
                    continue;
                };
                for id in units {
                    if !unit_can_accept_player_command(entities, player, id) {
                        continue;
                    }
                    let Some(e) = entities.get_mut(id) else {
                        continue;
                    };
                    if !matches!(e.kind, EntityKind::AntiTankGun | EntityKind::Artillery) {
                        continue;
                    }
                    if matches!(
                        e.weapon_setup(),
                        WeaponSetup::SettingUp { .. } | WeaponSetup::Deployed
                    ) {
                        e.clear_orders();
                        e.set_path_goal(None);
                        e.set_weapon_setup(WeaponSetup::TearingDown {
                            ticks: setup_ticks_for(e.kind),
                        });
                    } else if matches!(e.weapon_setup(), WeaponSetup::Packed) {
                        e.set_emplacement_facing(None);
                        e.set_pending_redeploy_facing(None);
                    }
                }
            }
            SimCommand::UseAbility {
                ability,
                units,
                x,
                y,
                queued,
            } => {
                let Some(units) =
                    validate_command_units(entities, events, player, units, command_admission)
                else {
                    continue;
                };
                let mut ctx = command_context!();
                use_ability(
                    &mut ctx,
                    players,
                    player,
                    AbilityUse {
                        ability,
                        units,
                        x,
                        y,
                        queued,
                        max_units_per_command: command_admission.max_units_per_command,
                    },
                );
            }
            SimCommand::RecastAbility {
                ability,
                units,
                target_object_id,
                queued: _,
            } => {
                let Some(units) =
                    validate_command_units(entities, events, player, units, command_admission)
                else {
                    continue;
                };
                ability_orders::execute_recast_return(
                    map,
                    entities,
                    ability_runtime,
                    events,
                    player,
                    &faction_id,
                    ability,
                    units,
                    target_object_id,
                    tick,
                );
            }
            SimCommand::SetAutocast {
                ability,
                units,
                enabled,
            } => {
                let Some(units) =
                    validate_command_units(entities, events, player, units, command_admission)
                else {
                    continue;
                };
                let definition = ability::definition(ability);
                if !definition.autocast {
                    continue;
                }
                if ability == AbilityKind::MortarFire
                    && !players.iter().any(|p| {
                        p.id == player && p.upgrades.contains(&UpgradeKind::MortarAutocast)
                    })
                {
                    continue;
                }
                for id in units {
                    if owns_unit(entities, player, id)
                        && ability_orders::caster_allowed_by_faction(
                            entities,
                            &faction_id,
                            id,
                            ability,
                        )
                    {
                        if let Some(e) = entities.get_mut(id) {
                            e.set_autocast_enabled(ability, enabled);
                        }
                    }
                }
            }
            SimCommand::Gather {
                units,
                node,
                queued,
            } => {
                let Some(units) =
                    validate_command_units(entities, events, player, units, command_admission)
                else {
                    continue;
                };
                let node_valid = gather_node_valid(entities, player, node);
                let request = planner::OrderRequest {
                    units: units.clone(),
                    mode: issue_mode(queued),
                    order: planner::RequestedOrder::Gather { node, node_valid },
                };
                apply_planned!(
                    player,
                    admission_facts!(player, &faction_id, command_admission, units, None),
                    &request,
                    command_admission
                );
            }
            SimCommand::Build {
                units,
                building,
                tile_x,
                tile_y,
                queued,
            } => {
                let Some(units) =
                    validate_command_units(entities, events, player, units, command_admission)
                else {
                    continue;
                };
                let (target_x, target_y) = build_target_center(building, tile_x, tile_y);
                let request = planner::OrderRequest {
                    units: units.clone(),
                    mode: issue_mode(queued),
                    order: planner::RequestedOrder::Build {
                        kind: build_kind_code(building),
                        tile_x,
                        tile_y,
                        target: planner::Point::new(target_x, target_y),
                        placement_valid: true,
                    },
                };
                apply_planned!(
                    player,
                    admission_facts!(player, &faction_id, command_admission, units, None),
                    &request,
                    command_admission
                );
            }
            SimCommand::Train { building, unit } => {
                order_train(entities, players, player, building, unit, events);
            }
            SimCommand::SetProductionRepeat {
                buildings,
                unit,
                enabled,
            } => {
                production_repeat::set(
                    entities,
                    &faction_id,
                    player,
                    buildings,
                    unit,
                    enabled,
                    command_admission.max_units_per_command,
                );
            }
            SimCommand::Research { building, upgrade } => {
                let definition = upgrade::definition(upgrade);
                let Some(building_kind) = entities.get(building).map(|b| b.kind) else {
                    notice(events, player, "Cannot research that here");
                    continue;
                };
                let ok = matches!(entities.get(building), Some(b)
                if b.owner == player && b.is_building() && !b.under_construction()
                && b.kind == definition.researched_at
                && rules::economy::can_research_for_faction(
                    &faction_id,
                    upgrade.to_protocol_str(),
                    building_kind,
                ));
                if !ok {
                    notice(events, player, "Cannot research that here");
                    continue;
                }
                if entities.iter().any(|e| {
                    e.owner == player
                        && e.research_queue()
                            .iter()
                            .any(|item| item.upgrade == upgrade)
                }) {
                    notice(events, player, "Already researching");
                    continue;
                }

                let Some(ps) = players.iter_mut().find(|p| p.id == player) else {
                    continue;
                };
                if ps.upgrades.contains(&upgrade) {
                    notice(events, player, "Already researched");
                    continue;
                }
                if definition
                    .requires_upgrade
                    .is_some_and(|required| !ps.upgrades.contains(&required))
                {
                    notice(events, player, "Requirement not met");
                    continue;
                }
                let cost =
                    rules::economy::ResourceCost::new(definition.cost_steel, definition.cost_oil);
                if !ps.can_afford(cost.steel, cost.oil) {
                    notice(
                        events,
                        player,
                        rules::economy::resource_shortage_notice_for_cost(ps.steel, ps.oil, cost),
                    );
                    continue;
                }
                if !ps.spend_cost(cost) {
                    notice(
                        events,
                        player,
                        rules::economy::resource_shortage_notice_for_cost(ps.steel, ps.oil, cost),
                    );
                    continue;
                }

                let queued = entities.get_mut(building).is_some_and(|b| {
                    b.push_research(ResearchItem {
                        upgrade,
                        progress: 0,
                        total: definition.research_ticks,
                    })
                });
                if !queued {
                    ps.refund_cost(cost);
                }
            }
            SimCommand::Cancel { building } => {
                order_cancel(entities, players, player, building);
            }
            SimCommand::Stop { units } => {
                let Some(units) =
                    validate_command_units(entities, events, player, units, command_admission)
                else {
                    continue;
                };
                for id in units {
                    if unit_can_accept_ground_command(entities, player, id) {
                        entities.release_miner(id);
                        if let Some(e) = entities.get_mut(id) {
                            e.clear_orders();
                            e.clear_worker_carry();
                        }
                    }
                }
            }
            SimCommand::HoldPosition { units, queued } => {
                let Some(units) =
                    validate_command_units(entities, events, player, units, command_admission)
                else {
                    continue;
                };
                let request = planner::OrderRequest {
                    units: units.clone(),
                    mode: issue_mode(queued),
                    order: planner::RequestedOrder::HoldPosition,
                };
                apply_planned!(
                    player,
                    admission_facts!(player, &faction_id, command_admission, units, None),
                    &request,
                    command_admission
                );
            }
            SimCommand::SetRally {
                building,
                x,
                y,
                kind,
                queued,
            } => {
                let Some(rally) = rally_intent_for_map(map, kind, x, y) else {
                    continue;
                };
                order_set_rally(entities, &faction_id, player, building, rally, queued);
            }
            SimCommand::Rejected { reason } => {
                notice(events, player, reason.notice_message());
            }
        }
    }
}

fn validate_command_units(
    entities: &EntityStore,
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    units: Vec<u32>,
    admission: CommandAdmissionPolicy,
) -> Option<Vec<u32>> {
    let units = dedupe_cap_units(units, admission.max_units_per_command);
    if admission.enforce_budget && guards::command_budget_exceeded(entities, player, &units) {
        notice(events, player, "Command supply exceeded");
        return None;
    }
    Some(units)
}

fn command_admission_for(
    admission: CommandAdmission,
    player_is_ai: bool,
) -> CommandAdmissionPolicy {
    match admission {
        CommandAdmission::Normal => CommandAdmissionPolicy {
            enforce_budget: !player_is_ai,
            max_units_per_command: MAX_UNITS_PER_COMMAND,
        },
        CommandAdmission::LabIgnoreCommandLimits => CommandAdmissionPolicy {
            enforce_budget: false,
            max_units_per_command: LAB_MAX_UNITS_PER_COMMAND,
        },
    }
}

fn issue_mode(queued: bool) -> planner::IssueMode {
    if queued {
        planner::IssueMode::Queue
    } else {
        planner::IssueMode::Immediate
    }
}

fn faction_id_for<'a>(mut players: impl Iterator<Item = (u32, &'a str)>, player: u32) -> String {
    players
        .find(|(id, _)| *id == player)
        .map(|(_, faction_id)| faction_id.to_string())
        .unwrap_or_else(|| rules::faction::DEFAULT_FACTION_ID.to_string())
}

fn build_target_center(building: EntityKind, tile_x: u32, tile_y: u32) -> (f32, f32) {
    let Some(stats) = config::building_stats(building) else {
        return (0.0, 0.0);
    };
    let ts = config::TILE_SIZE as f32;
    (
        tile_x as f32 * ts + stats.foot_w as f32 * ts * 0.5,
        tile_y as f32 * ts + stats.foot_h as f32 * ts * 0.5,
    )
}

mod planned_actions {
    use super::*;
    pub(super) fn execute(
        ctx: &mut CommandExecutionContext<'_, '_>,
        players: &mut [PlayerState],
        player: u32,
        facts: &[planner::UnitFacts],
        request: &planner::OrderRequest,
        max_units_per_command: usize,
    ) {
        let faction_id = faction_id_for(
            players.iter().map(|p| (p.id, p.faction_id.as_str())),
            player,
        );
        let map = ctx.map;
        let entities = &mut *ctx.entities;
        let spatial = ctx.spatial;
        let coordinator = &mut *ctx.coordinator;
        let teams = &ctx.teams;
        let fog = ctx.fog;
        let smokes = &mut *ctx.smokes;
        let ability_runtime = &mut *ctx.ability_runtime;
        let mortar_shells = &mut *ctx.mortar_shells;
        let artillery_shells = &mut *ctx.artillery_shells;
        let firing_reveals = &mut *ctx.firing_reveals;
        let events = &mut *ctx.events;
        let tick = ctx.tick;
        let output = planner::plan_order(planner_config(max_units_per_command), facts, request);
        let mut move_units = Vec::new();
        let mut attack_move_units = Vec::new();
        let mut move_goal = None;
        let mut attack_move_goal = None;
        for action in output.actions {
            match action {
                planner::PlannedAction::ReplaceActive { unit, intent } => match intent {
                    planner::OrderIntent::Move(point) => {
                        if immediate_unit_can_replace(entities, player, unit) {
                            move_goal = Some((point.x, point.y));
                            move_units.push(unit);
                        }
                    }
                    planner::OrderIntent::AttackMove(point) => {
                        if immediate_unit_can_replace(entities, player, unit) {
                            attack_move_goal = Some((point.x, point.y));
                            attack_move_units.push(unit);
                        }
                    }
                    planner::OrderIntent::HoldPosition => {
                        if unit_can_accept_ground_command(entities, player, unit) {
                            entities.release_miner(unit);
                            if let Some(e) = entities.get_mut(unit) {
                                e.hold_position();
                                e.clear_worker_carry();
                            }
                        }
                    }
                    planner::OrderIntent::AttackTarget(target) => {
                        if immediate_unit_can_replace(entities, player, unit)
                            && attack_target_valid(
                                entities,
                                teams,
                                fog,
                                smokes,
                                player,
                                &[unit],
                                target,
                            )
                            && !deployed_anti_tank_gun_target_outside_arc(entities, unit, target)
                        {
                            if let Some(e) = entities.get_mut(unit) {
                                e.clear_queued_orders();
                            }
                            clear_staged_anti_tank_gun_setup(entities, &[unit]);
                            coordinator.order_attack(entities, unit, target);
                        }
                    }
                    planner::OrderIntent::Gather(node) => {
                        if gather_unit_can_use_node(entities, players, player, unit, node) {
                            if let Some(e) = entities.get_mut(unit) {
                                e.clear_queued_orders();
                            }
                            coordinator.order_gather(entities, unit, node);
                        }
                    }
                    planner::OrderIntent::Build {
                        kind,
                        tile_x,
                        tile_y,
                    } => {
                        let Some(building) = build_kind_from_code(kind) else {
                            continue;
                        };
                        order_build(
                            map,
                            entities,
                            players,
                            spatial,
                            coordinator,
                            player,
                            unit,
                            building,
                            tile_x,
                            tile_y,
                            events,
                        );
                    }
                    planner::OrderIntent::Deconstruct(target) => {
                        if immediate_unit_can_replace(entities, player, unit)
                            && deconstruct_unit_can_target(
                                entities, teams, fog, smokes, player, unit, target,
                            )
                        {
                            if let Some(e) = entities.get_mut(unit) {
                                e.clear_queued_orders();
                            }
                            clear_staged_anti_tank_gun_setup(entities, &[unit]);
                            coordinator.order_deconstruct(entities, unit, target);
                        }
                    }
                    planner::OrderIntent::SetupAntiTankGuns { face_toward } => {
                        if immediate_unit_can_replace(entities, player, unit) {
                            execute_anti_tank_gun_setup(
                                entities,
                                unit,
                                face_toward.x,
                                face_toward.y,
                                FutureOrderMode::Clear,
                            );
                        }
                    }
                    planner::OrderIntent::WorldAbility { ability, target } => {
                        let Some(ability) = ability_from_planner(ability) else {
                            continue;
                        };
                        if let Some(mode) = artillery_fire_mode_for(ability) {
                            order_artillery_point_fire(
                                map,
                                entities,
                                players,
                                teams,
                                fog,
                                artillery_shells,
                                firing_reveals,
                                events,
                                player,
                                unit,
                                target.x,
                                target.y,
                                tick,
                                mode,
                            );
                            continue;
                        }
                        if !immediate_unit_can_replace(entities, player, unit) {
                            continue;
                        }
                        if let Some(e) = entities.get_mut(unit) {
                            e.clear_queued_orders();
                        }
                        clear_staged_anti_tank_gun_setup(entities, &[unit]);
                        order_or_launch_world_ability(
                            map,
                            entities,
                            players,
                            fog,
                            teams,
                            coordinator,
                            smokes,
                            ability_runtime,
                            mortar_shells,
                            events,
                            player,
                            &faction_id,
                            unit,
                            ability,
                            target.x,
                            target.y,
                            tick,
                            true,
                        );
                    }
                    planner::OrderIntent::SelfAbility { ability } => {
                        if let Some(ability) = ability_from_planner(ability) {
                            launch_self_ability(
                                entities,
                                events,
                                &faction_id,
                                player,
                                unit,
                                ability,
                            );
                        }
                    }
                },
                planner::PlannedAction::AppendQueued { unit, intent } => {
                    if let planner::OrderIntent::WorldAbility { ability, target } = intent {
                        if let Some(mode) =
                            ability_from_planner(ability).and_then(artillery_fire_mode_for)
                        {
                            if let Some(locked) = queued_artillery_point_fire_target(
                                map, entities, player, unit, target.x, target.y,
                            ) {
                                if let Some(e) = entities.get_mut(unit) {
                                    let intent = match mode {
                                        ArtilleryFireMode::Point => {
                                            OrderIntent::point_fire(locked.x, locked.y)
                                        }
                                        ArtilleryFireMode::Blanket => {
                                            OrderIntent::blanket_fire(locked.x, locked.y)
                                        }
                                    };
                                    e.append_queued_order(intent);
                                }
                            }
                            continue;
                        }
                    }
                    if let Some(intent) = entity_order_intent_from_planner(intent) {
                        match &intent {
                            OrderIntent::Attack(attack)
                                if !attack_target_valid(
                                    entities,
                                    teams,
                                    fog,
                                    smokes,
                                    player,
                                    &[unit],
                                    attack.target,
                                ) =>
                            {
                                continue;
                            }
                            OrderIntent::Deconstruct(deconstruct)
                                if !deconstruct_unit_can_target(
                                    entities,
                                    teams,
                                    fog,
                                    smokes,
                                    player,
                                    unit,
                                    deconstruct.target,
                                ) =>
                            {
                                continue;
                            }
                            _ => {}
                        }
                        if let Some(e) = entities.get_mut(unit) {
                            e.append_queued_order(intent);
                        }
                    }
                }
                planner::PlannedAction::ExecuteAbilityNow {
                    unit,
                    ability,
                    target,
                    preserve_orders,
                } => {
                    let Some(ability) = ability_from_planner(ability) else {
                        continue;
                    };
                    match target {
                        planner::AbilityTarget::SelfTarget => {
                            launch_self_ability(
                                entities,
                                events,
                                &faction_id,
                                player,
                                unit,
                                ability,
                            );
                        }
                        planner::AbilityTarget::WorldPoint(point) => {
                            if let Some(mode) = artillery_fire_mode_for(ability) {
                                order_artillery_point_fire(
                                    map,
                                    entities,
                                    players,
                                    teams,
                                    fog,
                                    artillery_shells,
                                    firing_reveals,
                                    events,
                                    player,
                                    unit,
                                    point.x,
                                    point.y,
                                    tick,
                                    mode,
                                );
                                continue;
                            }
                            launch_world_ability(
                                map,
                                entities,
                                players,
                                fog,
                                teams,
                                smokes,
                                ability_runtime,
                                mortar_shells,
                                events,
                                player,
                                &faction_id,
                                unit,
                                ability,
                                point.x,
                                point.y,
                                tick,
                                preserve_orders,
                                true,
                            );
                        }
                    }
                }
            }
        }

        if let Some(goal) = move_goal.filter(|_| !move_units.is_empty()) {
            clear_queued_orders(entities, &move_units);
            clear_staged_anti_tank_gun_setup(entities, &move_units);
            coordinator.order_group_move(entities, player, &move_units, goal, false);
            begin_artillery_teardown_for_movement(entities, &move_units);
        }
        if let Some(goal) = attack_move_goal.filter(|_| !attack_move_units.is_empty()) {
            clear_queued_orders(entities, &attack_move_units);
            clear_staged_anti_tank_gun_setup(entities, &attack_move_units);
            coordinator.order_group_move(entities, player, &attack_move_units, goal, true);
            begin_artillery_teardown_for_movement(entities, &attack_move_units);
        }
        for planner_notice in output.notices {
            match planner_notice {
                planner::PlannerNotice::QueueFull { .. } => {
                    notice(events, player, "Command queue full");
                }
            }
        }
    }
}
fn immediate_unit_can_replace(entities: &EntityStore, player: u32, unit: u32) -> bool {
    unit_can_accept_player_command(entities, player, unit)
}
fn attack_target_valid(
    entities: &EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    player: u32,
    units: &[u32],
    target: u32,
) -> bool {
    units.iter().copied().any(|unit| {
        world_query::unit_explicit_attack_target_valid(
            entities,
            teams,
            fog,
            Some(smokes),
            player,
            unit,
            target,
        )
    })
}

fn deconstruct_target_valid(
    entities: &EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    player: u32,
    units: &[u32],
    target: u32,
) -> bool {
    units
        .iter()
        .copied()
        .any(|unit| deconstruct_unit_can_target(entities, teams, fog, smokes, player, unit, target))
}

fn deconstruct_unit_can_target(
    entities: &EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    player: u32,
    unit: u32,
    target: u32,
) -> bool {
    if !matches!(entities.get(unit), Some(e) if e.owner == player && e.kind == EntityKind::Worker && e.hp > 0)
    {
        return false;
    }
    let Some(target) = entities.get(target) else {
        return false;
    };
    if target.kind != EntityKind::TankTrap || target.hp == 0 || target.under_construction() {
        return false;
    }
    teams.same_team_or_same_owner(player, target.owner)
        || (rules::projection::team_visible_world(player, target.pos_x, target.pos_y, fog, teams)
            && !smokes.point_inside(target.pos_x, target.pos_y))
}

fn gather_node_valid(entities: &EntityStore, player: u32, node: u32) -> bool {
    world_query::steel_node_is_mineable_by_player(entities, player, node)
}

fn gather_unit_can_use_node(
    entities: &EntityStore,
    players: &[PlayerState],
    player: u32,
    unit: u32,
    node: u32,
) -> bool {
    let faction_id = faction_id_for(
        players.iter().map(|p| (p.id, p.faction_id.as_str())),
        player,
    );
    owns_unit(entities, player, unit)
        && matches!(entities.get(unit), Some(e) if rules::economy::can_gather_for_faction(&faction_id, e.kind))
        && gather_node_valid(entities, player, node)
}

fn build_kind_code(kind: EntityKind) -> planner::BuildKind {
    EntityKind::ALL
        .iter()
        .position(|candidate| *candidate == kind)
        .unwrap_or(usize::MAX) as planner::BuildKind
}

fn build_kind_from_code(code: planner::BuildKind) -> Option<EntityKind> {
    EntityKind::ALL.get(code as usize).copied()
}

fn ability_to_planner(ability: AbilityKind) -> planner::AbilityId {
    planner::AbilityId(ability.to_planner_code())
}

fn ability_from_planner(ability: planner::AbilityId) -> Option<AbilityKind> {
    AbilityKind::from_planner_code(ability.0)
}

fn artillery_fire_mode_for(ability: AbilityKind) -> Option<ArtilleryFireMode> {
    match ability {
        AbilityKind::PointFire => Some(ArtilleryFireMode::Point),
        AbilityKind::BlanketFire => Some(ArtilleryFireMode::Blanket),
        _ => None,
    }
}

struct AbilityUse {
    ability: AbilityKind,
    x: Option<f32>,
    y: Option<f32>,
    units: Vec<u32>,
    queued: bool,
    max_units_per_command: usize,
}

fn use_ability(
    ctx: &mut CommandExecutionContext<'_, '_>,
    players: &mut [PlayerState],
    player: u32,
    request: AbilityUse,
) {
    let faction_id = faction_id_for(
        players.iter().map(|p| (p.id, p.faction_id.as_str())),
        player,
    );
    let map = ctx.map;
    let entities = &mut *ctx.entities;
    let fog = ctx.fog;
    let artillery_shells = &mut *ctx.artillery_shells;
    let firing_reveals = &mut *ctx.firing_reveals;
    let events = &mut *ctx.events;
    let teams = &ctx.teams;
    let tick = ctx.tick;

    let ability = request.ability;
    let definition = ability::definition(ability);
    if request.queued && !definition.may_queue {
        return;
    }
    if definition.effect_hook == AbilityEffectHook::ReservedNoop {
        return;
    }
    if ability == AbilityKind::ScoutPlane {
        scout_plane_ability::use_ability(
            map,
            entities,
            players,
            events,
            player,
            &faction_id,
            request,
        );
        return;
    }
    if let Some(mode) = artillery_fire_mode_for(ability) {
        let Some(x) = request.x else {
            return;
        };
        let Some(y) = request.y else {
            return;
        };
        for unit in dedupe_cap_units(request.units, request.max_units_per_command) {
            if !ability_orders::caster_allowed_by_faction(entities, &faction_id, unit, ability) {
                continue;
            }
            if request.queued {
                if let Some(target) =
                    queued_artillery_point_fire_target(map, entities, player, unit, x, y)
                {
                    if let Some(e) = entities.get_mut(unit) {
                        let intent = match mode {
                            ArtilleryFireMode::Point => OrderIntent::point_fire(target.x, target.y),
                            ArtilleryFireMode::Blanket => {
                                OrderIntent::blanket_fire(target.x, target.y)
                            }
                        };
                        e.append_queued_order(intent);
                    }
                }
            } else {
                order_artillery_point_fire(
                    map,
                    entities,
                    players,
                    teams,
                    fog,
                    artillery_shells,
                    firing_reveals,
                    events,
                    player,
                    unit,
                    x,
                    y,
                    tick,
                    mode,
                );
            }
        }
        return;
    }
    let planner_id = ability_to_planner(ability);
    let tech_ready = tech_requirement_met(entities, player, ability);

    let (target, target_point) = match definition.target_mode {
        AbilityTargetMode::SelfTarget => (planner::AbilityTarget::SelfTarget, None),
        AbilityTargetMode::WorldPoint => {
            let Some(x) = request.x else {
                return;
            };
            let Some(y) = request.y else {
                return;
            };
            let Some((x, y)) = SmokeCloudStore::clamp_point_to_map(map, x, y) else {
                return;
            };
            (
                planner::AbilityTarget::WorldPoint(planner::Point::new(x, y)),
                Some((x, y)),
            )
        }
    };

    let units = if !request.queued {
        if let Some((x, y)) = target_point {
            let eligible: Vec<u32> =
                dedupe_cap_units(request.units.clone(), request.max_units_per_command)
                    .into_iter()
                    .filter(|id| caster_can_accept_order(entities, player, *id, ability))
                    .collect();
            match choose_smoke_caster(map, entities, ability, &eligible, x, y) {
                Some(caster) => vec![caster],
                None => request.units.clone(),
            }
        } else {
            request.units.clone()
        }
    } else {
        request.units.clone()
    };

    let facts = planner_facts(
        entities,
        player,
        &faction_id,
        &units,
        Some(AbilityFactInput {
            kind: ability,
            id: planner_id,
            tech_ready,
            target: target_point,
            map,
        }),
        request.max_units_per_command,
    );
    let order = planner::OrderRequest {
        units,
        mode: issue_mode(request.queued),
        order: planner::RequestedOrder::UseAbility {
            ability: planner_id,
            target,
        },
    };
    planned_actions::execute(
        ctx,
        players,
        player,
        &facts,
        &order,
        request.max_units_per_command,
    );
}

#[allow(clippy::too_many_arguments)]
fn order_artillery_point_fire(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    teams: &TeamRelations,
    fog: &Fog,
    artillery_shells: &mut ArtilleryShellStore,
    firing_reveals: &mut Vec<FiringRevealSource>,
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    unit: u32,
    x: f32,
    y: f32,
    tick: u32,
    mode: ArtilleryFireMode,
) -> bool {
    let Some(target) = artillery_point_fire_target(
        map,
        entities,
        player,
        unit,
        x,
        y,
        ArtilleryPointFireAcceptance::Command,
    ) else {
        return false;
    };
    if !start_artillery_fire_command_order(entities, unit, target, mode) {
        return false;
    }
    if !target.inside_field_of_fire {
        return true;
    }
    try_fire_artillery(
        entities,
        players,
        teams,
        fog,
        artillery_shells,
        firing_reveals,
        events,
        player,
        unit,
        target.x,
        target.y,
        tick,
        mode,
    )
}

#[allow(clippy::too_many_arguments)]
fn try_fire_artillery(
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    teams: &TeamRelations,
    fog: &Fog,
    artillery_shells: &mut ArtilleryShellStore,
    firing_reveals: &mut Vec<FiringRevealSource>,
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    unit: u32,
    x: f32,
    y: f32,
    tick: u32,
    mode: ArtilleryFireMode,
) -> bool {
    let ready = matches!(entities.get(unit), Some(e)
        if e.owner == player
            && e.kind == EntityKind::Artillery
            && e.hp > 0
            && e.attack_cd() == 0
            && matches!(e.weapon_setup(), WeaponSetup::Deployed));
    if !ready {
        return false;
    }
    let faction_id = faction_id_for(
        players.iter().map(|p| (p.id, p.faction_id.as_str())),
        player,
    );
    let ability = match mode {
        ArtilleryFireMode::Point => AbilityKind::PointFire,
        ArtilleryFireMode::Blanket => AbilityKind::BlanketFire,
    };
    if !ability_orders::caster_allowed_by_faction(entities, &faction_id, unit, ability) {
        return false;
    }
    let ammo_cost = ability::definition(ability).cost;
    let Some(ps) = players.iter_mut().find(|p| p.id == player) else {
        return false;
    };
    if !ps.can_afford(ammo_cost.steel, ammo_cost.oil) {
        notice(events, player, "Not enough steel");
        if let Some(e) = entities.get_mut(unit) {
            e.set_attack_cd(config::ARTILLERY_RELOAD_TICKS);
        }
        return false;
    }
    if !ps.spend_cost(ammo_cost) {
        notice(events, player, "Not enough steel");
        return false;
    }
    let ballistic_tables_researched =
        mode == ArtilleryFireMode::Point && ps.has_upgrade(UpgradeKind::BallisticTables);
    let (target_x, target_y) = {
        let Some(e) = entities.get_mut(unit) else {
            ps.refund_cost(ammo_cost);
            return false;
        };
        let shot_number = match mode {
            ArtilleryFireMode::Point if ballistic_tables_researched => {
                e.increment_artillery_shots_fired()
            }
            ArtilleryFireMode::Point => {
                e.reset_artillery_accuracy();
                1
            }
            ArtilleryFireMode::Blanket => e.increment_artillery_blanket_shots_fired(),
        };
        e.set_attack_cd(config::ARTILLERY_RELOAD_TICKS);
        match mode {
            ArtilleryFireMode::Point => artillery_scattered_point(
                unit,
                tick,
                (e.pos_x, e.pos_y),
                (x, y),
                shot_number,
                ballistic_tables_researched,
            ),
            ArtilleryFireMode::Blanket => {
                artillery_blanket_point(unit, player, tick, (x, y), shot_number)
            }
        }
    };
    let reveal = entities.get(unit).map(|attacker| AttackReveal {
        owner: attacker.owner,
        kind: protocol::kind_to_wire(attacker.kind).to_string(),
        x: attacker.pos_x,
        y: attacker.pos_y,
        facing: Some(attacker.facing()),
        weapon_facing: attacker.weapon_facing(),
        setup_state: Some(attacker.weapon_setup().to_protocol_str().to_string()),
    });
    artillery_shells.schedule(player, unit, target_x, target_y, tick);
    if let Some(reveal) = reveal.as_ref() {
        let facing = reveal.weapon_facing.or(reveal.facing).unwrap_or(0.0);
        let player_ids: Vec<u32> = events.keys().copied().collect();
        record_global_firing_reveals_for_enemy_players(
            firing_reveals,
            &player_ids,
            teams,
            player,
            unit,
            tick,
            config::ARTILLERY_RELOAD_TICKS,
        );
        for pid in player_ids {
            events.entry(pid).or_default().push(Event::ArtilleryFiring {
                owner: reveal.owner,
                x: reveal.x,
                y: reveal.y,
                facing,
            });
        }
    }
    for pid in events.keys().copied().collect::<Vec<_>>() {
        if teams.same_team_or_same_owner(pid, player) {
            events.entry(pid).or_default().push(Event::ArtilleryTarget {
                from: unit,
                x: target_x,
                y: target_y,
                radius_tiles: config::ARTILLERY_OUTER_RADIUS_TILES,
                delay_ticks: config::ARTILLERY_SHELL_DELAY_TICKS,
            });
        }
    }
    if let Some(reveal) = reveal {
        let player_ids: Vec<u32> = events.keys().copied().collect();
        for pid in player_ids {
            if teams.same_team_or_same_owner(pid, player)
                || !crate::rules::projection::team_visible_world(
                    pid, reveal.x, reveal.y, fog, teams,
                )
            {
                continue;
            }
            events.entry(pid).or_default().push(Event::Attack {
                from: unit,
                to: unit,
                reveal: Some(reveal.clone()),
                to_pos: None,
                weapon_kind: Some(WeaponKind::ArtilleryGun.stable_id().to_string()),
            });
        }
    }
    true
}
#[allow(clippy::too_many_arguments)]
pub(in crate::game) fn artillery_point_fire_system(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    artillery_shells: &mut ArtilleryShellStore,
    firing_reveals: &mut Vec<FiringRevealSource>,
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    tick: u32,
) {
    let teams = TeamRelations::from_player_teams(players.iter().map(|p| (p.id, p.team_id)));
    let orders: Vec<(u32, u32, f32, f32, ArtilleryFireMode)> = entities
        .ids()
        .into_iter()
        .filter_map(|id| {
            let e = entities.get(id)?;
            let (x, y, mode) = match e.order() {
                Order::ArtilleryPointFire(order) => {
                    (order.intent.x, order.intent.y, ArtilleryFireMode::Point)
                }
                Order::ArtilleryBlanketFire(order) => {
                    (order.intent.x, order.intent.y, ArtilleryFireMode::Blanket)
                }
                _ => return None,
            };
            Some((id, e.owner, x, y, mode))
        })
        .collect();
    for (id, owner, x, y, mode) in orders {
        let target = stored_artillery_point_fire_target(
            map,
            entities,
            owner,
            id,
            x,
            y,
            ArtilleryPointFireAcceptance::Command,
        );
        if !target.is_some_and(|target| target.inside_field_of_fire) {
            if matches!(
                entities.get(id).map(|e| e.weapon_setup()),
                Some(
                    WeaponSetup::Packed
                        | WeaponSetup::SettingUp { .. }
                        | WeaponSetup::TearingDown { .. }
                        | WeaponSetup::TearingDownToRedeploy { .. }
                )
            ) && stored_artillery_point_fire_target(
                map,
                entities,
                owner,
                id,
                x,
                y,
                ArtilleryPointFireAcceptance::BasicTarget,
            )
            .is_some()
            {
                continue;
            }
            if let Some(e) = entities.get_mut(id) {
                e.clear_active_order();
            }
            continue;
        }
        try_fire_artillery(
            entities,
            players,
            &teams,
            fog,
            artillery_shells,
            firing_reveals,
            events,
            owner,
            id,
            x,
            y,
            tick,
            mode,
        );
    }
}

fn setup_ticks_for(kind: EntityKind) -> u16 {
    match kind {
        EntityKind::Artillery => config::ARTILLERY_SETUP_TICKS,
        _ => config::ANTI_TANK_GUN_SETUP_TICKS,
    }
}

fn choose_smoke_caster(
    map: &Map,
    entities: &EntityStore,
    ability: AbilityKind,
    eligible: &[u32],
    x: f32,
    y: f32,
) -> Option<u32> {
    let mut furthest_in_range: Option<(u32, f32)> = None;
    let mut closest: Option<(u32, f32)> = None;
    for id in eligible {
        let Some(e) = entities.get(*id) else {
            continue;
        };
        let d2 = dist2(e.pos_x, e.pos_y, x, y);
        if closest.is_none_or(|(_, best)| d2 < best) {
            closest = Some((*id, d2));
        }
        if ability_orders::caster_in_range(map, entities, *id, ability, x, y)
            && furthest_in_range.is_none_or(|(_, best)| d2 > best)
        {
            furthest_in_range = Some((*id, d2));
        }
    }
    furthest_in_range.or(closest).map(|(id, _)| id)
}

fn clear_queued_orders(entities: &mut EntityStore, ids: &[u32]) {
    for id in ids {
        if let Some(e) = entities.get_mut(*id) {
            e.clear_queued_orders();
        }
    }
}

fn clear_staged_anti_tank_gun_setup(entities: &mut EntityStore, ids: &[u32]) {
    for id in ids {
        let Some(e) = entities.get_mut(*id) else {
            continue;
        };
        if e.kind == EntityKind::AntiTankGun {
            e.set_emplacement_facing(None);
            e.set_pending_redeploy_facing(None);
        }
    }
}

fn deployed_anti_tank_gun_target_outside_arc(entities: &EntityStore, id: u32, target: u32) -> bool {
    let Some(attacker) = entities.get(id) else {
        return false;
    };
    if attacker.kind != EntityKind::AntiTankGun
        || !matches!(attacker.weapon_setup(), WeaponSetup::Deployed)
    {
        return false;
    }
    let Some(center) = attacker
        .emplacement_facing()
        .or_else(|| attacker.weapon_facing())
        .filter(|facing| facing.is_finite())
    else {
        return false;
    };
    let Some(target) = entities.get(target) else {
        return false;
    };
    let target_angle = (target.pos_y - attacker.pos_y).atan2(target.pos_x - attacker.pos_x);
    if !target_angle.is_finite() {
        return true;
    }
    angle_delta(center, target_angle).abs() > config::ANTI_TANK_GUN_FIELD_OF_FIRE_RAD * 0.5
}

/// Issue a build order under the "reserve on arrival" model. Validates intent, emits
/// best-effort feedback notices to the player, then walks the worker toward the target
/// tile. Resources are not deducted and no building is spawned here; that happens in the
/// construction system when the worker arrives, at which point placement and affordability
/// are re-checked. Other units may walk through the tile and other build commands may race
/// for it — first arrival wins.
#[allow(clippy::too_many_arguments)]
fn order_build(
    map: &Map,
    entities: &mut EntityStore,
    players: &[PlayerState],
    _spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    player: u32,
    worker: u32,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    if !owns_unit(entities, player, worker) {
        return;
    }
    let faction_id = faction_id_for(
        players.iter().map(|p| (p.id, p.faction_id.as_str())),
        player,
    );
    let worker_kind = entities.get(worker).map(|e| e.kind);
    if !matches!(worker_kind, Some(kind) if rules::economy::can_build_for_faction(&faction_id, kind, building))
    {
        let msg = if worker_kind == Some(EntityKind::Worker) {
            "Building unavailable"
        } else {
            "Only workers can build"
        };
        notice(events, player, msg);
        return;
    }
    if is_constructing(entities, worker) {
        return;
    }
    if config::building_stats(building).is_none() {
        notice(events, player, "Unknown building");
        return;
    }

    let owned = world_query::completed_building_kinds(entities, player);
    if !rules::economy::build_requirement_met_for_faction(&faction_id, building, &owned) {
        notice(events, player, "Requirement not met");
        return;
    }

    if tile_x >= map.size || tile_y >= map.size {
        notice(events, player, "Cannot build there");
        return;
    }

    // Feedback only; construction repeats a stricter final-placement check at arrival.
    // Unit bodies can move away before the worker arrives, so only permanent blockers reject here.
    let can_resume_existing =
        resumable_site_for_build_intent(map, entities, player, building, tile_x, tile_y).is_some();
    if !can_resume_existing {
        match standability::building_site_status_for_build_intent(
            map, entities, building, tile_x, tile_y, worker,
        ) {
            standability::BuildSiteStatus::Clear | standability::BuildSiteStatus::BlockedByUnit => {
            }
            standability::BuildSiteStatus::BlockedByBuilding
            | standability::BuildSiteStatus::BlockedByResourceNode
            | standability::BuildSiteStatus::InvalidFootprint => {
                notice(events, player, "Cannot build there");
                return;
            }
        }
    }

    let built = coordinator.order_build(entities, worker, building, tile_x, tile_y);
    if !built {
        notice(events, player, "Cannot build there");
    } else if let Some(e) = entities.get_mut(worker) {
        e.clear_queued_orders();
    }
}

/// Queue a unit at a production building. Reserves cost + supply on enqueue.
fn order_train(
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    player: u32,
    building: u32,
    unit: EntityKind,
    events: &mut HashMap<u32, Vec<Event>>,
) {
    let faction_id = faction_id_for(
        players.iter().map(|p| (p.id, p.faction_id.as_str())),
        player,
    );
    let ok = matches!(entities.get(building), Some(b)
        if b.owner == player && b.is_building() && !b.under_construction()
        && rules::economy::trainable_units_for_faction(&faction_id, b.kind).contains(&unit));
    if !ok {
        notice(events, player, "Cannot train that here");
        return;
    }
    let owned_complete = world_query::completed_building_kinds(entities, player);
    if !rules::economy::train_requirement_met_for_faction(&faction_id, unit, &owned_complete) {
        notice(events, player, "Requirement not met");
        return;
    }
    let stats = match config::unit_stats(unit) {
        Some(s) => s,
        None => {
            notice(events, player, "Unknown unit");
            return;
        }
    };

    let ps = match players.iter_mut().find(|p| p.id == player) {
        Some(p) => p,
        None => return,
    };
    if upgrade::required_for_unit(unit).is_some_and(|upgrade| !ps.upgrades.contains(&upgrade)) {
        notice(events, player, "Upgrade required");
        return;
    }
    let cost = rules::economy::resource_cost(unit);
    let supply = rules::economy::supply_cost(unit);
    if !ps.can_afford(cost.steel, cost.oil) {
        notice(
            events,
            player,
            rules::economy::resource_shortage_notice_for_cost(ps.steel, ps.oil, cost),
        );
        return;
    }
    if ps
        .supply_used
        .checked_add(supply)
        .is_none_or(|used| used > ps.supply_cap)
    {
        notice(events, player, "Not enough supply");
        return;
    }
    if !ps.spend_cost(cost) {
        notice(
            events,
            player,
            rules::economy::resource_shortage_notice_for_cost(ps.steel, ps.oil, cost),
        );
        return;
    }
    if !ps.reserve_supply(supply) {
        ps.refund_cost(cost);
        notice(events, player, "Not enough supply");
        return;
    }

    let queued = entities.get_mut(building).is_some_and(|b| {
        b.push_production(ProdItem {
            unit,
            progress: 0,
            total: stats.build_ticks,
        })
    });
    if !queued {
        if let Some(ps) = players.iter_mut().find(|p| p.id == player) {
            ps.refund_cost(cost);
            ps.release_supply(supply);
        }
    }
}

/// Set a unit-producing building's rally point. Validates ownership and that the building is a
/// completed producer; sanitizes/clamps the point to the map. Invalid requests are ignored
/// silently (consistent with movement commands), so a hostile client cannot wedge the tick loop.
fn order_set_rally(
    entities: &mut EntityStore,
    faction_id: &str,
    player: u32,
    building: u32,
    rally: RallyIntent,
    queued: bool,
) {
    let ok = matches!(entities.get(building), Some(b)
        if b.owner == player && b.is_building() && !b.under_construction()
        && rules::economy::can_act_as_production_anchor_for_faction(faction_id, b.kind));
    if !ok {
        return;
    }
    if let Some(b) = entities.get_mut(building) {
        if queued {
            b.append_rally_stage(rally, MAX_RALLY_STAGES);
        } else {
            b.clear_rally_stages();
            b.set_rally_point(Some(rally));
        }
    }
}

/// Cancel the latest item in a building's production queue, refunding its cost + supply.
fn order_cancel(
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    player: u32,
    building: u32,
) {
    enum Cancelled {
        Unit(EntityKind),
        Upgrade(UpgradeKind),
    }

    let cancelled = {
        let b = match entities.get_mut(building) {
            Some(b) if b.owner == player && b.is_building() => b,
            _ => return,
        };
        b.set_repeat_production(None, false);
        if let Some(item) = b.pop_last_research() {
            Cancelled::Upgrade(item.upgrade)
        } else if let Some(item) = b.pop_last_production() {
            Cancelled::Unit(item.unit)
        } else {
            return;
        }
    };
    if let Some(ps) = players.iter_mut().find(|p| p.id == player) {
        match cancelled {
            Cancelled::Unit(unit) if config::unit_stats(unit).is_some() => {
                ps.refund_cost(rules::economy::resource_cost(unit));
                ps.release_supply(rules::economy::supply_cost(unit));
            }
            Cancelled::Upgrade(upgrade) => {
                let definition = upgrade::definition(upgrade);
                ps.refund_cost(rules::economy::ResourceCost::new(
                    definition.cost_steel,
                    definition.cost_oil,
                ));
            }
            _ => {}
        }
    }
}

/// Push a best-effort `Notice` event to a player.
pub(crate) fn notice(events: &mut HashMap<u32, Vec<Event>>, player: u32, msg: &str) {
    events.entry(player).or_default().push(Event::Notice {
        msg: msg.to_string(),
        x: None,
        y: None,
        severity: NoticeSeverity::Info,
    });
}

/// Push a positioned `Notice` event to a player, anchored at world coordinates `(x, y)`.
pub(crate) fn notice_positioned(
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    msg: &str,
    severity: crate::protocol::NoticeSeverity,
    x: f32,
    y: f32,
) {
    events.entry(player).or_default().push(Event::Notice {
        msg: msg.to_string(),
        x: Some(x),
        y: Some(y),
        severity,
    });
}

#[cfg(test)]
mod tests;
