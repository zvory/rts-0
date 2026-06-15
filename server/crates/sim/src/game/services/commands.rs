use std::collections::{HashMap, HashSet};

use crate::config;
use crate::game::ability::{self, AbilityKind, AbilityTargetMode};
use crate::game::ability_runtime::AbilityRuntime;
use crate::game::artillery::ArtilleryShellStore;
use crate::game::command::SimCommand;
use crate::game::entity::{
    BuildPhase, Entity, EntityKind, EntityStore, Order, OrderIntent, ProdItem, RallyIntent,
    ResearchItem, WeaponSetup, MAX_QUEUED_ORDERS,
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
use crate::game::services::order_planner as planner;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::standability;
use crate::game::services::world_query;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::game::upgrade::{self, UpgradeKind};
use crate::game::PlayerState;
use crate::protocol::{self, AttackReveal, Event, NoticeSeverity};
use crate::rules;

const BASE_COMMAND_SUPPLY_CAP: u32 = 24;
const COMMAND_CAR_SUPPLY_CAP_BONUS: u32 = 12;
/// Max submitted unit ids inspected per multi-unit command. Caps the per-id work a single command
/// can force, so a repeated/huge id list can't be used to stall the tick loop.
const MAX_UNITS_PER_COMMAND: usize = 256;
const MAX_RALLY_STAGES: usize = 4;

/// Drain + apply queued commands (validate ownership / cost / supply / tech / placement).
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_commands(
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
    pending: Vec<(u32, SimCommand)>,
    events: &mut HashMap<u32, Vec<Event>>,
    tick: u32,
) {
    let teams = TeamRelations::from_player_teams(players.iter().map(|p| (p.id, p.team_id)));
    for (player, cmd) in pending {
        let faction_id = faction_id_for(
            players.iter().map(|p| (p.id, p.faction_id.as_str())),
            player,
        );
        let enforce_command_budget = !player_is_ai(
            players
                .iter()
                .map(|candidate| (candidate.id, candidate.is_ai)),
            player,
        );
        match cmd {
            SimCommand::Move {
                units,
                x,
                y,
                queued,
            } => {
                let Some(units) =
                    validate_command_units(entities, events, player, units, enforce_command_budget)
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
                apply_planned_unit_order(
                    map,
                    entities,
                    players,
                    spatial,
                    coordinator,
                    &teams,
                    fog,
                    smokes,
                    ability_runtime,
                    mortar_shells,
                    artillery_shells,
                    events,
                    player,
                    &planner_facts(entities, player, &faction_id, &units, false, None),
                    &request,
                    tick,
                );
            }
            SimCommand::AttackMove {
                units,
                x,
                y,
                queued,
            } => {
                let Some(units) =
                    validate_command_units(entities, events, player, units, enforce_command_budget)
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
                apply_planned_unit_order(
                    map,
                    entities,
                    players,
                    spatial,
                    coordinator,
                    &teams,
                    fog,
                    smokes,
                    ability_runtime,
                    mortar_shells,
                    artillery_shells,
                    events,
                    player,
                    &planner_facts(entities, player, &faction_id, &units, false, None),
                    &request,
                    tick,
                );
            }
            SimCommand::Attack {
                units,
                target,
                queued,
            } => {
                let Some(units) =
                    validate_command_units(entities, events, player, units, enforce_command_budget)
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
                apply_planned_unit_order(
                    map,
                    entities,
                    players,
                    spatial,
                    coordinator,
                    &teams,
                    fog,
                    smokes,
                    ability_runtime,
                    mortar_shells,
                    artillery_shells,
                    events,
                    player,
                    &planner_facts(entities, player, &faction_id, &units, false, None),
                    &request,
                    tick,
                );
            }
            SimCommand::SetupAntiTankGuns {
                units,
                x,
                y,
                queued,
            } => {
                let Some(units) =
                    validate_command_units(entities, events, player, units, enforce_command_budget)
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
                let facts = planner_facts(entities, player, &faction_id, &units, false, None);
                apply_planned_unit_order(
                    map,
                    entities,
                    players,
                    spatial,
                    coordinator,
                    &teams,
                    fog,
                    smokes,
                    ability_runtime,
                    mortar_shells,
                    artillery_shells,
                    events,
                    player,
                    &facts,
                    &request,
                    tick,
                );
            }
            SimCommand::TearDownAntiTankGuns { units } => {
                let Some(units) =
                    validate_command_units(entities, events, player, units, enforce_command_budget)
                else {
                    continue;
                };
                for id in units {
                    if !owns_unit(entities, player, id) || is_constructing(entities, id) {
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
                    validate_command_units(entities, events, player, units, enforce_command_budget)
                else {
                    continue;
                };
                use_ability(
                    map,
                    entities,
                    players,
                    spatial,
                    coordinator,
                    fog,
                    smokes,
                    ability_runtime,
                    mortar_shells,
                    artillery_shells,
                    events,
                    player,
                    AbilityUse {
                        ability,
                        units,
                        x,
                        y,
                        queued,
                    },
                    tick,
                );
            }
            SimCommand::RecastAbility {
                ability,
                units,
                target_object_id,
                queued: _,
            } => {
                let Some(units) =
                    validate_command_units(entities, events, player, units, enforce_command_budget)
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
                    validate_command_units(entities, events, player, units, enforce_command_budget)
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
                    validate_command_units(entities, events, player, units, enforce_command_budget)
                else {
                    continue;
                };
                let node_valid = gather_node_valid(entities, player, node);
                let request = planner::OrderRequest {
                    units: units.clone(),
                    mode: issue_mode(queued),
                    order: planner::RequestedOrder::Gather { node, node_valid },
                };
                apply_planned_unit_order(
                    map,
                    entities,
                    players,
                    spatial,
                    coordinator,
                    &teams,
                    fog,
                    smokes,
                    ability_runtime,
                    mortar_shells,
                    artillery_shells,
                    events,
                    player,
                    &planner_facts(entities, player, &faction_id, &units, false, None),
                    &request,
                    tick,
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
                    validate_command_units(entities, events, player, units, enforce_command_budget)
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
                apply_planned_unit_order(
                    map,
                    entities,
                    players,
                    spatial,
                    coordinator,
                    &teams,
                    fog,
                    smokes,
                    ability_runtime,
                    mortar_shells,
                    artillery_shells,
                    events,
                    player,
                    &planner_facts(entities, player, &faction_id, &units, false, None),
                    &request,
                    tick,
                );
            }
            SimCommand::Train { building, unit } => {
                order_train(entities, players, player, building, unit, events);
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
                    validate_command_units(entities, events, player, units, enforce_command_budget)
                else {
                    continue;
                };
                for id in units {
                    if owns_unit(entities, player, id) && !is_constructing(entities, id) {
                        entities.release_miner(id);
                        if let Some(e) = entities.get_mut(id) {
                            e.clear_orders();
                            e.clear_worker_carry();
                        }
                    }
                }
            }
            SimCommand::SetRally {
                building,
                x,
                y,
                kind,
                queued,
            } => {
                if !x.is_finite() || !y.is_finite() {
                    continue;
                }
                let max = (map.world_size_px() - 1.0).max(0.0);
                let rally = RallyIntent::new(kind, x.clamp(0.0, max), y.clamp(0.0, max));
                order_set_rally(entities, &faction_id, player, building, rally, queued);
            }
            SimCommand::Rejected { reason } => {
                notice(events, player, reason.notice_message());
            }
        }
    }
}

/// Dedupe a command's unit ids (preserving first-seen order) and cap inspected ids at
/// `MAX_UNITS_PER_COMMAND`.
fn dedupe_cap_units(units: Vec<u32>) -> Vec<u32> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(units.len().min(MAX_UNITS_PER_COMMAND));
    for id in units.into_iter().take(MAX_UNITS_PER_COMMAND) {
        if seen.insert(id) {
            out.push(id);
        }
    }
    out
}

fn player_is_ai(mut players: impl Iterator<Item = (u32, bool)>, player: u32) -> bool {
    players
        .find(|(candidate, _)| *candidate == player)
        .is_some_and(|(_, is_ai)| is_ai)
}

fn validate_command_units(
    entities: &EntityStore,
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    units: Vec<u32>,
    enforce_budget: bool,
) -> Option<Vec<u32>> {
    let units = dedupe_cap_units(units);
    if enforce_budget && command_budget_exceeded(entities, player, &units) {
        notice(events, player, "Command supply exceeded");
        return None;
    }
    Some(units)
}

fn command_budget_exceeded(entities: &EntityStore, player: u32, units: &[u32]) -> bool {
    let mut used = 0u32;
    let mut cap = BASE_COMMAND_SUPPLY_CAP;
    for id in units {
        let Some(entity) = entities.get(*id) else {
            continue;
        };
        if entity.owner != player || !entity.is_unit() || entity.under_construction() {
            continue;
        }
        used = used.saturating_add(command_weight(entity.kind));
        if entity.kind == EntityKind::CommandCar {
            cap = cap.saturating_add(COMMAND_CAR_SUPPLY_CAP_BONUS);
        }
    }
    used > cap
}

fn command_weight(kind: EntityKind) -> u32 {
    rules::economy::supply_cost(kind).max(1)
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

fn planner_config() -> planner::PlannerConfig {
    planner::PlannerConfig {
        max_units_per_command: MAX_UNITS_PER_COMMAND,
        max_queue_len: MAX_QUEUED_ORDERS,
    }
}

fn planner_facts(
    entities: &EntityStore,
    player: u32,
    faction_id: &str,
    units: &[u32],
    build_notice_compat: bool,
    ability: Option<AbilityFactInput>,
) -> Vec<planner::UnitFacts> {
    dedupe_cap_units(units.to_vec())
        .into_iter()
        .filter_map(|id| {
            let e = entities.get(id)?;
            if !e.is_unit() || e.owner != player {
                return None;
            }
            let mut facts = planner::UnitFacts::new(id);
            facts.pos = planner::Point::new(e.pos_x, e.pos_y);
            facts.queue_len = e.queued_orders().len();
            facts.queue_terminal = e
                .queued_orders()
                .iter()
                .any(|intent| matches!(intent, OrderIntent::PointFire(_)));
            facts.active_build = matches!(e.order(), Order::Build(_));
            facts.activity = match e.order() {
                Order::Idle => planner::UnitActivity::Idle,
                Order::Move(_) | Order::AttackMove(_) | Order::Ability(_) => {
                    planner::UnitActivity::Moving
                }
                _ => planner::UnitActivity::Busy,
            };
            facts.can_attack = e.can_attack();
            facts.can_gather = rules::economy::can_gather_for_faction(faction_id, e.kind);
            facts.can_build = rules::faction::catalog_for(faction_id)
                .is_some_and(|catalog| catalog.builders.contains(&e.kind))
                || build_notice_compat;
            facts.can_setup_anti_tank_gun =
                matches!(e.kind, EntityKind::AntiTankGun | EntityKind::Artillery);
            if let Some(ability) = ability {
                if ability_orders::caster_can_accept_order(entities, player, id, ability.kind)
                    && ability_orders::caster_allowed_by_faction(
                        entities,
                        faction_id,
                        id,
                        ability.kind,
                    )
                    && ability.tech_ready
                {
                    facts.abilities.push(planner::AbilityFacts {
                        ability: ability.id,
                        ready_at_issue: true,
                        can_execute_without_interrupt: ability.target.is_some_and(|(x, y)| {
                            world_ability_can_execute_without_interrupt(ability.kind)
                                && ability_orders::caster_in_range(
                                    ability.map,
                                    entities,
                                    id,
                                    ability.kind,
                                    x,
                                    y,
                                )
                                && ability_orders::world_ability_current_facing_ready(
                                    entities,
                                    id,
                                    ability.kind,
                                    x,
                                    y,
                                )
                        }),
                        can_interrupt_active_order: world_ability_may_interrupt_active_order(
                            ability.kind,
                        ),
                    });
                }
            }
            Some(facts)
        })
        .collect()
}

#[derive(Clone, Copy)]
struct AbilityFactInput<'a> {
    kind: AbilityKind,
    id: planner::AbilityId,
    tech_ready: bool,
    target: Option<(f32, f32)>,
    map: &'a Map,
}

fn world_ability_can_execute_without_interrupt(ability: AbilityKind) -> bool {
    matches!(
        ability,
        AbilityKind::Smoke | AbilityKind::EkatTeleport | AbilityKind::EkatLineShot
    )
}

fn world_ability_may_interrupt_active_order(ability: AbilityKind) -> bool {
    matches!(
        ability,
        AbilityKind::MortarFire | AbilityKind::EkatTeleport | AbilityKind::EkatLineShot
    )
}

#[allow(clippy::too_many_arguments)]
fn apply_planned_unit_order(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    teams: &TeamRelations,
    fog: &Fog,
    smokes: &mut SmokeCloudStore,
    ability_runtime: &mut AbilityRuntime,
    mortar_shells: &mut MortarShellStore,
    artillery_shells: &mut ArtilleryShellStore,
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    facts: &[planner::UnitFacts],
    request: &planner::OrderRequest,
    tick: u32,
) {
    let output = planner::plan_order(planner_config(), facts, request);
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
                planner::OrderIntent::AttackTarget(target) => {
                    if immediate_unit_can_replace(entities, player, unit)
                        && attack_unit_can_target(
                            entities, teams, fog, smokes, player, unit, target,
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
                planner::OrderIntent::SetupAntiTankGuns { face_toward } => {
                    if immediate_unit_can_replace(entities, player, unit) {
                        execute_anti_tank_gun_setup(entities, unit, face_toward.x, face_toward.y);
                    }
                }
                planner::OrderIntent::WorldAbility { ability, target } => {
                    let Some(ability) = ability_from_planner(ability) else {
                        continue;
                    };
                    if ability == AbilityKind::PointFire {
                        order_artillery_point_fire(
                            map,
                            entities,
                            players,
                            teams,
                            fog,
                            artillery_shells,
                            events,
                            player,
                            unit,
                            target.x,
                            target.y,
                            tick,
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
                    let faction_id = faction_id_for(
                        players.iter().map(|p| (p.id, p.faction_id.as_str())),
                        player,
                    );
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
                        let faction_id = faction_id_for(
                            players.iter().map(|p| (p.id, p.faction_id.as_str())),
                            player,
                        );
                        launch_self_ability(entities, &faction_id, player, unit, ability);
                    }
                }
            },
            planner::PlannedAction::AppendQueued { unit, intent } => {
                if let planner::OrderIntent::WorldAbility { ability, target } = intent {
                    if ability_from_planner(ability) == Some(AbilityKind::PointFire) {
                        if artillery_point_fire_command_target(
                            map, entities, player, unit, target.x, target.y,
                        )
                        .is_some()
                        {
                            if let Some(e) = entities.get_mut(unit) {
                                e.append_queued_order(OrderIntent::point_fire(target.x, target.y));
                            }
                        }
                        continue;
                    }
                }
                if let Some(intent) = entity_order_intent_from_planner(intent) {
                    if matches!(intent, OrderIntent::Attack(_))
                        && !matches!(
                            intent,
                            OrderIntent::Attack(attack)
                                if attack_unit_can_target(
                                    entities,
                                    teams,
                                    fog,
                                    smokes,
                                    player,
                                    unit,
                                    attack.target
                                )
                        )
                    {
                        continue;
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
                        let faction_id = faction_id_for(
                            players.iter().map(|p| (p.id, p.faction_id.as_str())),
                            player,
                        );
                        launch_self_ability(entities, &faction_id, player, unit, ability);
                    }
                    planner::AbilityTarget::WorldPoint(point) => {
                        if ability == AbilityKind::PointFire {
                            order_artillery_point_fire(
                                map,
                                entities,
                                players,
                                teams,
                                fog,
                                artillery_shells,
                                events,
                                player,
                                unit,
                                point.x,
                                point.y,
                                tick,
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
                            &faction_id_for(
                                players.iter().map(|p| (p.id, p.faction_id.as_str())),
                                player,
                            ),
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

    if let Some(goal) = move_goal {
        clear_queued_orders(entities, &move_units);
        clear_staged_anti_tank_gun_setup(entities, &move_units);
        coordinator.order_group_move(entities, player, &move_units, goal, false);
        begin_artillery_teardown_for_movement(entities, &move_units);
    }
    if let Some(goal) = attack_move_goal {
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

fn immediate_unit_can_replace(entities: &EntityStore, player: u32, unit: u32) -> bool {
    owns_unit(entities, player, unit) && !is_constructing(entities, unit)
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
    dedupe_cap_units(units.to_vec())
        .into_iter()
        .any(|unit| attack_unit_can_target(entities, teams, fog, smokes, player, unit, target))
}

fn attack_unit_can_target(
    entities: &EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    player: u32,
    unit: u32,
    target: u32,
) -> bool {
    matches!(entities.get(target),
        Some(t) if world_query::is_enemy_targetable(t, teams, player, unit)
            && fog.is_visible_world(player, t.pos_x, t.pos_y)
            && !smokes.point_inside(t.pos_x, t.pos_y))
}

fn gather_node_valid(entities: &EntityStore, player: u32, node: u32) -> bool {
    matches!(entities.get(node), Some(n) if n.is_node() && n.remaining().unwrap_or(0) > 0)
        && world_query::resource_has_completed_mining_cc(entities, player, node)
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
        && !matches!(entities.node_slot_holder(node), Some(holder) if holder != unit)
}

fn entity_order_intent_from_planner(intent: planner::OrderIntent) -> Option<OrderIntent> {
    match intent {
        planner::OrderIntent::Move(point) => Some(OrderIntent::move_to(point.x, point.y)),
        planner::OrderIntent::AttackMove(point) => {
            Some(OrderIntent::attack_move_to(point.x, point.y))
        }
        planner::OrderIntent::AttackTarget(target) => Some(OrderIntent::attack(target)),
        planner::OrderIntent::Gather(node) => Some(OrderIntent::gather(node)),
        planner::OrderIntent::Build {
            kind,
            tile_x,
            tile_y,
        } => {
            build_kind_from_code(kind).map(|building| OrderIntent::build(building, tile_x, tile_y))
        }
        planner::OrderIntent::WorldAbility { ability, target } => ability_from_planner(ability)
            .map(|ability| OrderIntent::ability(ability, target.x, target.y)),
        planner::OrderIntent::SelfAbility { ability } => {
            ability_from_planner(ability).map(OrderIntent::self_ability)
        }
        planner::OrderIntent::SetupAntiTankGuns { face_toward } => Some(
            OrderIntent::setup_anti_tank_guns(face_toward.x, face_toward.y),
        ),
    }
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
    match ability {
        AbilityKind::Charge => planner::AbilityId(0),
        AbilityKind::Smoke => planner::AbilityId(1),
        AbilityKind::MortarFire => planner::AbilityId(2),
        AbilityKind::PointFire => planner::AbilityId(3),
        AbilityKind::Breakthrough => planner::AbilityId(4),
        AbilityKind::EkatTeleport => planner::AbilityId(5),
        AbilityKind::EkatLineShot => planner::AbilityId(6),
    }
}

fn ability_from_planner(ability: planner::AbilityId) -> Option<AbilityKind> {
    match ability.0 {
        0 => Some(AbilityKind::Charge),
        1 => Some(AbilityKind::Smoke),
        2 => Some(AbilityKind::MortarFire),
        3 => Some(AbilityKind::PointFire),
        4 => Some(AbilityKind::Breakthrough),
        5 => Some(AbilityKind::EkatTeleport),
        6 => Some(AbilityKind::EkatLineShot),
        _ => None,
    }
}

struct AbilityUse {
    ability: AbilityKind,
    x: Option<f32>,
    y: Option<f32>,
    units: Vec<u32>,
    queued: bool,
}

#[allow(clippy::too_many_arguments)]
fn use_ability(
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
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    request: AbilityUse,
    tick: u32,
) {
    let ability = request.ability;
    let definition = ability::definition(ability);
    if request.queued && !definition.may_queue {
        return;
    }
    if ability == AbilityKind::PointFire {
        let Some(x) = request.x else {
            return;
        };
        let Some(y) = request.y else {
            return;
        };
        let Some((x, y)) = SmokeCloudStore::clamp_point_to_map(map, x, y) else {
            return;
        };
        let faction_id = faction_id_for(
            players.iter().map(|p| (p.id, p.faction_id.as_str())),
            player,
        );
        let teams = TeamRelations::from_player_teams(players.iter().map(|p| (p.id, p.team_id)));
        for unit in dedupe_cap_units(request.units) {
            if !ability_orders::caster_allowed_by_faction(entities, &faction_id, unit, ability) {
                continue;
            }
            if request.queued {
                if artillery_point_fire_command_target(map, entities, player, unit, x, y).is_some()
                {
                    if let Some(e) = entities.get_mut(unit) {
                        if !e
                            .queued_orders()
                            .iter()
                            .any(|intent| matches!(intent, OrderIntent::PointFire(_)))
                        {
                            e.append_queued_order(OrderIntent::point_fire(x, y));
                        }
                    }
                }
            } else {
                order_artillery_point_fire(
                    map,
                    entities,
                    players,
                    &teams,
                    fog,
                    artillery_shells,
                    events,
                    player,
                    unit,
                    x,
                    y,
                    tick,
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
            let eligible: Vec<u32> = dedupe_cap_units(request.units.clone())
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
        &faction_id_for(
            players.iter().map(|p| (p.id, p.faction_id.as_str())),
            player,
        ),
        &units,
        false,
        Some(AbilityFactInput {
            kind: ability,
            id: planner_id,
            tech_ready,
            target: target_point,
            map,
        }),
    );
    let order = planner::OrderRequest {
        units,
        mode: issue_mode(request.queued),
        order: planner::RequestedOrder::UseAbility {
            ability: planner_id,
            target,
        },
    };
    let teams = TeamRelations::from_player_teams(players.iter().map(|p| (p.id, p.team_id)));
    apply_planned_unit_order(
        map,
        entities,
        players,
        spatial,
        coordinator,
        &teams,
        fog,
        smokes,
        ability_runtime,
        mortar_shells,
        artillery_shells,
        events,
        player,
        &facts,
        &order,
        tick,
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
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    unit: u32,
    x: f32,
    y: f32,
    tick: u32,
) -> bool {
    let Some(target) = artillery_point_fire_command_target(map, entities, player, unit, x, y)
    else {
        return false;
    };
    entities.release_miner(unit);
    let Some(e) = entities.get_mut(unit) else {
        return false;
    };
    e.clear_orders();
    e.set_path_goal(None);
    e.set_target_id(None);
    e.reset_gather_state();
    let (px, py) = (e.pos_x, e.pos_y);
    e.reset_stuck(px, py);
    if !target.inside_field_of_fire {
        e.set_pending_redeploy_facing(Some(target.facing));
        e.set_weapon_setup(WeaponSetup::TearingDownToRedeploy {
            ticks: setup_ticks_for(e.kind),
        });
        e.set_order(Order::artillery_point_fire(target.x, target.y));
        return true;
    }
    e.set_desired_weapon_facing(target.facing);
    e.set_order(Order::artillery_point_fire(target.x, target.y));
    try_fire_artillery(
        entities,
        players,
        teams,
        fog,
        artillery_shells,
        events,
        player,
        unit,
        target.x,
        target.y,
        tick,
    )
}

#[derive(Clone, Copy)]
struct ArtilleryPointFireTarget {
    x: f32,
    y: f32,
    facing: f32,
    inside_field_of_fire: bool,
}

fn artillery_point_fire_command_target(
    map: &Map,
    entities: &EntityStore,
    player: u32,
    unit: u32,
    x: f32,
    y: f32,
) -> Option<ArtilleryPointFireTarget> {
    let target = artillery_point_fire_target(map, entities, player, unit, x, y)?;
    let e = entities.get(unit)?;
    artillery_can_accept_point_fire_command(e).then_some(target)
}

fn artillery_point_fire_target_valid(
    map: &Map,
    entities: &EntityStore,
    player: u32,
    unit: u32,
    x: f32,
    y: f32,
) -> bool {
    artillery_point_fire_command_target(map, entities, player, unit, x, y)
        .is_some_and(|target| target.inside_field_of_fire)
}

fn artillery_point_fire_target(
    map: &Map,
    entities: &EntityStore,
    player: u32,
    unit: u32,
    x: f32,
    y: f32,
) -> Option<ArtilleryPointFireTarget> {
    let (x, y) = SmokeCloudStore::clamp_point_to_map(map, x, y)?;
    let e = entities.get(unit)?;
    if e.owner != player
        || e.kind != EntityKind::Artillery
        || e.hp == 0
        || e.under_construction()
        || !e.path_is_empty()
    {
        return None;
    }
    let dx = x - e.pos_x;
    let dy = y - e.pos_y;
    let distance2 = dx * dx + dy * dy;
    if !distance2.is_finite() {
        return None;
    }
    let min_px = config::ARTILLERY_MIN_RANGE_TILES as f32 * config::TILE_SIZE as f32;
    let max_px = config::ARTILLERY_MAX_RANGE_TILES as f32 * config::TILE_SIZE as f32;
    if distance2 < min_px * min_px || distance2 > max_px * max_px {
        return None;
    }
    let center = artillery_point_fire_field_center(e).filter(|facing| facing.is_finite())?;
    let facing = dy.atan2(dx);
    if !facing.is_finite() {
        return None;
    }
    let inside_field_of_fire =
        angle_delta(center, facing).abs() <= config::ARTILLERY_FIELD_OF_FIRE_RAD * 0.5;
    Some(ArtilleryPointFireTarget {
        x,
        y,
        facing,
        inside_field_of_fire,
    })
}

fn artillery_can_accept_point_fire_command(e: &Entity) -> bool {
    matches!(e.weapon_setup(), WeaponSetup::Deployed)
        || (matches!(e.order(), Order::ArtilleryPointFire(_))
            && matches!(
                e.weapon_setup(),
                WeaponSetup::TearingDownToRedeploy { .. }
                    | WeaponSetup::Packed
                    | WeaponSetup::SettingUp { .. }
            ))
}

fn artillery_point_fire_field_center(e: &Entity) -> Option<f32> {
    match e.weapon_setup() {
        WeaponSetup::TearingDownToRedeploy { .. } => e.pending_redeploy_facing(),
        WeaponSetup::Packed | WeaponSetup::SettingUp { .. } => e.emplacement_facing(),
        _ => e.emplacement_facing().or_else(|| e.weapon_facing()),
    }
}

#[allow(clippy::too_many_arguments)]
fn try_fire_artillery(
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    teams: &TeamRelations,
    fog: &Fog,
    artillery_shells: &mut ArtilleryShellStore,
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    unit: u32,
    x: f32,
    y: f32,
    tick: u32,
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
    if !ability_orders::caster_allowed_by_faction(
        entities,
        &faction_id,
        unit,
        AbilityKind::PointFire,
    ) {
        return false;
    }
    let ammo_cost = ability::definition(AbilityKind::PointFire).cost;
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
    let (target_x, target_y) = {
        let Some(e) = entities.get_mut(unit) else {
            ps.refund_cost(ammo_cost);
            return false;
        };
        let shot_number = e.increment_artillery_shots_fired();
        e.set_attack_cd(config::ARTILLERY_RELOAD_TICKS);
        artillery_scattered_point(unit, tick, x, y, shot_number)
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
            });
        }
    }
    true
}

fn artillery_scattered_point(unit: u32, tick: u32, x: f32, y: f32, shot_number: u16) -> (f32, f32) {
    let max_step = config::ARTILLERY_ACCURACY_SHOTS_TO_MIN
        .saturating_sub(1)
        .max(1) as f32;
    let progress = (shot_number.saturating_sub(1) as f32 / max_step).clamp(0.0, 1.0);
    let error_tiles = config::ARTILLERY_INITIAL_ERROR_TILES
        + (config::ARTILLERY_MIN_ERROR_TILES - config::ARTILLERY_INITIAL_ERROR_TILES) * progress;
    let radius_px = error_tiles.max(0.0) * config::TILE_SIZE as f32;
    if radius_px <= f32::EPSILON {
        return (x, y);
    }
    let seed = unit
        .wrapping_mul(1_103_515_245)
        .wrapping_add(tick)
        .wrapping_add((shot_number as u32).wrapping_mul(97_531));
    let angle = (seed as f32 * 1.618_034).rem_euclid(std::f32::consts::TAU);
    let radial = (((seed.rotate_left(13) >> 8) & 1023) as f32 / 1023.0).sqrt() * radius_px;
    (x + angle.cos() * radial, y + angle.sin() * radial)
}

pub(crate) fn artillery_point_fire_system(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    artillery_shells: &mut ArtilleryShellStore,
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    tick: u32,
) {
    let teams = TeamRelations::from_player_teams(players.iter().map(|p| (p.id, p.team_id)));
    let orders: Vec<(u32, u32, f32, f32)> = entities
        .ids()
        .into_iter()
        .filter_map(|id| {
            let e = entities.get(id)?;
            let Order::ArtilleryPointFire(order) = e.order() else {
                return None;
            };
            Some((id, e.owner, order.intent.x, order.intent.y))
        })
        .collect();
    for (id, owner, x, y) in orders {
        if !artillery_point_fire_target_valid(map, entities, owner, id, x, y) {
            if matches!(
                entities.get(id).map(|e| e.weapon_setup()),
                Some(
                    WeaponSetup::Packed
                        | WeaponSetup::SettingUp { .. }
                        | WeaponSetup::TearingDown { .. }
                        | WeaponSetup::TearingDownToRedeploy { .. }
                )
            ) && artillery_point_fire_target(map, entities, owner, id, x, y).is_some()
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
            events,
            owner,
            id,
            x,
            y,
            tick,
        );
    }
}

fn execute_anti_tank_gun_setup(entities: &mut EntityStore, id: u32, x: f32, y: f32) -> bool {
    let Some(e) = entities.get(id) else {
        return false;
    };
    if !matches!(e.kind, EntityKind::AntiTankGun | EntityKind::Artillery)
        || e.under_construction()
        || !x.is_finite()
        || !y.is_finite()
    {
        return false;
    }
    let facing = (y - e.pos_y).atan2(x - e.pos_x);
    if !facing.is_finite() {
        return false;
    }
    entities.release_miner(id);
    let Some(e) = entities.get_mut(id) else {
        return false;
    };
    e.clear_orders();
    e.set_path_goal(None);
    if matches!(e.weapon_setup(), WeaponSetup::Packed) {
        e.set_emplacement_facing(Some(facing));
        e.set_desired_weapon_facing(facing);
    } else {
        e.set_pending_redeploy_facing(Some(facing));
        e.set_weapon_setup(WeaponSetup::TearingDownToRedeploy {
            ticks: setup_ticks_for(e.kind),
        });
    }
    e.reset_gather_state();
    let (px, py) = (e.pos_x, e.pos_y);
    e.reset_stuck(px, py);
    true
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

/// Whether `player` owns a *unit* with this id. Local re-export of
/// [`world_query::owns_unit`] to keep call sites in this module terse.
fn owns_unit(entities: &EntityStore, player: u32, id: u32) -> bool {
    world_query::owns_unit(entities, player, id)
}

fn clear_queued_orders(entities: &mut EntityStore, ids: &[u32]) {
    for id in ids {
        if let Some(e) = entities.get_mut(*id) {
            e.clear_queued_orders();
        }
    }
}

/// True if this unit is a worker that has already begun laying concrete — it cannot
/// be pulled away until the building finishes or is destroyed.
fn is_constructing(entities: &EntityStore, id: u32) -> bool {
    matches!(
        entities.get(id),
        Some(e) if matches!(e.build_phase(), Some(BuildPhase::Constructing { .. }))
    )
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

fn begin_artillery_teardown_for_movement(entities: &mut EntityStore, ids: &[u32]) {
    for id in ids {
        let Some(e) = entities.get_mut(*id) else {
            continue;
        };
        if e.kind != EntityKind::Artillery {
            continue;
        }
        e.reset_artillery_accuracy();
        if !matches!(e.weapon_setup(), WeaponSetup::Packed) {
            e.set_weapon_setup(WeaponSetup::TearingDown {
                ticks: config::ARTILLERY_SETUP_TICKS,
            });
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
        notice(events, player, "Only workers can build");
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
    let can_resume_existing =
        resumable_site_for_build_intent(map, entities, player, building, tile_x, tile_y).is_some();
    if !can_resume_existing
        && !standability::building_site_clear_for_build_intent(
            map, entities, building, tile_x, tile_y, worker,
        )
    {
        notice(events, player, "Cannot build there");
        return;
    }

    let ps = match players.iter().find(|p| p.id == player) {
        Some(p) => p,
        None => return,
    };
    let cost = rules::economy::resource_cost(building);
    if !can_resume_existing && !ps.can_afford(cost.steel, cost.oil) {
        notice(
            events,
            player,
            rules::economy::resource_shortage_notice_for_cost(ps.steel, ps.oil, cost),
        );
        return;
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
            Some(b)
                if b.owner == player
                    && b.is_building()
                    && (!b.prod_queue().is_empty() || !b.research_queue().is_empty()) =>
            {
                b
            }
            _ => return,
        };
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
mod tests {
    use super::*;
    use crate::game::entity::{
        EntityKind, EntityStore, Order, OrderIntent, RallyKind, WeaponSetup,
    };
    use crate::game::map::Map;
    use crate::game::services::move_coordinator::MoveCoordinator;
    use crate::game::services::occupancy::{footprint_center, footprint_tiles, Occupancy};
    use crate::game::services::pathing::PathingService;
    use crate::game::services::spatial::SpatialIndex;
    use crate::game::ScoreState;
    use crate::protocol::terrain;

    #[test]
    fn build_order_can_start_when_worker_inside_intent_but_stages_outside() {
        let map = flat_map(16);
        let mut entities = EntityStore::new();
        let (wx, wy) = footprint_center(&map, EntityKind::Depot, 4, 4);
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, wx, wy)
            .expect("worker should spawn");
        let spatial = SpatialIndex::build(&entities, map.size);
        let occ = Occupancy::build(&map, &entities);
        let mut pathing = PathingService::new(1024, 32);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
        let mut players = vec![player_state(1)];
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1], &entities, &map);
        let mut smokes = SmokeCloudStore::new();
        let mut ability_runtime = AbilityRuntime::new();
        let mut mortar_shells = MortarShellStore::default();
        let mut artillery_shells = ArtilleryShellStore::default();
        let mut events: HashMap<u32, Vec<Event>> = players
            .iter()
            .map(|player| (player.id, Vec::new()))
            .collect();

        apply_commands(
            &map,
            &mut entities,
            &mut players,
            &spatial,
            &mut coordinator,
            &fog,
            &mut smokes,
            &mut ability_runtime,
            &mut mortar_shells,
            &mut artillery_shells,
            vec![(
                1,
                SimCommand::Build {
                    units: vec![worker],
                    building: EntityKind::Depot,
                    tile_x: 4,
                    tile_y: 4,
                    queued: false,
                },
            )],
            &mut events,
            1,
        );

        let worker = entities.get(worker).expect("worker should remain alive");
        assert!(
            matches!(worker.order(), Order::Build(_)),
            "worker should keep the accepted build order"
        );
        let goal = worker
            .path_goal()
            .expect("build order should set a staging goal");
        let goal_tile = map.tile_of(goal.0, goal.1);
        assert!(
            !footprint_tiles(EntityKind::Depot, 4, 4).contains(&goal_tile),
            "build-over-self order should stage outside the requested footprint"
        );
        assert!(
            events.get(&1).is_none_or(Vec::is_empty),
            "valid build-over-self intent should not emit a failure notice"
        );
    }

    #[test]
    fn build_order_does_not_pull_worker_off_active_construction() {
        let map = flat_map(16);
        let mut entities = EntityStore::new();
        let (site_x, site_y) = footprint_center(&map, EntityKind::Depot, 4, 4);
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, site_x, site_y)
            .expect("worker should spawn");
        let site = entities
            .spawn_building(1, EntityKind::Depot, site_x, site_y, false)
            .expect("scaffold should spawn");
        let worker_entity = entities.get_mut(worker).expect("worker should exist");
        worker_entity.set_order(Order::build(EntityKind::Depot, 4, 4));
        worker_entity.mark_build_phase(BuildPhase::Constructing { site });
        worker_entity.set_target_id(Some(site));

        let spatial = SpatialIndex::build(&entities, map.size);
        let occ = Occupancy::build(&map, &entities);
        let mut pathing = PathingService::new(1024, 32);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
        let mut players = vec![player_state(1)];
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1], &entities, &map);
        let mut smokes = SmokeCloudStore::new();
        let mut ability_runtime = AbilityRuntime::new();
        let mut mortar_shells = MortarShellStore::default();
        let mut artillery_shells = ArtilleryShellStore::default();
        let mut events: HashMap<u32, Vec<Event>> = players
            .iter()
            .map(|player| (player.id, Vec::new()))
            .collect();

        apply_commands(
            &map,
            &mut entities,
            &mut players,
            &spatial,
            &mut coordinator,
            &fog,
            &mut smokes,
            &mut ability_runtime,
            &mut mortar_shells,
            &mut artillery_shells,
            vec![(
                1,
                SimCommand::Build {
                    units: vec![worker],
                    building: EntityKind::Barracks,
                    tile_x: 8,
                    tile_y: 8,
                    queued: false,
                },
            )],
            &mut events,
            1,
        );

        let worker = entities.get(worker).expect("worker should remain alive");
        assert_eq!(
            worker.build_phase(),
            Some(BuildPhase::Constructing { site }),
            "active build command should keep constructing the original scaffold"
        );
        assert_eq!(
            worker.order().build_intent_tile(),
            Some((EntityKind::Depot, 4, 4)),
            "second build order must not replace the active construction intent"
        );
        assert_eq!(
            worker.target_id(),
            Some(site),
            "worker should stay latched to the scaffold it is building"
        );
        assert!(
            events.get(&1).is_none_or(Vec::is_empty),
            "ignored build command should not emit a failure notice"
        );
    }

    #[test]
    fn build_order_accepts_resuming_owned_scaffold() {
        let map = flat_map(16);
        let mut entities = EntityStore::new();
        let (site_x, site_y) = footprint_center(&map, EntityKind::Depot, 4, 4);
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, 64.0, 64.0)
            .expect("worker should spawn");
        let scaffold = entities
            .spawn_building(1, EntityKind::Depot, site_x, site_y, false)
            .expect("scaffold should spawn");
        let spatial = SpatialIndex::build(&entities, map.size);
        let occ = Occupancy::build(&map, &entities);
        let mut pathing = PathingService::new(1024, 32);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
        let mut players = vec![player_state(1)];
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1], &entities, &map);
        let mut smokes = SmokeCloudStore::new();
        let mut ability_runtime = AbilityRuntime::new();
        let mut mortar_shells = MortarShellStore::default();
        let mut artillery_shells = ArtilleryShellStore::default();
        let mut events = HashMap::new();

        apply_commands(
            &map,
            &mut entities,
            &mut players,
            &spatial,
            &mut coordinator,
            &fog,
            &mut smokes,
            &mut ability_runtime,
            &mut mortar_shells,
            &mut artillery_shells,
            vec![(
                1,
                SimCommand::Build {
                    units: vec![worker],
                    building: EntityKind::Depot,
                    tile_x: 4,
                    tile_y: 4,
                    queued: false,
                },
            )],
            &mut events,
            1,
        );

        let worker = entities.get(worker).expect("worker should remain alive");
        assert!(
            matches!(worker.order(), Order::Build(_)),
            "worker should accept the resume order"
        );
        assert_eq!(
            worker.order().build_intent_tile(),
            Some((EntityKind::Depot, 4, 4)),
            "resume order should keep the scaffold footprint intent"
        );
        assert_ne!(
            worker.path_goal(),
            None,
            "resume order should still path the worker to the scaffold"
        );
        assert!(
            entities
                .get(scaffold)
                .expect("scaffold should survive")
                .under_construction(),
            "existing scaffold should remain available for resume"
        );
        assert!(
            events.get(&1).is_none_or(Vec::is_empty),
            "resume order should not emit a placement failure notice"
        );
    }

    #[test]
    fn build_order_accepts_resuming_owned_scaffold_without_resources() {
        let map = flat_map(16);
        let mut entities = EntityStore::new();
        let (site_x, site_y) = footprint_center(&map, EntityKind::Depot, 4, 4);
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, 64.0, 64.0)
            .expect("worker should spawn");
        entities
            .spawn_building(1, EntityKind::Depot, site_x, site_y, false)
            .expect("scaffold should spawn");
        let spatial = SpatialIndex::build(&entities, map.size);
        let occ = Occupancy::build(&map, &entities);
        let mut pathing = PathingService::new(1024, 32);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
        let mut players = vec![player_state(1)];
        players[0].steel = 0;
        players[0].oil = 0;
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1], &entities, &map);
        let mut smokes = SmokeCloudStore::new();
        let mut ability_runtime = AbilityRuntime::new();
        let mut mortar_shells = MortarShellStore::default();
        let mut artillery_shells = ArtilleryShellStore::default();
        let mut events = HashMap::new();

        apply_commands(
            &map,
            &mut entities,
            &mut players,
            &spatial,
            &mut coordinator,
            &fog,
            &mut smokes,
            &mut ability_runtime,
            &mut mortar_shells,
            &mut artillery_shells,
            vec![(
                1,
                SimCommand::Build {
                    units: vec![worker],
                    building: EntityKind::Depot,
                    tile_x: 4,
                    tile_y: 4,
                    queued: false,
                },
            )],
            &mut events,
            1,
        );

        let worker = entities.get(worker).expect("worker should remain alive");
        assert!(
            matches!(worker.order(), Order::Build(_)),
            "worker should accept resume orders even when the original cost is no longer affordable"
        );
        assert_eq!(
            worker.order().build_intent_tile(),
            Some((EntityKind::Depot, 4, 4))
        );
        assert_eq!(players[0].steel, 0, "resume order should not charge steel");
        assert_eq!(players[0].oil, 0, "resume order should not charge oil");
        assert!(
            events.get(&1).is_none_or(Vec::is_empty),
            "resume order should not emit a resource shortage notice"
        );
    }

    #[test]
    fn anti_tank_gun_and_tank_training_require_finished_unlock_upgrades() {
        let map = flat_map(24);
        for (producer, unit, upgrade, setup_extra) in [
            (
                EntityKind::Steelworks,
                EntityKind::AntiTankGun,
                UpgradeKind::AntiTankGunUnlock,
                None,
            ),
            (
                EntityKind::Factory,
                EntityKind::Tank,
                UpgradeKind::TankUnlock,
                None,
            ),
        ] {
            let mut entities = EntityStore::new();
            let (px, py) = footprint_center(&map, producer, 6, 6);
            let building = entities
                .spawn_building(1, producer, px, py, true)
                .expect("producer should spawn");
            if let Some(kind) = setup_extra {
                let (x, y) = footprint_center(&map, kind, 10, 6);
                entities
                    .spawn_building(1, kind, x, y, true)
                    .expect("tech building should spawn");
            }
            let mut players = vec![player_state(1), player_state(2)];
            let command = SimCommand::Train { building, unit };
            let events = apply_with_players(
                &map,
                &mut entities,
                &mut players,
                vec![(1, command.clone())],
            );
            assert!(
                entities
                    .get(building)
                    .expect("producer")
                    .prod_queue()
                    .is_empty(),
                "{unit:?} should not queue before {upgrade:?} finishes"
            );
            assert!(matches!(
                events.get(&1).and_then(|events| events.first()),
                Some(Event::Notice { msg, .. }) if msg == "Upgrade required"
            ));

            players[0].upgrades.insert(upgrade);
            apply_with_players(&map, &mut entities, &mut players, vec![(1, command)]);
            let queue = entities.get(building).expect("producer").prod_queue();
            assert_eq!(queue.len(), 1);
            assert_eq!(queue[0].unit, unit);
        }
    }

    #[test]
    fn advanced_unlocks_research_only_at_research_complex() {
        let map = flat_map(24);
        for (wrong_building_kind, upgrade) in [
            (EntityKind::Steelworks, UpgradeKind::AntiTankGunUnlock),
            (EntityKind::Steelworks, UpgradeKind::ArtilleryUnlock),
            (EntityKind::Factory, UpgradeKind::TankUnlock),
            (EntityKind::Steelworks, UpgradeKind::MortarAutocast),
        ] {
            let mut entities = EntityStore::new();
            let (wrong_x, wrong_y) = footprint_center(&map, wrong_building_kind, 4, 4);
            let wrong_building = entities
                .spawn_building(1, wrong_building_kind, wrong_x, wrong_y, true)
                .expect("wrong research building should spawn");
            let (rd_x, rd_y) = footprint_center(&map, EntityKind::ResearchComplex, 10, 4);
            let research_complex = entities
                .spawn_building(1, EntityKind::ResearchComplex, rd_x, rd_y, true)
                .expect("research complex should spawn");
            let mut players = vec![player_state(1), player_state(2)];
            if upgrade == UpgradeKind::ArtilleryUnlock {
                players[0].upgrades.insert(UpgradeKind::AntiTankGunUnlock);
            }

            let events = apply_with_players(
                &map,
                &mut entities,
                &mut players,
                vec![(
                    1,
                    SimCommand::Research {
                        building: wrong_building,
                        upgrade,
                    },
                )],
            );
            assert!(entities
                .get(wrong_building)
                .expect("wrong building")
                .research_queue()
                .is_empty());
            assert!(matches!(
                events.get(&1).and_then(|events| events.first()),
                Some(Event::Notice { msg, .. }) if msg == "Cannot research that here"
            ));

            apply_with_players(
                &map,
                &mut entities,
                &mut players,
                vec![(
                    1,
                    SimCommand::Research {
                        building: research_complex,
                        upgrade,
                    },
                )],
            );
            let queue = entities
                .get(research_complex)
                .expect("research complex")
                .research_queue();
            assert_eq!(queue.len(), 1);
            assert_eq!(queue[0].upgrade, upgrade);
        }
    }

    #[test]
    fn fixture_faction_rejects_global_build_train_and_research_commands() {
        let map = flat_map(24);
        let mut players = vec![player_state(1), player_state(2)];
        players[0].faction_id = rules::faction::EMPTY_FIXTURE_FACTION_ID.to_string();
        let mut entities = EntityStore::new();
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, 96.0, 96.0)
            .expect("worker should spawn");
        let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 5, 5);
        let city_centre = entities
            .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
            .expect("city centre should spawn");
        let (rd_x, rd_y) = footprint_center(&map, EntityKind::ResearchComplex, 10, 5);
        let research_complex = entities
            .spawn_building(1, EntityKind::ResearchComplex, rd_x, rd_y, true)
            .expect("research complex should spawn");
        let resources_before = (
            players[0].steel,
            players[0].oil,
            players[0].supply_used,
            players[0].supply_cap,
        );

        let events = apply_with_players(
            &map,
            &mut entities,
            &mut players,
            vec![
                (
                    1,
                    SimCommand::Build {
                        units: vec![worker],
                        building: EntityKind::Depot,
                        tile_x: 8,
                        tile_y: 8,
                        queued: false,
                    },
                ),
                (
                    1,
                    SimCommand::Train {
                        building: city_centre,
                        unit: EntityKind::Worker,
                    },
                ),
                (
                    1,
                    SimCommand::Research {
                        building: research_complex,
                        upgrade: UpgradeKind::TankUnlock,
                    },
                ),
            ],
        );

        assert_eq!(
            (
                players[0].steel,
                players[0].oil,
                players[0].supply_used,
                players[0].supply_cap,
            ),
            resources_before,
            "fixture-faction illegal build/train/research commands must not spend Steel/Oil or reserve Supply"
        );
        assert!(
            !matches!(
                entities.get(worker).expect("worker").order(),
                Order::Build(_)
            ),
            "fixture faction worker must not receive a current-faction build order"
        );
        assert!(
            entities
                .get(city_centre)
                .expect("city centre")
                .prod_queue()
                .is_empty(),
            "fixture faction must not train globally-defined current units"
        );
        assert!(
            entities
                .get(research_complex)
                .expect("research complex")
                .research_queue()
                .is_empty(),
            "fixture faction must not research globally-defined current upgrades"
        );
        let notices: Vec<_> = events
            .get(&1)
            .into_iter()
            .flatten()
            .filter_map(|event| match event {
                Event::Notice { msg, .. } => Some(msg.as_str()),
                _ => None,
            })
            .collect();
        assert!(notices.contains(&"Cannot train that here"));
        assert!(notices.contains(&"Cannot research that here"));
    }

    #[test]
    fn fixture_faction_point_fire_does_not_spend_steel() {
        let map = flat_map(64);
        let mut players = vec![player_state(1), player_state(2)];
        players[0].faction_id = rules::faction::EMPTY_FIXTURE_FACTION_ID.to_string();
        let mut entities = EntityStore::new();
        let artillery = entities
            .spawn_unit(1, EntityKind::Artillery, 320.0, 320.0)
            .expect("artillery should spawn");
        {
            let gun = entities.get_mut(artillery).expect("artillery should exist");
            gun.set_weapon_setup(WeaponSetup::Deployed);
            gun.set_emplacement_facing(Some(0.0));
        }
        let steel_before = players[0].steel;

        apply_with_players(
            &map,
            &mut entities,
            &mut players,
            vec![(
                1,
                SimCommand::UseAbility {
                    ability: AbilityKind::PointFire,
                    units: vec![artillery],
                    x: Some(960.0),
                    y: Some(320.0),
                    queued: false,
                },
            )],
        );

        assert_eq!(
            players[0].steel, steel_before,
            "out-of-faction artillery ability must not spend Steel"
        );
        assert_eq!(
            entities
                .get(artillery)
                .expect("artillery should exist")
                .attack_cd(),
            0,
            "out-of-faction artillery ability must not start the firing cooldown"
        );
    }

    #[test]
    fn set_mortar_autocast_requires_completed_research() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let mortar = entities
            .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
            .expect("mortar should spawn");
        let command = SimCommand::SetAutocast {
            ability: AbilityKind::MortarFire,
            units: vec![mortar],
            enabled: true,
        };
        let mut players = vec![player_state(1), player_state(2)];

        apply_with_players(
            &map,
            &mut entities,
            &mut players,
            vec![(1, command.clone())],
        );
        assert_eq!(
            entities
                .get(mortar)
                .expect("mortar should exist")
                .autocast_enabled(AbilityKind::MortarFire),
            Some(false),
            "pre-research autocast command should be ignored"
        );

        players[0].upgrades.insert(UpgradeKind::MortarAutocast);
        apply_with_players(&map, &mut entities, &mut players, vec![(1, command)]);
        assert_eq!(
            entities
                .get(mortar)
                .expect("mortar should exist")
                .autocast_enabled(AbilityKind::MortarFire),
            Some(true),
            "researched autocast command should be accepted"
        );
    }

    #[test]
    fn set_mortar_autocast_rejects_wrong_faction_carriers() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let mortar = entities
            .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
            .expect("mortar should spawn");
        let mut players = vec![player_state(1), player_state(2)];
        players[0].faction_id = rules::faction::EMPTY_FIXTURE_FACTION_ID.to_string();
        players[0].upgrades.insert(UpgradeKind::MortarAutocast);

        apply_with_players(
            &map,
            &mut entities,
            &mut players,
            vec![(
                1,
                SimCommand::SetAutocast {
                    ability: AbilityKind::MortarFire,
                    units: vec![mortar],
                    enabled: true,
                },
            )],
        );

        assert_eq!(
            entities
                .get(mortar)
                .expect("mortar should exist")
                .autocast_enabled(AbilityKind::MortarFire),
            Some(false),
            "out-of-faction autocast commands should not toggle carrier state"
        );
    }

    #[test]
    fn artillery_research_requires_anti_tank_gun_research() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let (rd_x, rd_y) = footprint_center(&map, EntityKind::ResearchComplex, 6, 6);
        let research_complex = entities
            .spawn_building(1, EntityKind::ResearchComplex, rd_x, rd_y, true)
            .expect("research complex should spawn");
        let mut players = vec![player_state(1), player_state(2)];
        let command = SimCommand::Research {
            building: research_complex,
            upgrade: UpgradeKind::ArtilleryUnlock,
        };

        let events = apply_with_players(
            &map,
            &mut entities,
            &mut players,
            vec![(1, command.clone())],
        );
        assert!(entities
            .get(research_complex)
            .expect("research complex")
            .research_queue()
            .is_empty());
        assert!(matches!(
            events.get(&1).and_then(|events| events.first()),
            Some(Event::Notice { msg, .. }) if msg == "Requirement not met"
        ));

        players[0].upgrades.insert(UpgradeKind::AntiTankGunUnlock);
        apply_with_players(&map, &mut entities, &mut players, vec![(1, command)]);
        let queue = entities
            .get(research_complex)
            .expect("research complex")
            .research_queue();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].upgrade, UpgradeKind::ArtilleryUnlock);
    }

    #[test]
    fn set_rally_stores_point_on_producer_and_rejects_others() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::Barracks, 6, 6);
        let barracks = entities
            .spawn_building(1, EntityKind::Barracks, bx, by, true)
            .expect("barracks should spawn");
        let (dx, dy) = footprint_center(&map, EntityKind::Depot, 12, 6);
        let depot = entities
            .spawn_building(1, EntityKind::Depot, dx, dy, true)
            .expect("depot should spawn");
        let (ex, ey) = footprint_center(&map, EntityKind::Barracks, 6, 12);
        let enemy_barracks = entities
            .spawn_building(2, EntityKind::Barracks, ex, ey, true)
            .expect("enemy barracks should spawn");

        apply(
            &map,
            &mut entities,
            vec![
                (
                    1,
                    SimCommand::SetRally {
                        building: barracks,
                        x: 100.0,
                        y: 200.0,
                        kind: RallyKind::Move,
                        queued: false,
                    },
                ),
                // Depot trains nothing -> rejected.
                (
                    1,
                    SimCommand::SetRally {
                        building: depot,
                        x: 50.0,
                        y: 50.0,
                        kind: RallyKind::Move,
                        queued: false,
                    },
                ),
                // Not the owner -> rejected.
                (
                    1,
                    SimCommand::SetRally {
                        building: enemy_barracks,
                        x: 10.0,
                        y: 10.0,
                        kind: RallyKind::Move,
                        queued: false,
                    },
                ),
            ],
        );

        assert_eq!(
            entities.get(barracks).unwrap().rally_point(),
            Some((100.0, 200.0)),
            "owned producer should store the rally point"
        );
        assert_eq!(
            entities.get(depot).unwrap().rally_point(),
            None,
            "non-producer building should not accept a rally point"
        );
        assert_eq!(
            entities.get(enemy_barracks).unwrap().rally_point(),
            None,
            "rally on an enemy building should be ignored"
        );
    }

    #[test]
    fn set_rally_clamps_out_of_bounds_point() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::Barracks, 6, 6);
        let barracks = entities
            .spawn_building(1, EntityKind::Barracks, bx, by, true)
            .expect("barracks should spawn");

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::SetRally {
                    building: barracks,
                    x: 1.0e9,
                    y: -50.0,
                    kind: RallyKind::Move,
                    queued: false,
                },
            )],
        );

        let max = map.world_size_px() - 1.0;
        assert_eq!(
            entities.get(barracks).unwrap().rally_point(),
            Some((max, 0.0)),
            "rally point should be clamped into the map bounds"
        );
    }

    #[test]
    fn queued_move_appends_until_cap_and_normal_move_clears_queue() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");

        let queued_moves = (0..10)
            .map(|i| {
                (
                    1,
                    SimCommand::Move {
                        units: vec![unit],
                        x: 120.0 + i as f32,
                        y: 140.0,
                        queued: true,
                    },
                )
            })
            .collect();
        apply(&map, &mut entities, queued_moves);

        let entity = entities.get(unit).expect("unit should exist");
        assert_eq!(
            entity.queued_orders().len(),
            8,
            "unit queue should enforce the phase-0 cap"
        );
        assert!(
            matches!(entity.order(), Order::Idle),
            "queued command should not interrupt the active order in phase 0"
        );

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Move {
                    units: vec![unit],
                    x: 200.0,
                    y: 220.0,
                    queued: false,
                },
            )],
        );

        let entity = entities.get(unit).expect("unit should exist");
        assert!(
            entity.queued_orders().is_empty(),
            "replacement move should clear queued intents"
        );
        assert!(
            matches!(entity.order(), Order::Move(_)),
            "replacement move should still issue the active order"
        );
    }

    #[test]
    fn planner_backed_existing_command_families_preserve_active_and_queued_state() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
            .expect("city centre should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, cc_x + 16.0, cc_y)
            .expect("worker should spawn");
        let rifle = entities
            .spawn_unit(1, EntityKind::Rifleman, cc_x + 48.0, cc_y)
            .expect("rifleman should spawn");
        let target = entities
            .spawn_unit(2, EntityKind::Rifleman, cc_x + 96.0, cc_y)
            .expect("target should spawn");
        let node = entities
            .spawn_node(EntityKind::Steel, cc_x + 64.0, cc_y)
            .expect("node should spawn");

        entities
            .get_mut(rifle)
            .unwrap()
            .append_queued_order(OrderIntent::move_to(700.0, 700.0));
        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Move {
                    units: vec![rifle],
                    x: 180.0,
                    y: 180.0,
                    queued: false,
                },
            )],
        );
        assert!(matches!(
            entities.get(rifle).unwrap().order(),
            Order::Move(_)
        ));
        assert!(entities.get(rifle).unwrap().queued_orders().is_empty());

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::AttackMove {
                    units: vec![rifle],
                    x: 220.0,
                    y: 180.0,
                    queued: true,
                },
            )],
        );
        assert!(matches!(
            entities.get(rifle).unwrap().order(),
            Order::Move(_)
        ));
        assert!(matches!(
            entities.get(rifle).unwrap().queued_orders().last(),
            Some(OrderIntent::AttackMove(_))
        ));

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Attack {
                    units: vec![rifle],
                    target,
                    queued: false,
                },
            )],
        );
        assert!(matches!(
            entities.get(rifle).unwrap().order(),
            Order::Attack(_)
        ));
        assert!(entities.get(rifle).unwrap().queued_orders().is_empty());

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Gather {
                    units: vec![worker],
                    node,
                    queued: false,
                },
            )],
        );
        assert!(matches!(
            entities.get(worker).unwrap().order(),
            Order::Gather(_)
        ));

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Build {
                    units: vec![worker],
                    building: EntityKind::Depot,
                    tile_x: 10,
                    tile_y: 10,
                    queued: true,
                },
            )],
        );
        assert!(matches!(
            entities.get(worker).unwrap().queued_orders().last(),
            Some(OrderIntent::Build(_))
        ));
    }

    #[test]
    fn planner_backed_valid_queued_commands_emit_queue_full_notices() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
            .expect("city centre should spawn");
        let mover = entities
            .spawn_unit(1, EntityKind::Tank, cc_x + 16.0, cc_y)
            .expect("tank should spawn");
        let attacker = entities
            .spawn_unit(1, EntityKind::Rifleman, cc_x + 48.0, cc_y)
            .expect("rifleman should spawn");
        let gatherer = entities
            .spawn_unit(1, EntityKind::Worker, cc_x + 16.0, cc_y + 32.0)
            .expect("gather worker should spawn");
        let builder = entities
            .spawn_unit(1, EntityKind::Worker, cc_x + 48.0, cc_y + 32.0)
            .expect("build worker should spawn");
        let target = entities
            .spawn_unit(2, EntityKind::Rifleman, cc_x + 96.0, cc_y)
            .expect("target should spawn");
        let node = entities
            .spawn_node(EntityKind::Steel, cc_x + 64.0, cc_y + 32.0)
            .expect("node should spawn");

        for id in [mover, attacker, gatherer, builder] {
            fill_queue(&mut entities, id);
        }

        let events = apply_with_players(
            &map,
            &mut entities,
            &mut [player_state(1), player_state(2)],
            vec![
                (
                    1,
                    SimCommand::Move {
                        units: vec![mover],
                        x: 160.0,
                        y: 160.0,
                        queued: true,
                    },
                ),
                (
                    1,
                    SimCommand::Attack {
                        units: vec![attacker],
                        target,
                        queued: true,
                    },
                ),
                (
                    1,
                    SimCommand::Gather {
                        units: vec![gatherer],
                        node,
                        queued: true,
                    },
                ),
                (
                    1,
                    SimCommand::Build {
                        units: vec![builder],
                        building: EntityKind::Depot,
                        tile_x: 10,
                        tile_y: 10,
                        queued: true,
                    },
                ),
            ],
        );

        let notices = events.get(&1).map(Vec::as_slice).unwrap_or(&[]);
        assert_eq!(
            notices
                .iter()
                .filter(|event| matches!(
                    event,
                    Event::Notice { msg, .. } if msg == "Command queue full"
                ))
                .count(),
            4,
            "each valid queued command that only fails the queue cap should notify"
        );
        for id in [mover, attacker, gatherer, builder] {
            assert_eq!(entities.get(id).unwrap().queued_orders().len(), 8);
        }
    }

    #[test]
    fn build_with_multiple_selected_workers_uses_idle_closest_worker() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
            .expect("city centre should spawn");
        let busy_close = entities
            .spawn_unit(1, EntityKind::Worker, 555.0, 512.0)
            .expect("busy worker should spawn");
        let idle_far = entities
            .spawn_unit(1, EntityKind::Worker, 100.0, 100.0)
            .expect("far worker should spawn");
        let idle_close = entities
            .spawn_unit(1, EntityKind::Worker, 570.0, 512.0)
            .expect("close worker should spawn");
        let node = entities
            .spawn_node(EntityKind::Steel, 560.0, 560.0)
            .expect("node should spawn");
        entities
            .get_mut(busy_close)
            .expect("busy worker should exist")
            .set_order(Order::gather(node));

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Build {
                    units: vec![busy_close, idle_far, idle_close],
                    building: EntityKind::Depot,
                    tile_x: 15,
                    tile_y: 15,
                    queued: false,
                },
            )],
        );

        assert!(matches!(
            entities.get(idle_close).expect("close worker").order(),
            Order::Build(_)
        ));
        assert!(matches!(
            entities.get(idle_far).expect("far worker").order(),
            Order::Idle
        ));
        assert!(matches!(
            entities.get(busy_close).expect("busy worker").order(),
            Order::Gather(_)
        ));
    }

    #[test]
    fn queued_builds_distribute_across_selected_workers_by_queue_length() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
            .expect("city centre should spawn");
        let first = entities
            .spawn_unit(1, EntityKind::Worker, cc_x + 64.0, cc_y)
            .expect("first worker should spawn");
        let second = entities
            .spawn_unit(1, EntityKind::Worker, cc_x + 96.0, cc_y)
            .expect("second worker should spawn");

        apply(
            &map,
            &mut entities,
            (0..4)
                .map(|i| {
                    (
                        1,
                        SimCommand::Build {
                            units: vec![first, second],
                            building: EntityKind::Depot,
                            tile_x: 10 + i,
                            tile_y: 10,
                            queued: true,
                        },
                    )
                })
                .collect(),
        );

        assert_eq!(entities.get(first).unwrap().queued_orders().len(), 2);
        assert_eq!(entities.get(second).unwrap().queued_orders().len(), 2);
    }

    #[test]
    fn queued_build_prefers_idle_worker_over_closer_active_builder() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
            .expect("city centre should spawn");
        let west = entities
            .spawn_unit(1, EntityKind::Worker, 320.0, 512.0)
            .expect("west worker should spawn");
        let east = entities
            .spawn_unit(1, EntityKind::Worker, 640.0, 512.0)
            .expect("east worker should spawn");
        entities
            .get_mut(west)
            .expect("west worker should exist")
            .set_order(Order::build(EntityKind::Depot, 8, 16));

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Build {
                    units: vec![west, east],
                    building: EntityKind::Depot,
                    tile_x: 9,
                    tile_y: 16,
                    queued: true,
                },
            )],
        );

        assert!(
            entities.get(west).unwrap().queued_orders().is_empty(),
            "closer worker already walking to build should not receive the queued build"
        );
        assert_eq!(
            entities.get(east).unwrap().queued_orders(),
            &[OrderIntent::build(EntityKind::Depot, 9, 16)],
            "idle worker should receive the next queued build"
        );
    }

    #[test]
    fn stop_clears_active_order_and_queued_orders() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        {
            let entity = entities.get_mut(unit).expect("unit should exist");
            entity.set_order(Order::move_to(300.0, 300.0));
            entity.append_queued_order(OrderIntent::move_to(400.0, 400.0));
        }

        apply(
            &map,
            &mut entities,
            vec![(1, SimCommand::Stop { units: vec![unit] })],
        );

        let entity = entities.get(unit).expect("unit should exist");
        assert!(matches!(entity.order(), Order::Idle));
        assert!(entity.queued_orders().is_empty());
    }

    #[test]
    fn legacy_charge_command_is_noop_after_removal() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let rifle = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, 120.0, 100.0)
            .expect("worker should spawn");
        let enemy_rifle = entities
            .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
            .expect("enemy rifleman should spawn");

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::UseAbility {
                    ability: AbilityKind::Charge,
                    units: vec![rifle, worker, enemy_rifle, rifle],
                    x: None,
                    y: None,
                    queued: false,
                },
            )],
        );

        assert_eq!(
            entities.get(rifle).unwrap().charge_ticks(),
            0,
            "charge should be locked before Training Centre is complete"
        );

        let (tx, ty) = footprint_center(&map, EntityKind::TrainingCentre, 6, 6);
        entities
            .spawn_building(1, EntityKind::TrainingCentre, tx, ty, true)
            .expect("training centre should spawn");

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::UseAbility {
                    ability: AbilityKind::Charge,
                    units: vec![rifle, worker, enemy_rifle, rifle],
                    x: None,
                    y: None,
                    queued: false,
                },
            )],
        );

        assert_eq!(
            entities.get(rifle).unwrap().charge_ticks(),
            0,
            "legacy Charge should no longer activate riflemen"
        );
        assert_eq!(
            entities
                .get(rifle)
                .unwrap()
                .ability_cooldown_ticks(AbilityKind::Charge),
            0
        );
        assert_eq!(
            entities.get(worker).unwrap().charge_ticks(),
            0,
            "non-riflemen in the selected list are ignored"
        );
        assert_eq!(
            entities.get(enemy_rifle).unwrap().charge_ticks(),
            0,
            "enemy riflemen are ignored"
        );
    }

    #[test]
    fn legacy_charge_command_does_not_start_cooldown() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let rifle = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        let (tx, ty) = footprint_center(&map, EntityKind::TrainingCentre, 6, 6);
        entities
            .spawn_building(1, EntityKind::TrainingCentre, tx, ty, true)
            .expect("training centre should spawn");

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::UseAbility {
                    ability: AbilityKind::Charge,
                    units: vec![rifle],
                    x: None,
                    y: None,
                    queued: false,
                },
            )],
        );
        let first_charge_ticks = entities.get(rifle).unwrap().charge_ticks();
        let first_cooldown_ticks = entities
            .get(rifle)
            .unwrap()
            .ability_cooldown_ticks(AbilityKind::Charge);

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::UseAbility {
                    ability: AbilityKind::Charge,
                    units: vec![rifle],
                    x: None,
                    y: None,
                    queued: false,
                },
            )],
        );
        assert_eq!(
            entities.get(rifle).unwrap().charge_ticks(),
            first_charge_ticks,
            "cooldown should block immediate charge reuse"
        );
        assert_eq!(
            entities
                .get(rifle)
                .unwrap()
                .ability_cooldown_ticks(AbilityKind::Charge),
            first_cooldown_ticks,
            "retrying during cooldown must not refresh the cooldown"
        );

        for _ in 0..config::RIFLEMAN_CHARGE_COOLDOWN_TICKS {
            entities.get_mut(rifle).unwrap().tick_ability_cooldowns();
        }
        entities.get_mut(rifle).unwrap().tick_charge();

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::UseAbility {
                    ability: AbilityKind::Charge,
                    units: vec![rifle],
                    x: None,
                    y: None,
                    queued: false,
                },
            )],
        );
        assert_eq!(entities.get(rifle).unwrap().charge_ticks(), 0);
        assert_eq!(
            entities
                .get(rifle)
                .unwrap()
                .ability_cooldown_ticks(AbilityKind::Charge),
            0
        );
    }

    #[test]
    fn queued_legacy_charge_is_skipped_and_later_attack_move_hits_selection() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let ready = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("ready rifleman should spawn");
        let cooldown = entities
            .spawn_unit(1, EntityKind::Rifleman, 120.0, 100.0)
            .expect("cooldown rifleman should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, 140.0, 100.0)
            .expect("worker should spawn");
        let (tx, ty) = footprint_center(&map, EntityKind::TrainingCentre, 6, 6);
        entities
            .spawn_building(1, EntityKind::TrainingCentre, tx, ty, true)
            .expect("training centre should spawn");
        entities
            .get_mut(cooldown)
            .unwrap()
            .start_ability_cooldown(AbilityKind::Charge, 5);

        apply(
            &map,
            &mut entities,
            vec![
                (
                    1,
                    SimCommand::UseAbility {
                        ability: AbilityKind::Charge,
                        units: vec![ready, cooldown, worker],
                        x: None,
                        y: None,
                        queued: true,
                    },
                ),
                (
                    1,
                    SimCommand::AttackMove {
                        units: vec![ready, cooldown, worker],
                        x: 400.0,
                        y: 100.0,
                        queued: true,
                    },
                ),
            ],
        );

        assert_eq!(entities.get(ready).unwrap().queued_orders().len(), 1);
        assert_eq!(
            entities.get(cooldown).unwrap().queued_orders().len(),
            1,
            "cooldown rifleman should skip Charge but still receive the later attack-move"
        );
        assert_eq!(
            entities.get(worker).unwrap().queued_orders().len(),
            1,
            "non-rifleman should skip Charge but still receive the later attack-move"
        );
    }

    #[test]
    fn in_range_smoke_launches_from_furthest_selected_carrier() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let target = map.tile_center(12, 8);
        let near = entities
            .spawn_unit(1, EntityKind::ScoutCar, target.0 - 96.0, target.1)
            .expect("near scout car should spawn");
        let far = entities
            .spawn_unit(1, EntityKind::ScoutCar, target.0 - 192.0, target.1)
            .expect("far scout car should spawn");
        let (sx, sy) = footprint_center(&map, EntityKind::Steelworks, 4, 4);
        entities
            .spawn_building(1, EntityKind::Steelworks, sx, sy, true)
            .expect("steelworks should spawn");
        let mut players = vec![player_state(1), player_state(2)];
        let mut smokes = SmokeCloudStore::new();
        let events = apply_with_players_and_smokes(
            &map,
            &mut entities,
            &mut players,
            &mut smokes,
            vec![(
                1,
                SimCommand::UseAbility {
                    ability: AbilityKind::Smoke,
                    units: vec![near, far],
                    x: Some(target.0),
                    y: Some(target.1),
                    queued: false,
                },
            )],
        );

        assert_eq!(smokes.iter().count(), 0);
        smokes.spawn_due(1 + config::SMOKE_LAUNCH_MAX_DELAY_TICKS);
        assert_eq!(smokes.iter().count(), 1);
        assert_eq!(players[0].steel, 1000);
        assert_eq!(players[0].oil, 1000);
        assert_eq!(
            entities
                .get(far)
                .unwrap()
                .ability_cooldown_ticks(AbilityKind::Smoke),
            config::SMOKE_ABILITY_COOLDOWN_TICKS,
            "furthest in-range selected carrier should launch"
        );
        assert_eq!(
            entities
                .get(near)
                .unwrap()
                .ability_cooldown_ticks(AbilityKind::Smoke),
            0
        );
        assert!(matches!(entities.get(far).unwrap().order(), Order::Idle));
        // Smoke launch emits local canister feedback plus a positioned info notice; no warn/alert events.
        let player_events = events.get(&1).map(Vec::as_slice).unwrap_or(&[]);
        assert!(player_events.iter().any(|ev| matches!(
            ev,
            Event::SmokeLaunch {
                from_x,
                from_y,
                to_x,
                to_y,
                delay_ticks,
            } if (*from_x - (target.0 - 192.0)).abs() < 0.001
                && (*from_y - target.1).abs() < 0.001
                && (*to_x - target.0).abs() < 0.001
                && (*to_y - target.1).abs() < 0.001
                && *delay_ticks == 2
        )));
        assert!(
            player_events.iter().all(|ev| matches!(
                ev,
                Event::Notice {
                    severity: crate::protocol::NoticeSeverity::Info,
                    ..
                } | Event::SmokeLaunch { .. }
            )),
            "smoke launch should emit at most info-level notices, got: {player_events:?}"
        );
    }

    #[test]
    fn in_range_smoke_preserves_active_move_and_future_queue() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let target = map.tile_center(12, 8);
        let scout = entities
            .spawn_unit(1, EntityKind::ScoutCar, target.0 - 96.0, target.1)
            .expect("scout car should spawn");
        let (sx, sy) = footprint_center(&map, EntityKind::Steelworks, 4, 4);
        entities
            .spawn_building(1, EntityKind::Steelworks, sx, sy, true)
            .expect("steelworks should spawn");

        apply(
            &map,
            &mut entities,
            vec![
                (
                    1,
                    SimCommand::Move {
                        units: vec![scout],
                        x: 640.0,
                        y: 320.0,
                        queued: false,
                    },
                ),
                (
                    1,
                    SimCommand::Move {
                        units: vec![scout],
                        x: 704.0,
                        y: 384.0,
                        queued: true,
                    },
                ),
            ],
        );
        let before_queue = entities.get(scout).unwrap().queued_orders().to_vec();
        assert!(matches!(
            entities.get(scout).unwrap().order(),
            Order::Move(_)
        ));

        let mut players = vec![player_state(1), player_state(2)];
        let mut smokes = SmokeCloudStore::new();
        apply_with_players_and_smokes(
            &map,
            &mut entities,
            &mut players,
            &mut smokes,
            vec![(
                1,
                SimCommand::UseAbility {
                    ability: AbilityKind::Smoke,
                    units: vec![scout],
                    x: Some(target.0),
                    y: Some(target.1),
                    queued: false,
                },
            )],
        );

        assert_eq!(smokes.iter().count(), 0);
        smokes.spawn_due(1 + config::SMOKE_LAUNCH_MAX_DELAY_TICKS);
        assert_eq!(smokes.iter().count(), 1);
        let scout_entity = entities.get(scout).expect("scout should remain alive");
        assert!(
            matches!(scout_entity.order(), Order::Move(_)),
            "reactive in-range smoke should not interrupt the active move"
        );
        assert_eq!(
            scout_entity.queued_orders(),
            before_queue.as_slice(),
            "reactive in-range smoke should preserve queued future orders"
        );
    }

    #[test]
    fn mortar_fire_replaces_active_move_order() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let mortar = entities
            .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
            .expect("mortar should spawn");
        {
            let mortar_entity = entities.get_mut(mortar).expect("mortar should exist");
            mortar_entity.set_weapon_setup(WeaponSetup::Deployed);
            mortar_entity.set_order(Order::move_to(640.0, 100.0));
            mortar_entity.set_path(vec![(160.0, 100.0), (640.0, 100.0)]);
            mortar_entity.set_path_goal(Some((640.0, 100.0)));
            mortar_entity.append_queued_order(OrderIntent::move_to(704.0, 100.0));
        }

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::UseAbility {
                    ability: AbilityKind::MortarFire,
                    units: vec![mortar],
                    x: Some(180.0),
                    y: Some(100.0),
                    queued: false,
                },
            )],
        );

        let mortar_entity = entities.get(mortar).expect("mortar should remain alive");
        assert!(
            matches!(mortar_entity.order(), Order::Ability(order)
                if order.intent.ability == AbilityKind::MortarFire),
            "manual Mortar Fire should replace the active move with an ability order"
        );
        assert!(
            mortar_entity.path_is_empty(),
            "replacing movement should stop the current path"
        );
        assert_eq!(
            mortar_entity.path_goal(),
            None,
            "in-range Mortar Fire should hold at the current position"
        );
        assert!(
            mortar_entity.queued_orders().is_empty(),
            "non-queued Mortar Fire should clear future queued orders"
        );
    }

    #[test]
    fn queued_smoke_appends_to_eligible_carriers_until_cap() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let target = map.tile_center(12, 8);
        let scout = entities
            .spawn_unit(1, EntityKind::ScoutCar, target.0 - 96.0, target.1)
            .expect("scout car should spawn");
        let rifle = entities
            .spawn_unit(1, EntityKind::Rifleman, target.0 - 64.0, target.1)
            .expect("rifleman should spawn");
        let (sx, sy) = footprint_center(&map, EntityKind::Steelworks, 4, 4);
        entities
            .spawn_building(1, EntityKind::Steelworks, sx, sy, true)
            .expect("steelworks should spawn");

        apply(
            &map,
            &mut entities,
            (0..10)
                .map(|_| {
                    (
                        1,
                        SimCommand::UseAbility {
                            ability: AbilityKind::Smoke,
                            units: vec![scout, rifle],
                            x: Some(target.0),
                            y: Some(target.1),
                            queued: true,
                        },
                    )
                })
                .collect(),
        );

        assert_eq!(entities.get(scout).unwrap().queued_orders().len(), 8);
        assert!(entities
            .get(scout)
            .unwrap()
            .queued_orders()
            .iter()
            .all(|intent| matches!(intent, OrderIntent::WorldAbility(_))));
        assert!(
            entities.get(rifle).unwrap().queued_orders().is_empty(),
            "non-carriers should not receive queued Smoke intents"
        );

        apply(
            &map,
            &mut entities,
            vec![(1, SimCommand::Stop { units: vec![scout] })],
        );
        assert!(entities.get(scout).unwrap().queued_orders().is_empty());
    }

    #[test]
    fn queued_smoke_distributes_one_click_per_ready_scout_by_queue_length() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let target = map.tile_center(12, 8);
        let first = entities
            .spawn_unit(1, EntityKind::ScoutCar, target.0 - 96.0, target.1)
            .expect("first scout car should spawn");
        let second = entities
            .spawn_unit(1, EntityKind::ScoutCar, target.0 - 128.0, target.1)
            .expect("second scout car should spawn");
        let cooling = entities
            .spawn_unit(1, EntityKind::ScoutCar, target.0 - 160.0, target.1)
            .expect("cooling scout car should spawn");
        entities
            .get_mut(cooling)
            .unwrap()
            .start_ability_cooldown(AbilityKind::Smoke, 5);
        let (sx, sy) = footprint_center(&map, EntityKind::Steelworks, 4, 4);
        entities
            .spawn_building(1, EntityKind::Steelworks, sx, sy, true)
            .expect("steelworks should spawn");

        apply(
            &map,
            &mut entities,
            (0..4)
                .map(|i| {
                    (
                        1,
                        SimCommand::UseAbility {
                            ability: AbilityKind::Smoke,
                            units: vec![first, second, cooling],
                            x: Some(target.0 + i as f32),
                            y: Some(target.1),
                            queued: true,
                        },
                    )
                })
                .collect(),
        );

        assert_eq!(entities.get(first).unwrap().queued_orders().len(), 2);
        assert_eq!(entities.get(second).unwrap().queued_orders().len(), 2);
        assert!(
            entities.get(cooling).unwrap().queued_orders().is_empty(),
            "cooldown scout car should not receive queued smoke at issue time"
        );
    }

    #[test]
    fn smoke_launches_without_resource_cost() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let target = map.tile_center(12, 8);
        let scout = entities
            .spawn_unit(1, EntityKind::ScoutCar, target.0 - 96.0, target.1)
            .expect("scout car should spawn");
        let (sx, sy) = footprint_center(&map, EntityKind::Steelworks, 4, 4);
        entities
            .spawn_building(1, EntityKind::Steelworks, sx, sy, true)
            .expect("steelworks should spawn");
        let mut players = vec![player_state(1), player_state(2)];
        players[0].steel = 0;
        players[0].oil = 0;
        let mut smokes = SmokeCloudStore::new();

        let events = apply_with_players_and_smokes(
            &map,
            &mut entities,
            &mut players,
            &mut smokes,
            vec![(
                1,
                SimCommand::UseAbility {
                    ability: AbilityKind::Smoke,
                    units: vec![scout],
                    x: Some(target.0),
                    y: Some(target.1),
                    queued: false,
                },
            )],
        );

        assert_eq!(smokes.iter().count(), 0);
        smokes.spawn_due(1 + config::SMOKE_LAUNCH_MAX_DELAY_TICKS);
        assert_eq!(smokes.iter().count(), 1);
        assert_eq!(players[0].steel, 0);
        assert_eq!(players[0].oil, 0);
        assert!(events.get(&1).is_none_or(|events| {
            events.iter().all(|ev| {
                matches!(
                    ev,
                    Event::Notice {
                        severity: crate::protocol::NoticeSeverity::Info,
                        ..
                    } | Event::SmokeLaunch { .. }
                )
            })
        }));
    }

    #[test]
    fn queued_move_ignores_stale_ids_and_invalid_coordinates() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Move {
                    units: vec![unit, 99_999],
                    x: f32::NAN,
                    y: 140.0,
                    queued: true,
                },
            )],
        );

        assert!(
            entities
                .get(unit)
                .expect("unit should exist")
                .queued_orders()
                .is_empty(),
            "invalid queued point should be ignored without appending or panicking"
        );
    }

    #[test]
    fn oversized_queued_unit_lists_are_deduped_and_capped_before_appending() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let owned = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("owned unit should spawn");
        let enemy = entities
            .spawn_unit(2, EntityKind::Rifleman, 130.0, 100.0)
            .expect("enemy unit should spawn");
        let mut units = vec![owned; 20_000];
        units.extend([99_999, enemy, owned]);

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Move {
                    units,
                    x: 180.0,
                    y: 180.0,
                    queued: true,
                },
            )],
        );

        assert_eq!(
            entities
                .get(owned)
                .expect("owned unit should exist")
                .queued_orders()
                .len(),
            1,
            "repeated ids, stale ids, and enemy ids should not multiply queued state"
        );
        assert!(
            entities
                .get(enemy)
                .expect("enemy unit should exist")
                .queued_orders()
                .is_empty(),
            "enemy ids in a hostile queued command must be ignored"
        );
    }

    #[test]
    fn queued_attack_and_gather_reject_dead_or_depleted_targets_before_appending() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
            .expect("city centre should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, cc_x + 16.0, cc_y)
            .expect("worker should spawn");
        let rifle = entities
            .spawn_unit(1, EntityKind::Rifleman, cc_x + 32.0, cc_y)
            .expect("rifleman should spawn");
        let target = entities
            .spawn_unit(2, EntityKind::Rifleman, cc_x + 96.0, cc_y)
            .expect("target should spawn");
        let node = entities
            .spawn_node(EntityKind::Steel, cc_x + 64.0, cc_y)
            .expect("node should spawn");
        entities.get_mut(target).expect("target should exist").hp = 0;
        if let Some(resource) = entities
            .get_mut(node)
            .expect("node should exist")
            .resource_node
            .as_mut()
        {
            resource.remaining = 0;
        }

        apply(
            &map,
            &mut entities,
            vec![
                (
                    1,
                    SimCommand::Attack {
                        units: vec![rifle],
                        target,
                        queued: true,
                    },
                ),
                (
                    1,
                    SimCommand::Gather {
                        units: vec![worker],
                        node,
                        queued: true,
                    },
                ),
            ],
        );

        assert!(
            entities
                .get(rifle)
                .expect("rifleman should exist")
                .queued_orders()
                .is_empty(),
            "dead attack targets should not create queued attack intents"
        );
        assert!(
            entities
                .get(worker)
                .expect("worker should exist")
                .queued_orders()
                .is_empty(),
            "depleted resources should not create queued gather intents"
        );
    }

    #[test]
    fn repeated_invalid_queued_builds_stay_bounded() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, 100.0, 100.0)
            .expect("worker should spawn");
        let pending = (0..32)
            .map(|_| {
                (
                    1,
                    SimCommand::Build {
                        units: vec![worker],
                        building: EntityKind::Depot,
                        tile_x: u32::MAX,
                        tile_y: u32::MAX,
                        queued: true,
                    },
                )
            })
            .collect();

        apply(&map, &mut entities, pending);

        assert_eq!(
            entities
                .get(worker)
                .expect("worker should exist")
                .queued_orders()
                .len(),
            8,
            "queued build intents should enforce the per-unit queue cap even when invalid"
        );
    }

    #[test]
    fn queued_rally_appends_until_four_stages_and_normal_rally_clears_queue() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::Barracks, 6, 6);
        let barracks = entities
            .spawn_building(1, EntityKind::Barracks, bx, by, true)
            .expect("barracks should spawn");

        apply(
            &map,
            &mut entities,
            vec![
                (
                    1,
                    SimCommand::SetRally {
                        building: barracks,
                        x: 100.0,
                        y: 100.0,
                        kind: RallyKind::Move,
                        queued: true,
                    },
                ),
                (
                    1,
                    SimCommand::SetRally {
                        building: barracks,
                        x: 200.0,
                        y: 200.0,
                        kind: RallyKind::AttackMove,
                        queued: true,
                    },
                ),
                (
                    1,
                    SimCommand::SetRally {
                        building: barracks,
                        x: 300.0,
                        y: 300.0,
                        kind: RallyKind::Move,
                        queued: true,
                    },
                ),
                (
                    1,
                    SimCommand::SetRally {
                        building: barracks,
                        x: 400.0,
                        y: 400.0,
                        kind: RallyKind::AttackMove,
                        queued: true,
                    },
                ),
                (
                    1,
                    SimCommand::SetRally {
                        building: barracks,
                        x: 500.0,
                        y: 500.0,
                        kind: RallyKind::Move,
                        queued: true,
                    },
                ),
            ],
        );

        assert_eq!(
            entities.get(barracks).unwrap().rally_point(),
            Some((100.0, 100.0)),
            "first queued rally should establish the active rally point"
        );
        let stages = entities.get(barracks).unwrap().rally_stages();
        assert_eq!(
            stages.len(),
            3,
            "rally plan should be capped at four total stages"
        );
        assert_eq!(stages[0].kind, RallyKind::AttackMove);
        assert_eq!((stages[0].point.x, stages[0].point.y), (200.0, 200.0));
        assert_eq!((stages[2].point.x, stages[2].point.y), (400.0, 400.0));

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::SetRally {
                    building: barracks,
                    x: 600.0,
                    y: 600.0,
                    kind: RallyKind::Move,
                    queued: false,
                },
            )],
        );

        let barracks = entities.get(barracks).expect("barracks should exist");
        assert!(barracks.rally_stages().is_empty());
        assert_eq!(barracks.rally_point(), Some((600.0, 600.0)));
    }

    #[test]
    fn setup_anti_tank_guns_filters_mixed_selection_and_records_facing() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let at = entities
            .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
            .expect("at gun should spawn");
        let rifle = entities
            .spawn_unit(1, EntityKind::Rifleman, 120.0, 100.0)
            .expect("rifleman should spawn");
        let enemy_at = entities
            .spawn_unit(2, EntityKind::AntiTankGun, 140.0, 100.0)
            .expect("enemy at gun should spawn");

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::SetupAntiTankGuns {
                    units: vec![at, rifle, enemy_at, at],
                    x: 100.0,
                    y: 140.0,
                    queued: false,
                },
            )],
        );

        let at = entities.get(at).expect("at gun should exist");
        assert_eq!(at.weapon_setup(), WeaponSetup::Packed);
        assert!(
            (at.emplacement_facing().unwrap_or_default() - std::f32::consts::FRAC_PI_2).abs()
                < 0.001,
            "setup command should store a finite facing toward the target point"
        );
        assert!(
            at.facing().abs() < 0.001,
            "setup command should not snap the anti-tank gun body to the target facing"
        );
        assert_eq!(
            entities
                .get(rifle)
                .expect("rifleman should exist")
                .weapon_setup(),
            WeaponSetup::Packed,
            "non-setup-capable units in the selected list are ignored"
        );
        assert_eq!(
            entities
                .get(enemy_at)
                .expect("enemy at gun should exist")
                .weapon_setup(),
            WeaponSetup::Packed,
            "enemy anti-tank guns are ignored"
        );
    }

    #[test]
    fn queued_setup_anti_tank_guns_filters_to_anti_tank_guns_and_preserves_later_attack_move() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let at = entities
            .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
            .expect("at gun should spawn");
        let rifle = entities
            .spawn_unit(1, EntityKind::Rifleman, 120.0, 100.0)
            .expect("rifleman should spawn");

        apply(
            &map,
            &mut entities,
            vec![
                (
                    1,
                    SimCommand::SetupAntiTankGuns {
                        units: vec![at, rifle],
                        x: 100.0,
                        y: 140.0,
                        queued: true,
                    },
                ),
                (
                    1,
                    SimCommand::AttackMove {
                        units: vec![at, rifle],
                        x: 220.0,
                        y: 100.0,
                        queued: true,
                    },
                ),
            ],
        );

        assert!(matches!(
            entities.get(at).unwrap().queued_orders()[0],
            OrderIntent::SetupAntiTankGuns(_)
        ));
        assert_eq!(entities.get(at).unwrap().queued_orders().len(), 2);
        assert_eq!(
            entities.get(rifle).unwrap().queued_orders().len(),
            1,
            "non-setup-capable units skip setup but keep later compatible stages"
        );
    }

    #[test]
    fn artillery_point_fire_inside_arc_keeps_setup_facing_fixed() {
        let map = flat_map(64);
        let mut entities = EntityStore::new();
        let mut players = vec![player_state(1), player_state(2)];
        let pos = (320.0, 320.0);
        let angle = config::ARTILLERY_FIELD_OF_FIRE_RAD * 0.45;
        let distance = config::TILE_SIZE as f32 * 22.0;
        let target = (
            pos.0 + angle.cos() * distance,
            pos.1 + angle.sin() * distance,
        );
        let artillery = entities
            .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
            .expect("artillery should spawn");
        {
            let unit = entities.get_mut(artillery).expect("artillery should exist");
            unit.set_weapon_setup(WeaponSetup::Deployed);
            unit.set_emplacement_facing(Some(0.0));
            unit.set_weapon_facing(0.0);
        }

        let events = apply_with_players(
            &map,
            &mut entities,
            &mut players,
            vec![(
                1,
                SimCommand::UseAbility {
                    ability: AbilityKind::PointFire,
                    units: vec![artillery],
                    x: Some(target.0),
                    y: Some(target.1),
                    queued: false,
                },
            )],
        );

        let unit = entities.get(artillery).expect("artillery should exist");
        assert!(matches!(unit.weapon_setup(), WeaponSetup::Deployed));
        assert!(matches!(unit.order(), Order::ArtilleryPointFire(_)));
        assert!(
            unit.emplacement_facing().unwrap_or_default().abs() < 0.001,
            "in-arc point fire must not recenter the deployed field of fire"
        );
        assert_eq!(players[0].steel, 1_000 - config::ARTILLERY_AMMO_COST_STEEL);
        assert!(events.get(&1).is_some_and(|events| events.iter().any(
            |event| matches!(event, Event::ArtilleryTarget { from, .. } if *from == artillery)
        )));
    }

    #[test]
    fn artillery_point_fire_outside_arc_replaces_active_fire_with_redeploy() {
        let map = flat_map(64);
        let mut entities = EntityStore::new();
        let mut players = vec![player_state(1), player_state(2)];
        let pos = (320.0, 320.0);
        let old_target = (pos.0 + config::TILE_SIZE as f32 * 22.0, pos.1);
        let angle = config::ARTILLERY_FIELD_OF_FIRE_RAD;
        let distance = config::TILE_SIZE as f32 * 22.0;
        let target = (
            pos.0 + angle.cos() * distance,
            pos.1 + angle.sin() * distance,
        );
        let artillery = entities
            .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
            .expect("artillery should spawn");
        {
            let unit = entities.get_mut(artillery).expect("artillery should exist");
            unit.set_weapon_setup(WeaponSetup::Deployed);
            unit.set_emplacement_facing(Some(0.0));
            unit.set_weapon_facing(0.0);
            unit.set_attack_cd(config::ARTILLERY_RELOAD_TICKS);
            unit.set_order(Order::artillery_point_fire(old_target.0, old_target.1));
        }

        let events = apply_with_players(
            &map,
            &mut entities,
            &mut players,
            vec![(
                1,
                SimCommand::UseAbility {
                    ability: AbilityKind::PointFire,
                    units: vec![artillery],
                    x: Some(target.0),
                    y: Some(target.1),
                    queued: false,
                },
            )],
        );

        let unit = entities.get(artillery).expect("artillery should exist");
        assert!(matches!(
            unit.weapon_setup(),
            WeaponSetup::TearingDownToRedeploy { .. }
        ));
        assert!(matches!(unit.order(), Order::ArtilleryPointFire(_)));
        assert!(
            (unit.pending_redeploy_facing().unwrap_or_default() - angle).abs() < 0.001,
            "outside-arc point fire should store the requested redeploy facing"
        );
        assert_eq!(players[0].steel, 1_000);
        assert!(events
            .values()
            .flat_map(|events| events.iter())
            .all(|event| !matches!(event, Event::ArtilleryTarget { .. })));
        let Order::ArtilleryPointFire(order) = unit.order() else {
            panic!("retarget should keep an artillery point-fire order");
        };
        assert!((order.intent.x - target.0).abs() < 0.001);
        assert!((order.intent.y - target.1).abs() < 0.001);
    }

    #[test]
    fn artillery_point_fire_can_retarget_while_redeploying() {
        let map = flat_map(64);
        let mut entities = EntityStore::new();
        let mut players = vec![player_state(1), player_state(2)];
        let pos = (320.0, 320.0);
        let old_angle = config::ARTILLERY_FIELD_OF_FIRE_RAD;
        let new_angle = -config::ARTILLERY_FIELD_OF_FIRE_RAD;
        let distance = config::TILE_SIZE as f32 * 22.0;
        let old_target = (
            pos.0 + old_angle.cos() * distance,
            pos.1 + old_angle.sin() * distance,
        );
        let target = (
            pos.0 + new_angle.cos() * distance,
            pos.1 + new_angle.sin() * distance,
        );
        let artillery = entities
            .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
            .expect("artillery should spawn");
        {
            let unit = entities.get_mut(artillery).expect("artillery should exist");
            unit.set_weapon_setup(WeaponSetup::TearingDownToRedeploy {
                ticks: config::ARTILLERY_SETUP_TICKS,
            });
            unit.set_emplacement_facing(Some(0.0));
            unit.set_pending_redeploy_facing(Some(old_angle));
            unit.set_weapon_facing(0.0);
            unit.set_order(Order::artillery_point_fire(old_target.0, old_target.1));
        }

        apply_with_players(
            &map,
            &mut entities,
            &mut players,
            vec![(
                1,
                SimCommand::UseAbility {
                    ability: AbilityKind::PointFire,
                    units: vec![artillery],
                    x: Some(target.0),
                    y: Some(target.1),
                    queued: false,
                },
            )],
        );

        let unit = entities.get(artillery).expect("artillery should exist");
        assert!(matches!(
            unit.weapon_setup(),
            WeaponSetup::TearingDownToRedeploy { .. }
        ));
        let Order::ArtilleryPointFire(order) = unit.order() else {
            panic!("retarget should keep an artillery point-fire order");
        };
        assert!((order.intent.x - target.0).abs() < 0.001);
        assert!((order.intent.y - target.1).abs() < 0.001);
        assert!(
            (unit.pending_redeploy_facing().unwrap_or_default() - new_angle).abs() < 0.001,
            "retargeting during redeploy should update the pending facing"
        );
    }

    #[test]
    fn teardown_anti_tank_guns_only_affects_setting_up_or_deployed_anti_tank_guns() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let deployed = entities
            .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
            .expect("at gun should spawn");
        let packed = entities
            .spawn_unit(1, EntityKind::AntiTankGun, 130.0, 100.0)
            .expect("at gun should spawn");
        entities
            .get_mut(deployed)
            .unwrap()
            .set_weapon_setup(WeaponSetup::Deployed);
        entities
            .get_mut(packed)
            .unwrap()
            .set_emplacement_facing(Some(1.0));

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::TearDownAntiTankGuns {
                    units: vec![deployed, packed],
                },
            )],
        );

        assert!(matches!(
            entities.get(deployed).unwrap().weapon_setup(),
            WeaponSetup::TearingDown { .. }
        ));
        assert_eq!(
            entities.get(packed).unwrap().weapon_setup(),
            WeaponSetup::Packed
        );
        assert_eq!(
            entities.get(packed).unwrap().emplacement_facing(),
            None,
            "teardown should cancel a packed anti-tank gun's staged setup facing"
        );
    }

    #[test]
    fn move_order_tears_down_deployed_anti_tank_guns_before_moving() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let deployed = entities
            .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
            .expect("at gun should spawn");
        let packed = entities
            .spawn_unit(1, EntityKind::AntiTankGun, 130.0, 100.0)
            .expect("at gun should spawn");
        {
            let at = entities.get_mut(deployed).unwrap();
            at.set_weapon_setup(WeaponSetup::Deployed);
            at.set_emplacement_facing(Some(0.25));
            at.set_facing(0.25);
            at.set_weapon_facing(0.25);
        }

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Move {
                    units: vec![deployed, packed],
                    x: 220.0,
                    y: 100.0,
                    queued: false,
                },
            )],
        );

        let deployed = entities.get(deployed).expect("at gun should exist");
        assert!(matches!(
            deployed.weapon_setup(),
            WeaponSetup::TearingDown { .. }
        ));
        assert_eq!(
            deployed.facing(),
            0.25,
            "move order should not instantly rotate a deployed anti-tank gun before it moves"
        );
        assert!(
            matches!(deployed.order(), Order::Move(_)),
            "move should replace the deployed anti-tank gun order"
        );
        assert!(
            deployed.path_goal().is_some(),
            "move should preserve the movement destination while the anti-tank gun tears down"
        );
        assert_eq!(deployed.emplacement_facing(), None);
        assert_eq!(deployed.pending_redeploy_facing(), None);

        let packed = entities.get(packed).expect("packed at gun should exist");
        assert!(
            matches!(packed.order(), Order::Move(_)),
            "packed anti-tank guns should still accept move orders"
        );
    }

    #[test]
    fn attack_move_order_tears_down_deployed_anti_tank_guns_before_moving() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let deployed = entities
            .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
            .expect("at gun should spawn");
        {
            let at = entities.get_mut(deployed).unwrap();
            at.set_weapon_setup(WeaponSetup::Deployed);
            at.set_emplacement_facing(Some(-0.5));
            at.set_facing(-0.5);
            at.set_weapon_facing(-0.5);
        }

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::AttackMove {
                    units: vec![deployed],
                    x: 220.0,
                    y: 100.0,
                    queued: false,
                },
            )],
        );

        let deployed = entities.get(deployed).expect("at gun should exist");
        assert!(matches!(
            deployed.weapon_setup(),
            WeaponSetup::TearingDown { .. }
        ));
        assert_eq!(
            deployed.facing(),
            -0.5,
            "attack-move should not instantly rotate a deployed anti-tank gun before it moves"
        );
        assert!(
            matches!(deployed.order(), Order::AttackMove(_)),
            "attack-move should replace the deployed anti-tank gun order"
        );
        assert!(
            deployed.path_goal().is_some(),
            "attack-move should preserve the movement destination while the anti-tank gun tears down"
        );
        assert_eq!(deployed.emplacement_facing(), None);
        assert_eq!(deployed.pending_redeploy_facing(), None);
    }

    #[test]
    fn deployed_anti_tank_gun_rejects_explicit_attack_outside_field_of_fire() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let at = entities
            .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
            .expect("at gun should spawn");
        let front_target = entities
            .spawn_unit(2, EntityKind::Tank, 220.0, 100.0)
            .expect("target should spawn");
        let side_target = entities
            .spawn_unit(2, EntityKind::Tank, 100.0, 220.0)
            .expect("target should spawn");
        {
            let at = entities.get_mut(at).unwrap();
            at.set_weapon_setup(WeaponSetup::Deployed);
            at.set_emplacement_facing(Some(0.0));
        }

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Attack {
                    units: vec![at],
                    target: side_target,
                    queued: false,
                },
            )],
        );
        assert!(
            !matches!(entities.get(at).unwrap().order(), Order::Attack(_)),
            "out-of-arc attack should be ignored for the deployed anti-tank gun"
        );

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Attack {
                    units: vec![at],
                    target: front_target,
                    queued: false,
                },
            )],
        );
        assert!(
            matches!(entities.get(at).unwrap().order(), Order::Attack(_)),
            "in-arc attack should still be accepted"
        );
    }

    #[test]
    fn attack_command_rejects_hidden_targets() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let rifle = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        let hidden_target = entities
            .spawn_unit(2, EntityKind::Tank, 420.0, 100.0)
            .expect("hidden target should spawn");

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Attack {
                    units: vec![rifle],
                    target: hidden_target,
                    queued: false,
                },
            )],
        );

        let rifle = entities.get(rifle).expect("rifleman should exist");
        assert!(
            !matches!(rifle.order(), Order::Attack(_)),
            "hidden target ids should not become attack orders"
        );
        assert_eq!(rifle.target_id(), None);
        assert_eq!(rifle.path_goal(), None);
    }

    #[test]
    fn attack_command_rejects_allied_targets() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let rifle = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        let ally = entities
            .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
            .expect("ally should spawn");
        let mut players = vec![player_state(1), player_state(2)];
        players[0].team_id = 7;
        players[1].team_id = 7;

        apply_with_players(
            &map,
            &mut entities,
            &mut players,
            vec![(
                1,
                SimCommand::Attack {
                    units: vec![rifle],
                    target: ally,
                    queued: false,
                },
            )],
        );

        let rifle = entities.get(rifle).expect("rifleman should exist");
        assert!(
            !matches!(rifle.order(), Order::Attack(_)),
            "allied target ids should not become attack orders"
        );
        assert_eq!(rifle.target_id(), None);
        assert_eq!(rifle.path_goal(), None);
    }

    #[test]
    fn train_resource_shortages_emit_specific_notices() {
        let map = flat_map(24);

        let mut oil_missing_entities = EntityStore::new();
        let (fx, fy) = footprint_center(&map, EntityKind::Factory, 6, 6);
        let factory = oil_missing_entities
            .spawn_building(1, EntityKind::Factory, fx, fy, true)
            .expect("factory should spawn");
        let mut oil_missing_players = vec![player_state(1), player_state(2)];
        oil_missing_players[0].oil = 0;
        let oil_missing_events = apply_with_players(
            &map,
            &mut oil_missing_entities,
            &mut oil_missing_players,
            vec![(
                1,
                SimCommand::Train {
                    building: factory,
                    unit: EntityKind::ScoutCar,
                },
            )],
        );
        assert!(
            matches!(
                oil_missing_events.get(&1).and_then(|events| events.first()),
                Some(Event::Notice { msg, .. }) if msg == "Not enough oil"
            ),
            "oil-gated units should emit the oil voice-line notice"
        );

        let mut steel_missing_entities = EntityStore::new();
        let (cx, cy) = footprint_center(&map, EntityKind::CityCentre, 6, 6);
        let city_centre = steel_missing_entities
            .spawn_building(1, EntityKind::CityCentre, cx, cy, true)
            .expect("city centre should spawn");
        let mut steel_missing_players = vec![player_state(1), player_state(2)];
        steel_missing_players[0].steel = 0;
        let steel_missing_events = apply_with_players(
            &map,
            &mut steel_missing_entities,
            &mut steel_missing_players,
            vec![(
                1,
                SimCommand::Train {
                    building: city_centre,
                    unit: EntityKind::Worker,
                },
            )],
        );
        assert!(
            matches!(
                steel_missing_events.get(&1).and_then(|events| events.first()),
                Some(Event::Notice { msg, .. }) if msg == "Not enough steel"
            ),
            "steel-only units should emit the steel voice-line notice"
        );
    }

    #[test]
    fn cancel_train_removes_latest_queued_unit_without_resetting_active_progress() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::Barracks, 6, 6);
        let barracks = entities
            .spawn_building(1, EntityKind::Barracks, bx, by, true)
            .expect("barracks should spawn");
        let (tx, ty) = footprint_center(&map, EntityKind::TrainingCentre, 10, 6);
        entities
            .spawn_building(1, EntityKind::TrainingCentre, tx, ty, true)
            .expect("training centre should spawn");
        let mut players = vec![player_state(1), player_state(2)];

        apply_with_players(
            &map,
            &mut entities,
            &mut players,
            vec![(
                1,
                SimCommand::Train {
                    building: barracks,
                    unit: EntityKind::Rifleman,
                },
            )],
        );
        entities
            .get_mut(barracks)
            .expect("barracks should exist")
            .set_front_production_progress(17);
        apply_with_players(
            &map,
            &mut entities,
            &mut players,
            vec![(
                1,
                SimCommand::Train {
                    building: barracks,
                    unit: EntityKind::MachineGunner,
                },
            )],
        );
        let steel_after_queue = players[0].steel;
        let oil_after_queue = players[0].oil;
        let supply_after_queue = players[0].supply_used;

        apply_with_players(
            &map,
            &mut entities,
            &mut players,
            vec![(1, SimCommand::Cancel { building: barracks })],
        );

        let queue = entities.get(barracks).expect("barracks").prod_queue();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].unit, EntityKind::Rifleman);
        assert_eq!(
            queue[0].progress, 17,
            "canceling queued production should not reset active progress"
        );
        let (refunded_steel, refunded_oil) = rules::economy::cost(EntityKind::MachineGunner);
        assert_eq!(players[0].steel, steel_after_queue + refunded_steel);
        assert_eq!(players[0].oil, oil_after_queue + refunded_oil);
        assert_eq!(
            players[0].supply_used,
            supply_after_queue - rules::economy::supply_cost(EntityKind::MachineGunner)
        );
    }

    #[test]
    fn command_budget_allows_twenty_four_one_supply_units() {
        let map = flat_map(64);
        let mut entities = EntityStore::new();
        let units = spawn_units(&mut entities, 1, EntityKind::Rifleman, 24);
        mark_units_moving(&mut entities, &units);

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::Stop {
                    units: units.clone(),
                },
            )],
        );

        assert!(
            units.iter().all(|id| matches!(
                entities.get(*id).map(|entity| entity.order()),
                Some(Order::Idle)
            )),
            "24 one-supply units should fit the base command budget"
        );
    }

    #[test]
    fn command_budget_rejects_fifth_tank_without_command_car() {
        let map = flat_map(64);
        let mut entities = EntityStore::new();
        let units = spawn_units(&mut entities, 1, EntityKind::Tank, 5);
        mark_units_moving(&mut entities, &units);
        let events = apply_with_players(
            &map,
            &mut entities,
            &mut [player_state(1), player_state(2)],
            vec![(
                1,
                SimCommand::Stop {
                    units: units.clone(),
                },
            )],
        );

        assert!(
            units.iter().all(|id| matches!(
                entities.get(*id).map(|entity| entity.order()),
                Some(Order::Move(_))
            )),
            "five tanks should exceed the base command budget and keep their orders"
        );
        assert_notice(&events, 1, "Command supply exceeded");
    }

    #[test]
    fn command_car_bonus_stacks_while_consuming_supply() {
        let map = flat_map(64);

        let mut one_car_entities = EntityStore::new();
        let mut one_car_units = spawn_units(&mut one_car_entities, 1, EntityKind::Tank, 5);
        one_car_units.extend(spawn_units(
            &mut one_car_entities,
            1,
            EntityKind::CommandCar,
            1,
        ));
        mark_units_moving(&mut one_car_entities, &one_car_units);
        apply(
            &map,
            &mut one_car_entities,
            vec![(
                1,
                SimCommand::Stop {
                    units: one_car_units.clone(),
                },
            )],
        );
        assert!(
            one_car_units.iter().all(|id| matches!(
                one_car_entities.get(*id).map(|entity| entity.order()),
                Some(Order::Idle)
            )),
            "one Command Car should make five tanks legal: 34 used / 36 cap"
        );

        let mut two_car_entities = EntityStore::new();
        let mut two_car_units = spawn_units(&mut two_car_entities, 1, EntityKind::Tank, 6);
        two_car_units.extend(spawn_units(
            &mut two_car_entities,
            1,
            EntityKind::CommandCar,
            2,
        ));
        mark_units_moving(&mut two_car_entities, &two_car_units);
        apply(
            &map,
            &mut two_car_entities,
            vec![(
                1,
                SimCommand::Stop {
                    units: two_car_units.clone(),
                },
            )],
        );
        assert!(
            two_car_units.iter().all(|id| matches!(
                two_car_entities.get(*id).map(|entity| entity.order()),
                Some(Order::Idle)
            )),
            "two Command Cars should stack: 44 used / 48 cap"
        );
    }

    #[test]
    fn command_unit_id_processing_remains_absolutely_bounded() {
        let map = flat_map(64);
        let mut entities = EntityStore::new();
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, 100.0, 100.0)
            .expect("worker should spawn");
        entities
            .get_mut(worker)
            .expect("worker should exist")
            .set_order(Order::move_to(10.0, 10.0));
        let mut units = vec![worker; MAX_UNITS_PER_COMMAND + 2_000];
        units.extend(spawn_units(&mut entities, 1, EntityKind::Tank, 20));

        apply(&map, &mut entities, vec![(1, SimCommand::Stop { units })]);

        assert!(
            matches!(
                entities.get(worker).map(|entity| entity.order()),
                Some(Order::Idle)
            ),
            "only the bounded leading id window should be inspected"
        );
    }

    /// Run `apply_commands` with throwaway derived state for command-validation tests.
    fn apply(map: &Map, entities: &mut EntityStore, pending: Vec<(u32, SimCommand)>) {
        let mut players = vec![player_state(1), player_state(2)];
        let _ = apply_with_players(map, entities, &mut players, pending);
    }

    fn apply_with_players(
        map: &Map,
        entities: &mut EntityStore,
        players: &mut [PlayerState],
        pending: Vec<(u32, SimCommand)>,
    ) -> HashMap<u32, Vec<Event>> {
        let mut smokes = SmokeCloudStore::new();
        apply_with_players_and_smokes(map, entities, players, &mut smokes, pending)
    }

    fn apply_with_players_and_smokes(
        map: &Map,
        entities: &mut EntityStore,
        players: &mut [PlayerState],
        smokes: &mut SmokeCloudStore,
        pending: Vec<(u32, SimCommand)>,
    ) -> HashMap<u32, Vec<Event>> {
        let spatial = SpatialIndex::build(entities, map.size);
        let occ = Occupancy::build(map, entities);
        let mut pathing = PathingService::new(1024, 32);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, map, &occ, 1);
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2], entities, map);
        let mut events: HashMap<u32, Vec<Event>> = players
            .iter()
            .map(|player| (player.id, Vec::new()))
            .collect();
        let mut mortar_shells = MortarShellStore::default();
        let mut artillery_shells = ArtilleryShellStore::default();
        let mut ability_runtime = AbilityRuntime::new();
        apply_commands(
            map,
            entities,
            players,
            &spatial,
            &mut coordinator,
            &fog,
            smokes,
            &mut ability_runtime,
            &mut mortar_shells,
            &mut artillery_shells,
            pending,
            &mut events,
            1,
        );
        events
    }

    fn flat_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![],
            expansion_sites: vec![],
        }
    }

    fn player_state(id: u32) -> PlayerState {
        PlayerState {
            id,
            team_id: id,
            faction_id: "kriegsia".to_string(),
            name: format!("Player {id}"),
            color: "#fff".to_string(),
            start_tile: (0, 0),
            steel: 1_000,
            oil: 1_000,
            supply_used: 0,
            supply_cap: 20,
            is_ai: false,
            score: ScoreState::default(),
            upgrades: Default::default(),
        }
    }

    fn spawn_units(
        entities: &mut EntityStore,
        owner: u32,
        kind: EntityKind,
        count: usize,
    ) -> Vec<u32> {
        (0..count)
            .map(|index| {
                let x = 96.0 + (index % 8) as f32 * 32.0;
                let y = 96.0 + (index / 8) as f32 * 32.0;
                entities
                    .spawn_unit(owner, kind, x, y)
                    .expect("unit should spawn")
            })
            .collect()
    }

    fn mark_units_moving(entities: &mut EntityStore, units: &[u32]) {
        for id in units {
            entities
                .get_mut(*id)
                .expect("unit should exist")
                .set_order(Order::move_to(10.0, 10.0));
        }
    }

    fn assert_notice(events: &HashMap<u32, Vec<Event>>, player: u32, message: &str) {
        assert!(
            events
                .get(&player)
                .is_some_and(|player_events| player_events
                    .iter()
                    .any(|event| matches!(event, Event::Notice { msg, .. } if msg == message))),
            "expected notice {message:?} for player {player}: {events:?}"
        );
    }

    fn fill_queue(entities: &mut EntityStore, id: u32) {
        for _ in 0..MAX_QUEUED_ORDERS {
            entities
                .get_mut(id)
                .expect("unit should exist")
                .append_queued_order(OrderIntent::move_to(999.0, 999.0));
        }
    }
}
