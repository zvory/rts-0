use std::collections::{HashMap, HashSet};

use crate::config;
use crate::game::ability::{self, AbilityKind, AbilityTargetMode};
use crate::game::command::SimCommand;
use crate::game::entity::{
    BuildPhase, EntityKind, EntityStore, Order, OrderIntent, ProdItem, WeaponSetup,
    MAX_QUEUED_ORDERS,
};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services::ability_orders::{
    self, caster_can_attempt, launch_self_ability, order_or_launch_world_ability,
    tech_requirement_met,
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
use crate::game::PlayerState;
use crate::protocol::{Event, NoticeSeverity};
use crate::rules;

/// Max unique unit ids honored per multi-unit command. Caps the per-id work a single command can
/// force, so a repeated/huge id list can't be used to stall the tick loop.
const MAX_UNITS_PER_COMMAND: usize = 256;

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
    pending: Vec<(u32, SimCommand)>,
    events: &mut HashMap<u32, Vec<Event>>,
    tick: u32,
) {
    for (player, cmd) in pending {
        match cmd {
            SimCommand::Move {
                units,
                x,
                y,
                queued,
            } => {
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
                    fog,
                    smokes,
                    events,
                    player,
                    &planner_facts(entities, player, &units, false),
                    &request,
                );
            }
            SimCommand::AttackMove {
                units,
                x,
                y,
                queued,
            } => {
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
                    fog,
                    smokes,
                    events,
                    player,
                    &planner_facts(entities, player, &units, false),
                    &request,
                );
            }
            SimCommand::Attack {
                units,
                target,
                queued,
            } => {
                let target_valid =
                    attack_target_valid(entities, fog, smokes, player, &units, target);
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
                    fog,
                    smokes,
                    events,
                    player,
                    &planner_facts(entities, player, &units, false),
                    &request,
                );
            }
            SimCommand::SetupAtGuns {
                units,
                x,
                y,
                queued,
            } => {
                if !x.is_finite() || !y.is_finite() {
                    continue;
                }
                for id in dedupe_cap_units(units) {
                    if !owns_unit(entities, player, id) || is_constructing(entities, id) {
                        continue;
                    }
                    if !matches!(entities.get(id), Some(e) if e.kind == EntityKind::AtTeam) {
                        continue;
                    }
                    if queued {
                        append_queued_or_notice(
                            entities,
                            events,
                            player,
                            id,
                            OrderIntent::setup_at_guns(x, y),
                        );
                        continue;
                    }
                    execute_at_gun_setup(entities, id, x, y);
                }
            }
            SimCommand::TearDownAtGuns { units } => {
                for id in dedupe_cap_units(units) {
                    if !owns_unit(entities, player, id) || is_constructing(entities, id) {
                        continue;
                    }
                    let Some(e) = entities.get_mut(id) else {
                        continue;
                    };
                    if e.kind != EntityKind::AtTeam {
                        continue;
                    }
                    if matches!(
                        e.weapon_setup(),
                        WeaponSetup::SettingUp { .. } | WeaponSetup::Deployed
                    ) {
                        e.clear_orders();
                        e.set_path_goal(None);
                        e.set_weapon_setup(WeaponSetup::TearingDown {
                            ticks: config::AT_TEAM_SETUP_TICKS,
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
                use_ability(
                    map,
                    entities,
                    players,
                    coordinator,
                    smokes,
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
            SimCommand::Gather {
                units,
                node,
                queued,
            } => {
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
                    fog,
                    smokes,
                    events,
                    player,
                    &planner_facts(entities, player, &units, false),
                    &request,
                );
            }
            SimCommand::Build {
                worker,
                building,
                tile_x,
                tile_y,
                queued,
            } => {
                let request = planner::OrderRequest {
                    units: vec![worker],
                    mode: issue_mode(queued),
                    order: planner::RequestedOrder::Build {
                        kind: build_kind_code(building),
                        tile_x,
                        tile_y,
                        placement_valid: true,
                    },
                };
                apply_planned_unit_order(
                    map,
                    entities,
                    players,
                    spatial,
                    coordinator,
                    fog,
                    smokes,
                    events,
                    player,
                    &planner_facts(entities, player, &[worker], !queued),
                    &request,
                );
            }
            SimCommand::Train { building, unit } => {
                order_train(entities, players, player, building, unit, events);
            }
            SimCommand::Cancel { building } => {
                order_cancel(entities, players, player, building);
            }
            SimCommand::Stop { units } => {
                for id in dedupe_cap_units(units) {
                    if owns_unit(entities, player, id) && !is_constructing(entities, id) {
                        entities.release_miner(id);
                        if let Some(e) = entities.get_mut(id) {
                            e.clear_orders();
                            if let Some(w) = e.worker.as_mut() {
                                w.carry = None;
                            }
                        }
                    }
                }
            }
            SimCommand::SetRally {
                building,
                x,
                y,
                queued,
            } => {
                order_set_rally(map, entities, player, building, x, y, queued);
            }
            SimCommand::Rejected { reason } => {
                notice(events, player, reason.notice_message());
            }
        }
    }
}

/// Dedupe a command's unit ids (preserving first-seen order) and cap the count at
/// `MAX_UNITS_PER_COMMAND`.
fn dedupe_cap_units(units: Vec<u32>) -> Vec<u32> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(units.len().min(MAX_UNITS_PER_COMMAND));
    for id in units {
        if out.len() >= MAX_UNITS_PER_COMMAND {
            break;
        }
        if seen.insert(id) {
            out.push(id);
        }
    }
    out
}

fn issue_mode(queued: bool) -> planner::IssueMode {
    if queued {
        planner::IssueMode::Queue
    } else {
        planner::IssueMode::Immediate
    }
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
    units: &[u32],
    build_notice_compat: bool,
) -> Vec<planner::UnitFacts> {
    dedupe_cap_units(units.to_vec())
        .into_iter()
        .filter_map(|id| {
            let e = entities.get(id)?;
            if !e.is_unit() || e.owner != player {
                return None;
            }
            let mut facts = planner::UnitFacts::new(id);
            facts.queue_len = e.queued_orders().len();
            facts.activity = match e.order() {
                Order::Idle => planner::UnitActivity::Idle,
                Order::Move(_) | Order::AttackMove(_) | Order::Ability(_) => {
                    planner::UnitActivity::Moving
                }
                _ => planner::UnitActivity::Busy,
            };
            facts.can_attack = e.can_attack();
            facts.can_gather = e.kind == EntityKind::Worker;
            facts.can_build = e.kind == EntityKind::Worker || build_notice_compat;
            facts.can_setup_at_gun = e.kind == EntityKind::AtTeam;
            Some(facts)
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn apply_planned_unit_order(
    map: &Map,
    entities: &mut EntityStore,
    players: &[PlayerState],
    spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    facts: &[planner::UnitFacts],
    request: &planner::OrderRequest,
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
                        && attack_unit_can_target(entities, fog, smokes, player, unit, target)
                        && !deployed_at_gun_target_outside_arc(entities, unit, target)
                    {
                        if let Some(e) = entities.get_mut(unit) {
                            e.clear_queued_orders();
                        }
                        clear_staged_at_gun_setup(entities, &[unit]);
                        coordinator.order_attack(entities, unit, target);
                    }
                }
                planner::OrderIntent::Gather(node) => {
                    if gather_unit_can_use_node(entities, player, unit, node) {
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
                planner::OrderIntent::SetupAtGuns { .. }
                | planner::OrderIntent::WorldAbility { .. }
                | planner::OrderIntent::SelfAbility { .. } => {}
            },
            planner::PlannedAction::AppendQueued { unit, intent } => {
                if let Some(intent) = entity_order_intent_from_planner(intent) {
                    if matches!(intent, OrderIntent::Attack(_))
                        && !matches!(
                            intent,
                            OrderIntent::Attack(attack)
                                if attack_unit_can_target(
                                    entities,
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
            planner::PlannedAction::ExecuteAbilityNow { .. } => {}
        }
    }

    if let Some(goal) = move_goal {
        clear_queued_orders(entities, &move_units);
        clear_staged_at_gun_setup(entities, &move_units);
        coordinator.order_group_move(entities, player, &move_units, goal, false);
    }
    if let Some(goal) = attack_move_goal {
        clear_queued_orders(entities, &attack_move_units);
        clear_staged_at_gun_setup(entities, &attack_move_units);
        coordinator.order_group_move(entities, player, &attack_move_units, goal, true);
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
    fog: &Fog,
    smokes: &SmokeCloudStore,
    player: u32,
    units: &[u32],
    target: u32,
) -> bool {
    dedupe_cap_units(units.to_vec())
        .into_iter()
        .any(|unit| attack_unit_can_target(entities, fog, smokes, player, unit, target))
}

fn attack_unit_can_target(
    entities: &EntityStore,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    player: u32,
    unit: u32,
    target: u32,
) -> bool {
    matches!(entities.get(target),
        Some(t) if world_query::is_enemy_targetable(t, player, unit)
            && fog.is_visible_world(player, t.pos_x, t.pos_y)
            && !smokes.point_inside(t.pos_x, t.pos_y))
}

fn gather_node_valid(entities: &EntityStore, player: u32, node: u32) -> bool {
    matches!(entities.get(node), Some(n) if n.is_node() && n.remaining().unwrap_or(0) > 0)
        && world_query::resource_has_completed_mining_cc(entities, player, node)
}

fn gather_unit_can_use_node(entities: &EntityStore, player: u32, unit: u32, node: u32) -> bool {
    owns_unit(entities, player, unit)
        && matches!(entities.get(unit), Some(e) if e.kind == EntityKind::Worker)
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
        planner::OrderIntent::SelfAbility { .. } | planner::OrderIntent::SetupAtGuns { .. } => None,
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

fn ability_from_planner(ability: planner::AbilityId) -> Option<AbilityKind> {
    match ability.0 {
        0 => Some(AbilityKind::Charge),
        1 => Some(AbilityKind::Smoke),
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
    coordinator: &mut MoveCoordinator<'_>,
    smokes: &mut SmokeCloudStore,
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
    match definition.target_mode {
        AbilityTargetMode::SelfTarget => {}
        AbilityTargetMode::WorldPoint => {
            let Some(x) = request.x else {
                return;
            };
            let Some(y) = request.y else {
                return;
            };
            if !x.is_finite() || !y.is_finite() {
                return;
            }
        }
    }
    match ability {
        AbilityKind::Charge => {
            if !tech_requirement_met(entities, player, ability) {
                return;
            }
            for id in dedupe_cap_units(request.units) {
                if !caster_can_attempt(entities, player, id, ability) {
                    continue;
                }
                if request.queued {
                    append_queued_or_notice(
                        entities,
                        events,
                        player,
                        id,
                        OrderIntent::self_ability(ability),
                    );
                    continue;
                }
                launch_self_ability(entities, player, id, ability);
            }
        }
        AbilityKind::Smoke => {
            let Some(x) = request.x else {
                return;
            };
            let Some(y) = request.y else {
                return;
            };
            let Some((x, y)) = SmokeCloudStore::clamp_point_to_map(map, x, y) else {
                return;
            };
            if !tech_requirement_met(entities, player, ability) {
                return;
            }
            let eligible: Vec<u32> = dedupe_cap_units(request.units)
                .into_iter()
                .filter(|id| caster_can_attempt(entities, player, *id, ability))
                .collect();
            if eligible.is_empty() {
                return;
            }
            if request.queued {
                if let Some(caster) = choose_queued_smoke_caster(entities, &eligible) {
                    append_queued_or_notice(
                        entities,
                        events,
                        player,
                        caster,
                        OrderIntent::ability(ability, x, y),
                    );
                } else {
                    for id in eligible {
                        notice(events, player, queue_full_notice(id));
                    }
                }
                return;
            }

            let Some(caster) = choose_smoke_caster(map, entities, ability, &eligible, x, y) else {
                return;
            };
            if let Some(e) = entities.get_mut(caster) {
                e.clear_queued_orders();
            }
            order_or_launch_world_ability(
                map,
                entities,
                players,
                coordinator,
                smokes,
                events,
                player,
                caster,
                ability,
                x,
                y,
                tick,
                true,
            );
        }
    }
}

fn choose_queued_smoke_caster(entities: &EntityStore, eligible: &[u32]) -> Option<u32> {
    eligible
        .iter()
        .copied()
        .filter_map(|id| entities.get(id).map(|e| (id, e.queued_orders().len())))
        .filter(|(_, len)| *len < crate::game::entity::MAX_QUEUED_ORDERS)
        .min_by_key(|(_, len)| *len)
        .map(|(id, _)| id)
}

fn append_queued_or_notice(
    entities: &mut EntityStore,
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    unit: u32,
    intent: OrderIntent,
) -> bool {
    let appended = entities
        .get_mut(unit)
        .is_some_and(|e| e.append_queued_order(intent));
    if !appended {
        notice(events, player, queue_full_notice(unit));
    }
    appended
}

fn queue_full_notice(_unit: u32) -> &'static str {
    "Order queue full"
}

fn execute_at_gun_setup(entities: &mut EntityStore, id: u32, x: f32, y: f32) -> bool {
    let Some(e) = entities.get(id) else {
        return false;
    };
    if e.kind != EntityKind::AtTeam || e.under_construction() || !x.is_finite() || !y.is_finite() {
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
            ticks: config::AT_TEAM_SETUP_TICKS,
        });
    }
    e.reset_gather_state();
    let (px, py) = (e.pos_x, e.pos_y);
    e.reset_stuck(px, py);
    true
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

fn clear_staged_at_gun_setup(entities: &mut EntityStore, ids: &[u32]) {
    for id in ids {
        let Some(e) = entities.get_mut(*id) else {
            continue;
        };
        if e.kind == EntityKind::AtTeam {
            e.set_emplacement_facing(None);
            e.set_pending_redeploy_facing(None);
        }
    }
}

fn deployed_at_gun_target_outside_arc(entities: &EntityStore, id: u32, target: u32) -> bool {
    let Some(attacker) = entities.get(id) else {
        return false;
    };
    if attacker.kind != EntityKind::AtTeam
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
    angle_delta(center, target_angle).abs() > config::AT_GUN_FIELD_OF_FIRE_RAD * 0.5
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
    if !matches!(entities.get(worker), Some(e) if e.kind == EntityKind::Worker) {
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
    if !rules::economy::build_requirement_met(building, &owned) {
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
    let (cost_steel, cost_oil) = rules::economy::cost(building);
    if ps.steel < cost_steel || ps.oil < cost_oil {
        notice(
            events,
            player,
            rules::economy::resource_shortage_notice(ps.steel, ps.oil, cost_steel, cost_oil),
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
    let ok = matches!(entities.get(building), Some(b)
        if b.owner == player && b.is_building() && !b.under_construction()
        && rules::economy::trainable_units(b.kind).contains(&unit));
    if !ok {
        notice(events, player, "Cannot train that here");
        return;
    }
    let owned_complete = world_query::completed_building_kinds(entities, player);
    if !rules::economy::train_requirement_met(unit, &owned_complete) {
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
    let (cost_steel, cost_oil) = rules::economy::cost(unit);
    let supply = rules::economy::supply_cost(unit);
    if ps.steel < cost_steel || ps.oil < cost_oil {
        notice(
            events,
            player,
            rules::economy::resource_shortage_notice(ps.steel, ps.oil, cost_steel, cost_oil),
        );
        return;
    }
    if ps.supply_used + supply > ps.supply_cap {
        notice(events, player, "Not enough supply");
        return;
    }
    ps.steel -= cost_steel;
    ps.oil -= cost_oil;
    ps.supply_used += supply;

    if let Some(b) = entities.get_mut(building) {
        if let Some(queue) = b.prod_queue_mut() {
            queue.push(ProdItem {
                unit,
                progress: 0,
                total: stats.build_ticks,
            });
        }
    }
}

/// Set a unit-producing building's rally point. Validates ownership and that the building is a
/// completed producer; sanitizes/clamps the point to the map. Invalid requests are ignored
/// silently (consistent with movement commands), so a hostile client cannot wedge the tick loop.
fn order_set_rally(
    map: &Map,
    entities: &mut EntityStore,
    player: u32,
    building: u32,
    x: f32,
    y: f32,
    _queued: bool,
) {
    let ok = matches!(entities.get(building), Some(b)
        if b.owner == player && b.is_building() && !b.under_construction()
        && !rules::economy::trainable_units(b.kind).is_empty());
    if !ok {
        return;
    }
    if !x.is_finite() || !y.is_finite() {
        return;
    }
    let max = (map.world_size_px() - 1.0).max(0.0);
    let rally = (x.clamp(0.0, max), y.clamp(0.0, max));
    if let Some(b) = entities.get_mut(building) {
        b.clear_rally_stages();
        b.set_rally_point(Some(rally));
    }
}

/// Cancel the front item of a building's production queue, refunding its cost + supply.
fn order_cancel(
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    player: u32,
    building: u32,
) {
    let unit = {
        let b = match entities.get_mut(building) {
            Some(b) if b.owner == player && b.is_building() && !b.prod_queue().is_empty() => b,
            _ => return,
        };
        match b.prod_queue_mut() {
            Some(queue) => queue.remove(0).unit,
            None => return,
        }
    };
    if config::unit_stats(unit).is_some() {
        if let Some(ps) = players.iter_mut().find(|p| p.id == player) {
            let (cost_steel, cost_oil) = rules::economy::cost(unit);
            ps.steel += cost_steel;
            ps.oil += cost_oil;
            ps.supply_used = ps
                .supply_used
                .saturating_sub(rules::economy::supply_cost(unit));
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
    use crate::game::entity::{EntityKind, EntityStore, Order, OrderIntent, WeaponSetup};
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
        let mut events = HashMap::new();

        apply_commands(
            &map,
            &mut entities,
            &mut players,
            &spatial,
            &mut coordinator,
            &fog,
            &mut smokes,
            vec![(
                1,
                SimCommand::Build {
                    worker,
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
        let mut events = HashMap::new();

        apply_commands(
            &map,
            &mut entities,
            &mut players,
            &spatial,
            &mut coordinator,
            &fog,
            &mut smokes,
            vec![(
                1,
                SimCommand::Build {
                    worker,
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
        let mut events = HashMap::new();

        apply_commands(
            &map,
            &mut entities,
            &mut players,
            &spatial,
            &mut coordinator,
            &fog,
            &mut smokes,
            vec![(
                1,
                SimCommand::Build {
                    worker,
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
                    worker,
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
                        worker: builder,
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
    fn charge_requires_training_centre_and_filters_to_owned_riflemen() {
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
            config::RIFLEMAN_CHARGE_TICKS
        );
        assert_eq!(
            entities.get(rifle).unwrap().charge_cooldown_ticks(),
            config::RIFLEMAN_CHARGE_COOLDOWN_TICKS
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
    fn charge_respects_cooldown_before_allowing_reuse() {
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
        let first_cooldown_ticks = entities.get(rifle).unwrap().charge_cooldown_ticks();

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
            entities.get(rifle).unwrap().charge_cooldown_ticks(),
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
        assert_eq!(
            entities.get(rifle).unwrap().charge_ticks(),
            config::RIFLEMAN_CHARGE_TICKS,
            "charge should become available again after cooldown expiry"
        );
    }

    #[test]
    fn queued_charge_appends_to_ready_riflemen_only_and_later_attack_move_hits_selection() {
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

        assert!(matches!(
            entities.get(ready).unwrap().queued_orders()[0],
            OrderIntent::SelfAbility(_)
        ));
        assert_eq!(entities.get(ready).unwrap().queued_orders().len(), 2);
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
        // A positioned info notice is emitted on successful smoke launch; no warn/alert events.
        let player_events = events.get(&1).map(Vec::as_slice).unwrap_or(&[]);
        assert!(
            player_events.iter().all(|ev| matches!(
                ev,
                Event::Notice {
                    severity: crate::protocol::NoticeSeverity::Info,
                    ..
                }
            )),
            "smoke launch should emit at most info-level notices, got: {player_events:?}"
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
                    }
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
                        worker,
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
    fn queued_rally_replaces_the_single_rally_point() {
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
                        queued: true,
                    },
                ),
                (
                    1,
                    SimCommand::SetRally {
                        building: barracks,
                        x: 200.0,
                        y: 200.0,
                        queued: true,
                    },
                ),
                (
                    1,
                    SimCommand::SetRally {
                        building: barracks,
                        x: 300.0,
                        y: 300.0,
                        queued: true,
                    },
                ),
            ],
        );

        assert_eq!(
            entities.get(barracks).unwrap().rally_point(),
            Some((300.0, 300.0)),
            "queued rally commands should still replace the one active rally point"
        );
        assert!(entities.get(barracks).unwrap().rally_stages().is_empty());

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::SetRally {
                    building: barracks,
                    x: 400.0,
                    y: 400.0,
                    queued: false,
                },
            )],
        );

        let barracks = entities.get(barracks).expect("barracks should exist");
        assert!(barracks.rally_stages().is_empty());
        assert_eq!(barracks.rally_point(), Some((400.0, 400.0)));
    }

    #[test]
    fn setup_at_guns_filters_mixed_selection_and_records_facing() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let at = entities
            .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
            .expect("at gun should spawn");
        let rifle = entities
            .spawn_unit(1, EntityKind::Rifleman, 120.0, 100.0)
            .expect("rifleman should spawn");
        let enemy_at = entities
            .spawn_unit(2, EntityKind::AtTeam, 140.0, 100.0)
            .expect("enemy at gun should spawn");

        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::SetupAtGuns {
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
            "setup command should not snap the AT gun body to the target facing"
        );
        assert_eq!(
            entities
                .get(rifle)
                .expect("rifleman should exist")
                .weapon_setup(),
            WeaponSetup::Packed,
            "non-AT units in the selected list are ignored"
        );
        assert_eq!(
            entities
                .get(enemy_at)
                .expect("enemy at gun should exist")
                .weapon_setup(),
            WeaponSetup::Packed,
            "enemy AT guns are ignored"
        );
    }

    #[test]
    fn queued_setup_at_guns_filters_to_at_teams_and_preserves_later_attack_move() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let at = entities
            .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
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
                    SimCommand::SetupAtGuns {
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
            OrderIntent::SetupAtGuns(_)
        ));
        assert_eq!(entities.get(at).unwrap().queued_orders().len(), 2);
        assert_eq!(
            entities.get(rifle).unwrap().queued_orders().len(),
            1,
            "non-AT units skip setup but keep later compatible stages"
        );
    }

    #[test]
    fn teardown_at_guns_only_affects_setting_up_or_deployed_at_guns() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let deployed = entities
            .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
            .expect("at gun should spawn");
        let packed = entities
            .spawn_unit(1, EntityKind::AtTeam, 130.0, 100.0)
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
                SimCommand::TearDownAtGuns {
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
            "teardown should cancel a packed AT gun's staged setup facing"
        );
    }

    #[test]
    fn move_order_tears_down_deployed_at_guns_before_moving() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let deployed = entities
            .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
            .expect("at gun should spawn");
        let packed = entities
            .spawn_unit(1, EntityKind::AtTeam, 130.0, 100.0)
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
            "move order should not instantly rotate a deployed AT gun before it moves"
        );
        assert!(
            matches!(deployed.order(), Order::Move(_)),
            "move should replace the deployed AT gun order"
        );
        assert!(
            deployed.path_goal().is_some(),
            "move should preserve the movement destination while the AT gun tears down"
        );
        assert_eq!(deployed.emplacement_facing(), None);
        assert_eq!(deployed.pending_redeploy_facing(), None);

        let packed = entities.get(packed).expect("packed at gun should exist");
        assert!(
            matches!(packed.order(), Order::Move(_)),
            "packed AT guns should still accept move orders"
        );
    }

    #[test]
    fn attack_move_order_tears_down_deployed_at_guns_before_moving() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let deployed = entities
            .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
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
            "attack-move should not instantly rotate a deployed AT gun before it moves"
        );
        assert!(
            matches!(deployed.order(), Order::AttackMove(_)),
            "attack-move should replace the deployed AT gun order"
        );
        assert!(
            deployed.path_goal().is_some(),
            "attack-move should preserve the movement destination while the AT gun tears down"
        );
        assert_eq!(deployed.emplacement_facing(), None);
        assert_eq!(deployed.pending_redeploy_facing(), None);
    }

    #[test]
    fn deployed_at_gun_rejects_explicit_attack_outside_field_of_fire() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let at = entities
            .spawn_unit(1, EntityKind::AtTeam, 100.0, 100.0)
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
            "out-of-arc attack should be ignored for the deployed AT gun"
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
        let mut events = HashMap::new();
        apply_commands(
            map,
            entities,
            players,
            &spatial,
            &mut coordinator,
            &fog,
            smokes,
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
            name: format!("Player {id}"),
            color: "#fff".to_string(),
            start_tile: (0, 0),
            steel: 1_000,
            oil: 1_000,
            supply_used: 0,
            supply_cap: 20,
            is_ai: false,
            score: ScoreState::default(),
        }
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
