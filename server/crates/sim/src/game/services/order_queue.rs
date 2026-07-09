use crate::config;
use crate::game::ability::{self, AbilityKind, AbilityQueuePolicy};
use crate::game::ability_runtime::AbilityRuntime;
use crate::game::entity::{
    BuildPhase, Entity, EntityKind, EntityStore, MovePhase, Order, OrderIntent, MAX_QUEUED_ORDERS,
};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::mortar::MortarShellStore;
use crate::game::services::ability_orders::{
    active_ability_order_ready, caster_can_accept_waiting_order, caster_can_attempt,
    caster_can_promote_queued_world_ability, launch_self_ability, launch_world_ability,
    order_or_launch_world_ability, world_ability_facing_ready,
};
use crate::game::services::construction::resumable_site_for_build_intent;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::movement::angle_delta;
use crate::game::services::order_execution::targeting::{
    stored_artillery_point_fire_target, ArtilleryPointFireAcceptance,
};
use crate::game::services::order_execution::{
    begin_artillery_teardown_for_movement, execute_anti_tank_gun_setup,
    start_artillery_fire_promoted_order, ArtilleryFireMode, FutureOrderMode,
};
use crate::game::services::standability;
use crate::game::services::world_query;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::game::PlayerState;
use crate::protocol::{Event, NoticeSeverity};
use crate::rules;
use std::collections::BTreeMap;

use self::attack::{attack_can_fire_now, panzerfaust_attack_cycle_active};

mod attack;

const ATTACK_UNREACHABLE_PROMOTION_CHECKS: u16 = 3;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PointPromotionKey {
    owner: u32,
    attack_move: bool,
    x_bits: u32,
    y_bits: u32,
}

impl PointPromotionKey {
    fn new(owner: u32, attack_move: bool, x: f32, y: f32) -> Option<Self> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        Some(PointPromotionKey {
            owner,
            attack_move,
            x_bits: x.to_bits(),
            y_bits: y.to_bits(),
        })
    }

    fn point(self) -> (f32, f32) {
        (f32::from_bits(self.x_bits), f32::from_bits(self.y_bits))
    }
}
/// Outcome of popping the next queued intent for a unit. Move/AttackMove are batched into a
/// group move per destination point; gather/build are issued directly per worker.
enum PromotedIntent {
    PointMove(PointPromotionKey),
    Attack {
        target: u32,
    },
    Gather {
        node: u32,
    },
    Build {
        kind: EntityKind,
        tx: u32,
        ty: u32,
    },
    Deconstruct {
        target: u32,
    },
    WorldAbility {
        ability: crate::game::ability::AbilityKind,
        x: f32,
        y: f32,
    },
    SelfAbility {
        ability: crate::game::ability::AbilityKind,
    },
    SetupAntiTankGuns {
        x: f32,
        y: f32,
    },
    PointFire {
        x: f32,
        y: f32,
    },
    BlanketFire {
        x: f32,
        y: f32,
    },
}

/// Promote completed orders into the next queued intent.
///
/// Move/AttackMove intents are batched by destination so co-arriving units share a formation,
/// while Attack, Gather, and Build intents are issued directly per unit.
#[allow(clippy::too_many_arguments)]
pub(crate) fn promote_ready_orders(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    fog: &Fog,
    coordinator: &mut MoveCoordinator<'_>,
    smokes: &mut SmokeCloudStore,
    ability_runtime: &mut AbilityRuntime,
    mortar_shells: &mut MortarShellStore,
    events: &mut std::collections::HashMap<u32, Vec<Event>>,
    tick: u32,
) {
    let teams = TeamRelations::from_player_teams(players.iter().map(|p| (p.id, p.team_id)));
    let ready: Vec<u32> = entities
        .iter()
        .filter(|e| ready_for_next_order(map, entities, &teams, fog, smokes, e))
        .map(|e| e.id)
        .collect();
    if ready.is_empty() {
        return;
    }

    let mut groups: BTreeMap<PointPromotionKey, Vec<u32>> = BTreeMap::new();
    for id in ready {
        if let Some((_ability, _x, _y, MovePhase::PathFailed)) = entities
            .get(id)
            .and_then(|e| active_ability_order_ready(&e.order()))
        {
            clear_completed_active_order(entities, id);
        } else if let Some((ability, x, y, MovePhase::Arrived)) = entities
            .get(id)
            .and_then(|e| active_ability_order_ready(&e.order()))
        {
            let owner = match entities.get(id) {
                Some(e) => e.owner,
                None => continue,
            };
            if !world_ability_facing_ready(entities, id, ability, x, y) {
                continue;
            }
            if waits_for_readiness(entities, owner, id, ability) {
                continue;
            }
            let faction_id = players
                .iter()
                .find(|p| p.id == owner)
                .map(|p| p.faction_id.as_str())
                .unwrap_or(crate::rules::faction::DEFAULT_FACTION_ID)
                .to_string();
            let launched = launch_world_ability(
                map,
                entities,
                players,
                fog,
                &teams,
                smokes,
                ability_runtime,
                mortar_shells,
                events,
                owner,
                &faction_id,
                id,
                ability,
                x,
                y,
                tick,
                false,
                true,
            );
            if !launched {
                clear_completed_active_order(entities, id);
            }
        } else {
            clear_completed_active_order(entities, id);
        }

        let Some(promoted) =
            pop_next_valid_intent(map, entities, players, &teams, fog, smokes, events, id)
        else {
            continue;
        };
        match promoted {
            PromotedIntent::PointMove(key) => {
                groups.entry(key).or_default().push(id);
            }
            PromotedIntent::Attack { target } => {
                coordinator.order_attack(entities, id, target);
            }
            PromotedIntent::Gather { node } => {
                coordinator.order_gather(entities, id, node);
            }
            PromotedIntent::Build { kind, tx, ty } => {
                coordinator.order_build(entities, id, kind, tx, ty);
            }
            PromotedIntent::Deconstruct { target } => {
                coordinator.order_deconstruct(entities, id, target);
            }
            PromotedIntent::WorldAbility { ability, x, y } => {
                let Some(owner) = entities.get(id).map(|e| e.owner) else {
                    continue;
                };
                if ability == AbilityKind::PointFire {
                    execute_artillery_fire(map, entities, id, x, y, ArtilleryFireMode::Point);
                    continue;
                }
                if ability == AbilityKind::BlanketFire {
                    execute_artillery_fire(map, entities, id, x, y, ArtilleryFireMode::Blanket);
                    continue;
                }
                let faction_id = players
                    .iter()
                    .find(|p| p.id == owner)
                    .map(|p| p.faction_id.as_str())
                    .unwrap_or(crate::rules::faction::DEFAULT_FACTION_ID)
                    .to_string();
                order_or_launch_world_ability(
                    map,
                    entities,
                    players,
                    fog,
                    &teams,
                    coordinator,
                    smokes,
                    ability_runtime,
                    mortar_shells,
                    events,
                    owner,
                    &faction_id,
                    id,
                    ability,
                    x,
                    y,
                    tick,
                    true,
                );
            }
            PromotedIntent::SelfAbility { ability } => {
                let Some(owner) = entities.get(id).map(|e| e.owner) else {
                    continue;
                };
                let faction_id = players
                    .iter()
                    .find(|p| p.id == owner)
                    .map(|p| p.faction_id.as_str())
                    .unwrap_or(crate::rules::faction::DEFAULT_FACTION_ID);
                launch_self_ability(entities, events, faction_id, owner, id, ability);
            }
            PromotedIntent::SetupAntiTankGuns { x, y } => {
                execute_anti_tank_gun_setup(entities, id, x, y, FutureOrderMode::Preserve);
            }
            PromotedIntent::PointFire { x, y } => {
                execute_artillery_fire(map, entities, id, x, y, ArtilleryFireMode::Point);
            }
            PromotedIntent::BlanketFire { x, y } => {
                execute_artillery_fire(map, entities, id, x, y, ArtilleryFireMode::Blanket);
            }
        }
    }

    for (key, ids) in groups {
        coordinator.order_group_move(entities, key.owner, &ids, key.point(), key.attack_move);
        begin_artillery_teardown_for_movement(entities, &ids);
    }
}
fn ready_for_next_order(
    map: &Map,
    entities: &EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    e: &Entity,
) -> bool {
    if !e.is_unit() || e.kind == EntityKind::ScoutPlane {
        return false;
    }
    match e.order() {
        Order::Idle | Order::HoldPosition => !e.queued_orders().is_empty() && e.path_is_empty(),
        Order::Move(_) | Order::AttackMove(_) => {
            !e.queued_orders().is_empty()
                && e.path_is_empty()
                && matches!(
                    e.move_phase(),
                    Some(MovePhase::Arrived | MovePhase::PathFailed)
                )
        }
        Order::Attack(order) => {
            !e.queued_orders().is_empty()
                && attack_order_complete(map, entities, teams, fog, smokes, e, order.intent.target)
        }
        Order::Gather(_)
        | Order::Build(_)
        | Order::Deconstruct(_)
        | Order::ArtilleryPointFire(_)
        | Order::ArtilleryBlanketFire(_) => false,
        Order::Ability(_) => matches!(
            e.move_phase(),
            Some(MovePhase::Arrived | MovePhase::PathFailed)
        ),
    }
}

fn waits_for_readiness(entities: &EntityStore, owner: u32, id: u32, ability: AbilityKind) -> bool {
    ability::definition(ability).queue_policy == AbilityQueuePolicy::QueueWaitUntilReady
        && caster_can_accept_waiting_order(entities, owner, id, ability)
        && !caster_can_attempt(entities, owner, id, ability)
}

fn clear_completed_active_order(entities: &mut EntityStore, id: u32) {
    if let Some(e) = entities.get_mut(id) {
        e.clear_active_order();
    }
}

#[allow(clippy::too_many_arguments)]
fn pop_next_valid_intent(
    map: &Map,
    entities: &mut EntityStore,
    players: &[PlayerState],
    teams: &TeamRelations,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    events: &mut std::collections::HashMap<u32, Vec<Event>>,
    id: u32,
) -> Option<PromotedIntent> {
    let owner = entities.get(id)?.owner;
    for _ in 0..MAX_QUEUED_ORDERS {
        let intent = entities.get_mut(id)?.pop_promoted_intent()?;
        match intent {
            OrderIntent::Move(point) => {
                if let Some(key) = PointPromotionKey::new(owner, false, point.x, point.y) {
                    return Some(PromotedIntent::PointMove(key));
                }
            }
            OrderIntent::AttackMove(point) => {
                if let Some(key) = PointPromotionKey::new(owner, true, point.x, point.y) {
                    return Some(PromotedIntent::PointMove(key));
                }
            }
            OrderIntent::Gather(gather) => {
                if gather_intent_valid(entities, owner, id, gather.node) {
                    return Some(PromotedIntent::Gather { node: gather.node });
                }
            }
            OrderIntent::Build(build) => {
                if let Some(msg) = build_intent_promotion_error(
                    map,
                    entities,
                    players,
                    owner,
                    id,
                    build.kind,
                    build.tile_x,
                    build.tile_y,
                ) {
                    if !msg.is_empty() {
                        events.entry(owner).or_default().push(Event::Notice {
                            msg,
                            x: None,
                            y: None,
                            severity: NoticeSeverity::Info,
                        });
                    }
                } else {
                    return Some(PromotedIntent::Build {
                        kind: build.kind,
                        tx: build.tile_x,
                        ty: build.tile_y,
                    });
                }
            }
            OrderIntent::Deconstruct(intent) => {
                if deconstruct_intent_valid(entities, teams, fog, owner, id, intent.target) {
                    return Some(PromotedIntent::Deconstruct {
                        target: intent.target,
                    });
                }
            }
            OrderIntent::Attack(attack) => {
                if attack_intent_valid(entities, teams, fog, Some(smokes), owner, id, attack.target)
                {
                    return Some(PromotedIntent::Attack {
                        target: attack.target,
                    });
                }
            }
            OrderIntent::WorldAbility(ability) => {
                if world_ability_intent_valid(
                    map,
                    entities,
                    players,
                    owner,
                    id,
                    ability.ability,
                    (ability.x, ability.y),
                ) {
                    return Some(PromotedIntent::WorldAbility {
                        ability: ability.ability,
                        x: ability.x,
                        y: ability.y,
                    });
                }
            }
            OrderIntent::PointFire(point) => {
                if artillery_point_fire_intent_valid(map, entities, owner, id, point.x, point.y) {
                    return Some(PromotedIntent::PointFire {
                        x: point.x,
                        y: point.y,
                    });
                }
            }
            OrderIntent::BlanketFire(point) => {
                if artillery_point_fire_intent_valid(map, entities, owner, id, point.x, point.y) {
                    return Some(PromotedIntent::BlanketFire {
                        x: point.x,
                        y: point.y,
                    });
                }
            }
            OrderIntent::SelfAbility(ability) => {
                if self_ability_intent_valid(entities, owner, id, ability.ability) {
                    return Some(PromotedIntent::SelfAbility {
                        ability: ability.ability,
                    });
                }
            }
            OrderIntent::SetupAntiTankGuns(point) => {
                if setup_anti_tank_gun_intent_valid(entities, id, point.x, point.y) {
                    return Some(PromotedIntent::SetupAntiTankGuns {
                        x: point.x,
                        y: point.y,
                    });
                }
            }
        }
    }
    None
}

fn artillery_point_fire_intent_valid(
    map: &Map,
    entities: &EntityStore,
    owner: u32,
    id: u32,
    x: f32,
    y: f32,
) -> bool {
    if x < 0.0 || y < 0.0 || x >= map.world_size_px() || y >= map.world_size_px() {
        return false;
    }
    stored_artillery_point_fire_target(
        map,
        entities,
        owner,
        id,
        x,
        y,
        ArtilleryPointFireAcceptance::Command,
    )
    .is_some()
}

fn execute_artillery_fire(
    map: &Map,
    entities: &mut EntityStore,
    id: u32,
    x: f32,
    y: f32,
    mode: ArtilleryFireMode,
) -> bool {
    let Some(owner) = entities.get(id).map(|e| e.owner) else {
        return false;
    };
    let Some(target) = stored_artillery_point_fire_target(
        map,
        entities,
        owner,
        id,
        x,
        y,
        ArtilleryPointFireAcceptance::Command,
    ) else {
        return false;
    };
    start_artillery_fire_promoted_order(entities, id, target, mode)
}

fn world_ability_intent_valid(
    map: &Map,
    entities: &EntityStore,
    players: &[PlayerState],
    owner: u32,
    caster: u32,
    ability: crate::game::ability::AbilityKind,
    target: (f32, f32),
) -> bool {
    let (x, y) = target;
    if SmokeCloudStore::clamp_point_to_map(map, x, y).is_none() {
        return false;
    }
    if !caster_can_promote_queued_world_ability(entities, owner, caster, ability)
        || !crate::game::services::ability_orders::tech_requirement_met(entities, owner, ability)
    {
        return false;
    }
    let definition = crate::game::ability::definition(ability);
    let Some(ps) = players.iter().find(|p| p.id == owner) else {
        return false;
    };
    ps.steel >= definition.cost.steel && ps.oil >= definition.cost.oil
}

fn self_ability_intent_valid(
    entities: &EntityStore,
    owner: u32,
    caster: u32,
    ability: crate::game::ability::AbilityKind,
) -> bool {
    crate::game::services::ability_orders::caster_can_attempt(entities, owner, caster, ability)
        && crate::game::services::ability_orders::tech_requirement_met(entities, owner, ability)
        && crate::game::ability::definition(ability).target_mode
            == crate::game::ability::AbilityTargetMode::SelfTarget
}

fn setup_anti_tank_gun_intent_valid(entities: &EntityStore, id: u32, x: f32, y: f32) -> bool {
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
    facing.is_finite()
}

fn attack_intent_valid(
    entities: &EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    smokes: Option<&SmokeCloudStore>,
    owner: u32,
    attacker: u32,
    target: u32,
) -> bool {
    let Some(unit) = entities.get(attacker) else {
        return false;
    };
    if unit.owner != owner || !unit.is_unit() || !unit.can_attack() {
        return false;
    }
    if deployed_anti_tank_gun_target_outside_arc(entities, attacker, target) {
        return false;
    }
    world_query::unit_explicit_attack_target_valid(
        entities, teams, fog, smokes, owner, attacker, target,
    )
}

fn attack_order_complete(
    map: &Map,
    entities: &EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    attacker: &Entity,
    target: u32,
) -> bool {
    if !attack_intent_valid(
        entities,
        teams,
        fog,
        Some(smokes),
        attacker.owner,
        attacker.id,
        target,
    ) {
        return true;
    }
    if panzerfaust_attack_cycle_active(attacker) {
        return false;
    }
    attacker.attack_unreachable_checks() >= ATTACK_UNREACHABLE_PROMOTION_CHECKS
        && !attack_can_fire_now(map, entities, attacker, target)
}

fn deployed_anti_tank_gun_target_outside_arc(entities: &EntityStore, id: u32, target: u32) -> bool {
    let Some(attacker) = entities.get(id) else {
        return false;
    };
    if attacker.kind != EntityKind::AntiTankGun
        || !matches!(
            attacker.weapon_setup(),
            crate::game::entity::WeaponSetup::Deployed
        )
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

fn gather_intent_valid(entities: &EntityStore, owner: u32, worker: u32, node: u32) -> bool {
    let is_gatherer = matches!(entities.get(worker), Some(e)
        if e.owner == owner && e.hp > 0 && matches!(e.kind, EntityKind::Worker | EntityKind::Golem));
    if !is_gatherer {
        return false;
    }
    let node_ok = matches!(entities.get(node), Some(n)
        if n.is_node() && n.kind != EntityKind::Oil && n.remaining().unwrap_or(0) > 0);
    if !node_ok {
        return false;
    }
    if !world_query::resource_has_completed_mining_cc(entities, owner, node) {
        return false;
    }
    if matches!(entities.node_slot_holder(node), Some(holder) if holder != worker) {
        return false;
    }
    true
}

fn deconstruct_intent_valid(
    entities: &EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    owner: u32,
    worker: u32,
    target: u32,
) -> bool {
    if !matches!(entities.get(worker), Some(e) if e.owner == owner && e.kind == EntityKind::Worker && e.hp > 0)
    {
        return false;
    }
    let Some(target) = entities.get(target) else {
        return false;
    };
    if target.kind != EntityKind::TankTrap || target.hp == 0 || target.under_construction() {
        return false;
    }
    teams.same_team_or_same_owner(owner, target.owner)
        || rules::projection::team_visible_world(owner, target.pos_x, target.pos_y, fog, teams)
}

#[allow(clippy::too_many_arguments)]
fn build_intent_promotion_error(
    map: &Map,
    entities: &EntityStore,
    players: &[PlayerState],
    owner: u32,
    worker: u32,
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> Option<String> {
    if !matches!(entities.get(worker), Some(e) if e.kind == EntityKind::Worker) {
        return Some(String::new());
    }
    if matches!(entities.get(worker), Some(e)
        if matches!(e.build_phase(), Some(BuildPhase::Constructing { .. })))
    {
        return Some(String::new());
    }
    if config::building_stats(kind).is_none() {
        return Some("Unknown building".to_string());
    }
    let owner_faction = players
        .iter()
        .find(|p| p.id == owner)
        .map(|p| p.faction_id.as_str())
        .unwrap_or(rules::faction::DEFAULT_FACTION_ID);
    let owned = world_query::completed_building_kinds(entities, owner);
    if !rules::economy::build_requirement_met_for_faction(owner_faction, kind, &owned) {
        return Some("Requirement not met".to_string());
    }
    if tile_x >= map.size || tile_y >= map.size {
        return Some("Cannot build there".to_string());
    }
    let can_resume =
        resumable_site_for_build_intent(map, entities, owner, kind, tile_x, tile_y).is_some();
    if !can_resume {
        match standability::building_site_status_for_build_intent(
            map, entities, kind, tile_x, tile_y, worker,
        ) {
            standability::BuildSiteStatus::Clear | standability::BuildSiteStatus::BlockedByUnit => {
            }
            standability::BuildSiteStatus::BlockedByBuilding
            | standability::BuildSiteStatus::BlockedByResourceNode
            | standability::BuildSiteStatus::InvalidFootprint => {
                return Some("Cannot build there".to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    mod artillery_point_fire_tests;
    mod queued_attack_tests;

    use super::*;
    use crate::game::ability::AbilityKind;
    use crate::game::entity::{
        EntityKind, EntityStore, MovePhase, Order, OrderIntent, WeaponSetup,
    };
    use crate::game::fog::Fog;
    use crate::game::map::Map;
    use crate::game::services::move_coordinator::MoveCoordinator;
    use crate::game::services::occupancy::{footprint_center, Occupancy};
    use crate::game::services::pathing::PathingService;
    use crate::game::ScoreState;
    use crate::protocol::terrain;

    fn flat_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![(4, 4)],
            expansion_sites: Vec::new(),
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

    fn promote(map: &Map, entities: &mut EntityStore) {
        let players = vec![player_state(1)];
        promote_with_players(map, entities, &players);
    }

    fn promote_with_players(map: &Map, entities: &mut EntityStore, players: &[PlayerState]) {
        let _ = promote_with_players_events(map, entities, players);
    }

    fn promote_with_players_events(
        map: &Map,
        entities: &mut EntityStore,
        players: &[PlayerState],
    ) -> std::collections::HashMap<u32, Vec<Event>> {
        let mut players: Vec<PlayerState> = players
            .iter()
            .map(|p| PlayerState {
                id: p.id,
                team_id: p.id,
                faction_id: "kriegsia".to_string(),
                name: p.name.clone(),
                color: p.color.clone(),
                start_tile: p.start_tile,
                steel: p.steel,
                oil: p.oil,
                supply_used: p.supply_used,
                supply_cap: p.supply_cap,
                is_ai: p.is_ai,
                score: p.score.clone(),
                upgrades: p.upgrades.clone(),
            })
            .collect();
        let occ = Occupancy::build(map, entities);
        let mut pathing = PathingService::new(1024, 32);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, map, &occ, 1);
        let mut fog = Fog::new(map.size);
        let player_ids: Vec<u32> = players.iter().map(|p| p.id).collect();
        fog.recompute(&player_ids, entities, map);
        let mut smokes = SmokeCloudStore::new();
        let mut ability_runtime = AbilityRuntime::new();
        let mut mortar_shells = MortarShellStore::default();
        let mut events = std::collections::HashMap::new();
        promote_ready_orders(
            map,
            entities,
            &mut players,
            &fog,
            &mut coordinator,
            &mut smokes,
            &mut ability_runtime,
            &mut mortar_shells,
            &mut events,
            1,
        );
        events
    }

    #[test]
    fn idle_unit_promotes_first_queued_move() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        entities
            .get_mut(unit)
            .expect("unit should exist")
            .append_queued_order(OrderIntent::move_to(180.0, 100.0));

        promote(&map, &mut entities);

        let entity = entities.get(unit).expect("unit should exist");
        assert!(matches!(entity.order(), Order::Move(_)));
        assert_eq!(entity.move_phase(), Some(MovePhase::AwaitingPath));
        assert!(entity.queued_orders().is_empty());
    }

    #[test]
    fn queued_point_moves_promote_same_destination_as_group() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let first = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("first rifleman should spawn");
        let second = entities
            .spawn_unit(1, EntityKind::Rifleman, 132.0, 100.0)
            .expect("second rifleman should spawn");
        let target = (240.0, 160.0);
        for unit in [first, second] {
            entities
                .get_mut(unit)
                .expect("unit should exist")
                .append_queued_order(OrderIntent::move_to(target.0, target.1));
        }

        promote(&map, &mut entities);

        let first_goal = entities
            .get(first)
            .expect("first rifleman should exist")
            .move_intent()
            .expect("first rifleman should receive a move");
        let second_goal = entities
            .get(second)
            .expect("second rifleman should exist")
            .move_intent()
            .expect("second rifleman should receive a move");
        assert_ne!(
            first_goal, second_goal,
            "same-destination queued moves should promote through one grouped formation"
        );
        for unit in [first, second] {
            let entity = entities.get(unit).expect("unit should exist");
            assert!(matches!(entity.order(), Order::Move(_)));
            assert_eq!(entity.move_phase(), Some(MovePhase::AwaitingPath));
            assert!(entity.queued_orders().is_empty());
        }
    }

    #[test]
    fn attack_move_engagement_without_arrival_does_not_promote() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        {
            let entity = entities.get_mut(unit).expect("unit should exist");
            entity.set_order(Order::attack_move_to(300.0, 100.0));
            entity.mark_move_phase(MovePhase::Moving);
            entity.append_queued_order(OrderIntent::move_to(360.0, 100.0));
        }

        promote(&map, &mut entities);

        let entity = entities.get(unit).expect("unit should exist");
        assert!(matches!(entity.order(), Order::AttackMove(_)));
        assert_eq!(entity.queued_orders().len(), 1);
    }

    #[test]
    fn arrived_attack_move_promotes_after_reaching_destination() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        {
            let entity = entities.get_mut(unit).expect("unit should exist");
            entity.set_order(Order::attack_move_to(120.0, 100.0));
            entity.mark_move_phase(MovePhase::Arrived);
            entity.append_queued_order(OrderIntent::attack_move_to(180.0, 100.0));
        }

        promote(&map, &mut entities);

        let entity = entities.get(unit).expect("unit should exist");
        assert!(matches!(entity.order(), Order::AttackMove(_)));
        assert_eq!(entity.move_phase(), Some(MovePhase::AwaitingPath));
        assert!(entity.queued_orders().is_empty());
    }

    #[test]
    fn idle_worker_promotes_queued_gather_on_valid_node() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let (cx, cy) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::CityCentre, cx, cy, true)
            .expect("city centre should spawn");
        let node = entities
            .spawn_node(EntityKind::Steel, cx + 64.0, cy)
            .expect("steel node should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, cx, cy + 16.0)
            .expect("worker should spawn");
        entities
            .get_mut(worker)
            .expect("worker should exist")
            .append_queued_order(OrderIntent::gather(node));

        promote(&map, &mut entities);

        let entity = entities.get(worker).expect("worker should exist");
        assert!(matches!(entity.order(), Order::Gather(_)));
        assert!(entity.queued_orders().is_empty());
    }

    #[test]
    fn queued_gather_on_depleted_node_is_skipped_silently() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let (cx, cy) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::CityCentre, cx, cy, true)
            .expect("city centre should spawn");
        let node = entities
            .spawn_node(EntityKind::Steel, cx + 64.0, cy)
            .expect("node should spawn");
        // Deplete the node manually so it survives in-store but has nothing to mine.
        if let Some(n) = entities.get_mut(node) {
            n.harvest_resources(u32::MAX);
        }
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, cx, cy + 16.0)
            .expect("worker should spawn");
        let fallback = (cx + 96.0, cy);
        {
            let w = entities.get_mut(worker).expect("worker should exist");
            w.append_queued_order(OrderIntent::gather(node));
            w.append_queued_order(OrderIntent::move_to(fallback.0, fallback.1));
        }

        promote(&map, &mut entities);

        let entity = entities.get(worker).expect("worker should exist");
        assert!(
            matches!(entity.order(), Order::Move(_)),
            "depleted gather should be skipped and the next move intent should promote"
        );
        assert!(entity.queued_orders().is_empty());
    }

    #[test]
    fn queued_gather_on_oil_node_is_skipped() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let (cx, cy) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::CityCentre, cx, cy, true)
            .expect("city centre should spawn");
        let node = entities
            .spawn_node(EntityKind::Oil, cx + 64.0, cy)
            .expect("oil node should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, cx, cy + 16.0)
            .expect("worker should spawn");
        let fallback = (cx + 96.0, cy);
        {
            let w = entities.get_mut(worker).expect("worker should exist");
            w.append_queued_order(OrderIntent::gather(node));
            w.append_queued_order(OrderIntent::move_to(fallback.0, fallback.1));
        }

        promote(&map, &mut entities);

        let entity = entities.get(worker).expect("worker should exist");
        assert!(
            matches!(entity.order(), Order::Move(_)),
            "queued direct oil gather should be skipped and the next intent should promote"
        );
        assert!(entity.queued_orders().is_empty());
    }

    #[test]
    fn idle_worker_promotes_queued_build_on_clear_site() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
            .expect("city centre should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, cc_x + 96.0, cc_y)
            .expect("worker should spawn");
        entities
            .get_mut(worker)
            .expect("worker should exist")
            .append_queued_order(OrderIntent::build(EntityKind::Depot, 16, 16));

        let players = vec![player_state(1)];
        promote_with_players(&map, &mut entities, &players);

        let entity = entities.get(worker).expect("worker should exist");
        assert!(matches!(entity.order(), Order::Build(_)));
        assert!(entity.queued_orders().is_empty());
    }

    #[test]
    fn queued_build_promotes_when_player_cannot_afford() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
            .expect("city centre should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, cc_x + 96.0, cc_y)
            .expect("worker should spawn");
        let fallback = (cc_x + 160.0, cc_y);
        {
            let w = entities.get_mut(worker).expect("worker should exist");
            w.append_queued_order(OrderIntent::build(EntityKind::Depot, 16, 16));
            w.append_queued_order(OrderIntent::move_to(fallback.0, fallback.1));
        }
        let mut players = vec![player_state(1)];
        players[0].steel = 0;
        players[0].oil = 0;

        promote_with_players(&map, &mut entities, &players);

        let entity = entities.get(worker).expect("worker should exist");
        assert!(
            matches!(entity.order(), Order::Build(_)),
            "unaffordable build should promote and wait at the site instead of being skipped"
        );
        assert_eq!(
            entity.queued_orders(),
            &[OrderIntent::move_to(fallback.0, fallback.1)],
            "fallback queued orders should remain available after the active build order finishes"
        );
    }

    #[test]
    fn queued_build_promotion_defers_resource_notice_until_arrival() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
            .expect("city centre should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, cc_x + 96.0, cc_y)
            .expect("worker should spawn");
        entities
            .get_mut(worker)
            .expect("worker should exist")
            .append_queued_order(OrderIntent::build(EntityKind::Depot, 16, 16));
        let mut players = vec![player_state(1)];
        players[0].steel = 0;
        players[0].oil = 0;

        let events = promote_with_players_events(&map, &mut entities, &players);

        assert!(
            events.get(&1).is_none_or(Vec::is_empty),
            "promotion should not spam resource notices before the worker reaches the build site"
        );
        assert!(matches!(
            entities.get(worker).expect("worker should exist").order(),
            Order::Build(_)
        ));
    }

    #[test]
    fn queued_build_promotes_resume_when_player_cannot_afford_original_cost() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
            .expect("city centre should spawn");
        let (site_x, site_y) = footprint_center(&map, EntityKind::Depot, 16, 16);
        entities
            .spawn_building(1, EntityKind::Depot, site_x, site_y, false)
            .expect("scaffold should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, cc_x + 96.0, cc_y)
            .expect("worker should spawn");
        entities
            .get_mut(worker)
            .expect("worker should exist")
            .append_queued_order(OrderIntent::build(EntityKind::Depot, 16, 16));
        let mut players = vec![player_state(1)];
        players[0].steel = 0;
        players[0].oil = 0;

        promote_with_players(&map, &mut entities, &players);

        let entity = entities.get(worker).expect("worker should exist");
        assert!(
            matches!(entity.order(), Order::Build(_)),
            "queued resume should promote even when a new depot is unaffordable"
        );
        assert_eq!(
            entity.order().build_intent_tile(),
            Some((EntityKind::Depot, 16, 16))
        );
        assert!(entity.queued_orders().is_empty());
    }

    #[test]
    fn queued_build_with_huge_tiles_is_drained_without_panic() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
            .expect("city centre should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, cc_x + 96.0, cc_y)
            .expect("worker should spawn");
        {
            let w = entities.get_mut(worker).expect("worker should exist");
            for _ in 0..MAX_QUEUED_ORDERS {
                w.append_queued_order(OrderIntent::build(EntityKind::Depot, u32::MAX, u32::MAX));
            }
        }

        promote(&map, &mut entities);

        let entity = entities.get(worker).expect("worker should exist");
        assert!(
            matches!(entity.order(), Order::Idle),
            "invalid queued build intents should not promote to an active order"
        );
        assert!(
            entity.queued_orders().is_empty(),
            "promotion should drain bounded invalid intents instead of retrying forever"
        );
    }

    #[test]
    fn idle_worker_promotes_queued_deconstruct_on_visible_tank_trap() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, 100.0, 100.0)
            .expect("worker should spawn");
        let trap = entities
            .spawn_building(2, EntityKind::TankTrap, 132.0, 100.0, true)
            .expect("tank trap should spawn");
        entities
            .get_mut(worker)
            .expect("worker should exist")
            .append_queued_order(OrderIntent::deconstruct(trap));
        let players = vec![player_state(1), player_state(2)];

        promote_with_players(&map, &mut entities, &players);

        let entity = entities.get(worker).expect("worker should exist");
        assert!(matches!(entity.order(), Order::Deconstruct(_)));
        assert_eq!(entity.order().deconstruct_target(), Some(trap));
        assert_eq!(entity.target_id(), Some(trap));
        assert!(entity.queued_orders().is_empty());
    }

    #[test]
    fn queued_deconstruct_skips_non_tank_trap_and_promotes_next_stage() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, 100.0, 100.0)
            .expect("worker should spawn");
        let depot = entities
            .spawn_building(2, EntityKind::Depot, 132.0, 100.0, true)
            .expect("depot should spawn");
        {
            let w = entities.get_mut(worker).expect("worker should exist");
            w.append_queued_order(OrderIntent::deconstruct(depot));
            w.append_queued_order(OrderIntent::move_to(220.0, 100.0));
        }
        let players = vec![player_state(1), player_state(2)];

        promote_with_players(&map, &mut entities, &players);

        let entity = entities.get(worker).expect("worker should exist");
        assert!(
            matches!(entity.order(), Order::Move(_)),
            "invalid deconstruct target should be skipped and the next queued stage should promote"
        );
        assert!(entity.queued_orders().is_empty());
    }

    #[test]
    fn idle_combat_unit_promotes_queued_attack() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        let target = entities
            .spawn_unit(2, EntityKind::Rifleman, 160.0, 100.0)
            .expect("target should spawn");
        entities
            .get_mut(attacker)
            .expect("attacker should exist")
            .append_queued_order(OrderIntent::attack(target));
        let players = vec![player_state(1), player_state(2)];

        promote_with_players(&map, &mut entities, &players);

        let entity = entities.get(attacker).expect("attacker should exist");
        assert!(matches!(entity.order(), Order::Attack(_)));
        assert_eq!(entity.order().attack_target(), Some(target));
        assert_eq!(entity.target_id(), Some(target));
        assert!(entity.queued_orders().is_empty());
    }

    #[test]
    fn unit_executes_move_attack_then_move_queue() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        let target = entities
            .spawn_unit(2, EntityKind::Rifleman, 160.0, 100.0)
            .expect("target should spawn");
        {
            let unit = entities.get_mut(attacker).expect("attacker should exist");
            unit.set_order(Order::move_to(120.0, 100.0));
            unit.mark_move_phase(MovePhase::Arrived);
            unit.append_queued_order(OrderIntent::attack(target));
            unit.append_queued_order(OrderIntent::move_to(220.0, 100.0));
        }
        let players = vec![player_state(1), player_state(2)];

        promote_with_players(&map, &mut entities, &players);
        {
            let unit = entities.get(attacker).expect("attacker should exist");
            assert!(matches!(unit.order(), Order::Attack(_)));
            assert_eq!(unit.queued_orders().len(), 1);
        }
        entities.get_mut(target).expect("target should exist").hp = 0;

        promote_with_players(&map, &mut entities, &players);

        let unit = entities.get(attacker).expect("attacker should exist");
        assert!(matches!(unit.order(), Order::Move(_)));
        assert_eq!(unit.move_phase(), Some(MovePhase::AwaitingPath));
        assert!(unit.queued_orders().is_empty());
    }

    #[test]
    fn queued_attack_skips_when_target_is_dead_before_promotion() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        let target = entities
            .spawn_unit(2, EntityKind::Rifleman, 160.0, 100.0)
            .expect("target should spawn");
        let fallback = (220.0, 100.0);
        {
            let target = entities.get_mut(target).expect("target should exist");
            target.hp = 0;
        }
        {
            let unit = entities.get_mut(attacker).expect("attacker should exist");
            unit.append_queued_order(OrderIntent::attack(target));
            unit.append_queued_order(OrderIntent::move_to(fallback.0, fallback.1));
        }
        let players = vec![player_state(1), player_state(2)];

        promote_with_players(&map, &mut entities, &players);

        let unit = entities.get(attacker).expect("attacker should exist");
        assert!(
            matches!(unit.order(), Order::Move(_)),
            "dead attack target should be skipped and next move promoted"
        );
        assert!(unit.queued_orders().is_empty());
    }

    #[test]
    fn completed_attack_promotes_following_attack_move_destination() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        let target = entities
            .spawn_unit(2, EntityKind::Rifleman, 160.0, 100.0)
            .expect("target should spawn");
        {
            let unit = entities.get_mut(attacker).expect("attacker should exist");
            unit.set_order(Order::attack(target));
            unit.set_target_id(Some(target));
            unit.append_queued_order(OrderIntent::attack_move_to(240.0, 100.0));
        }
        entities.get_mut(target).expect("target should exist").hp = 0;
        let players = vec![player_state(1), player_state(2)];

        promote_with_players(&map, &mut entities, &players);

        let unit = entities.get(attacker).expect("attacker should exist");
        assert!(matches!(unit.order(), Order::AttackMove(_)));
        assert_eq!(unit.move_phase(), Some(MovePhase::AwaitingPath));
        let intent = unit
            .move_intent()
            .expect("attack-move promotion should keep a destination");
        assert!(
            (intent.0 - 240.0).abs() <= config::TILE_SIZE as f32
                && (intent.1 - 100.0).abs() <= config::TILE_SIZE as f32,
            "formation-adjusted attack-move intent should stay near the queued destination, got {intent:?}"
        );
    }

    #[test]
    fn queued_legacy_charge_skips_to_attack_move() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let rifle = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        let (tx, ty) = footprint_center(&map, EntityKind::TrainingCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::TrainingCentre, tx, ty, true)
            .expect("training centre should spawn");
        {
            let unit = entities.get_mut(rifle).expect("rifleman should exist");
            unit.set_order(Order::move_to(140.0, 100.0));
            unit.mark_move_phase(MovePhase::Arrived);
            unit.append_queued_order(OrderIntent::self_ability(AbilityKind::Charge));
            unit.append_queued_order(OrderIntent::attack_move_to(240.0, 100.0));
        }

        promote(&map, &mut entities);

        let unit = entities.get(rifle).expect("rifleman should exist");
        assert!(matches!(unit.order(), Order::AttackMove(_)));
        assert!(unit.queued_orders().is_empty());
    }

    #[test]
    fn queued_world_ability_rechecks_cooldown_and_charges_at_promotion() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let target = map.tile_center(10, 8);
        let cooling = entities
            .spawn_unit(1, EntityKind::ScoutCar, target.0 - 96.0, target.1)
            .expect("cooling scout car should spawn");
        let spent = entities
            .spawn_unit(1, EntityKind::ScoutCar, target.0 - 128.0, target.1)
            .expect("spent scout car should spawn");
        for scout in [cooling, spent] {
            let unit = entities.get_mut(scout).expect("scout car should exist");
            unit.append_queued_order(OrderIntent::ability(AbilityKind::Smoke, target.0, target.1));
            unit.append_queued_order(OrderIntent::move_to(target.0 + 96.0, target.1));
        }
        entities
            .get_mut(cooling)
            .expect("cooling scout car should exist")
            .start_ability_cooldown(AbilityKind::Smoke, 5);
        while entities
            .get_mut(spent)
            .expect("spent scout car should exist")
            .consume_ability_use(AbilityKind::Smoke)
        {}
        let players = vec![player_state(1), player_state(2)];

        promote_with_players(&map, &mut entities, &players);

        for scout in [cooling, spent] {
            let unit = entities.get(scout).expect("scout car should exist");
            assert!(
                matches!(unit.order(), Order::Move(_)),
                "stale queued Smoke should be skipped and the later move should promote"
            );
            assert!(
                unit.queued_orders().is_empty(),
                "promotion should drain the stale Smoke intent and promoted fallback"
            );
        }
    }

    #[test]
    fn queued_at_setup_faces_from_arrived_position_and_keeps_later_attack_move() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let at = entities
            .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
            .expect("at gun should spawn");
        {
            let unit = entities.get_mut(at).expect("at gun should exist");
            unit.set_order(Order::move_to(150.0, 100.0));
            unit.mark_move_phase(MovePhase::Arrived);
            unit.pos_x = 150.0;
            unit.pos_y = 100.0;
            unit.append_queued_order(OrderIntent::setup_anti_tank_guns(150.0, 140.0));
            unit.append_queued_order(OrderIntent::attack_move_to(240.0, 100.0));
        }

        promote(&map, &mut entities);

        let unit = entities.get(at).expect("at gun should exist");
        assert_eq!(unit.weapon_setup(), WeaponSetup::Packed);
        assert!(
            (unit.emplacement_facing().unwrap_or_default() - std::f32::consts::FRAC_PI_2).abs()
                < 0.001,
            "setup facing should be computed from the arrived position"
        );
        assert_eq!(unit.queued_orders().len(), 1);

        promote(&map, &mut entities);

        let unit = entities.get(at).expect("at gun should exist");
        assert!(matches!(unit.order(), Order::AttackMove(_)));
        assert!(unit.queued_orders().is_empty());
    }

    #[test]
    fn queued_artillery_point_fire_outside_arc_redeploys_on_promotion() {
        let map = flat_map(64);
        let mut entities = EntityStore::new();
        let pos = (320.0, 320.0);
        let angle = config::ARTILLERY_FIELD_OF_FIRE_RAD;
        let distance = config::TILE_SIZE as f32 * 30.0;
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
            unit.append_queued_order(OrderIntent::point_fire(target.0, target.1));
        }

        promote(&map, &mut entities);

        let unit = entities.get(artillery).expect("artillery should exist");
        assert!(matches!(
            unit.weapon_setup(),
            WeaponSetup::TearingDownToRedeploy { .. }
        ));
        assert!(matches!(unit.order(), Order::ArtilleryPointFire(_)));
        assert!(
            (unit.pending_redeploy_facing().unwrap_or_default() - angle).abs() < 0.001,
            "queued outside-arc point fire should redeploy toward the requested target"
        );
        assert!(
            unit.emplacement_facing().unwrap_or_default().abs() < 0.001,
            "queued point fire must not walk the active field of fire before redeploy"
        );
    }

    #[test]
    fn unreachable_attack_promotes_next_queued_order() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        let target = entities
            .spawn_unit(2, EntityKind::Rifleman, 700.0, 100.0)
            .expect("target should spawn");
        {
            let unit = entities.get_mut(attacker).expect("attacker should exist");
            unit.set_order(Order::attack(target));
            unit.set_target_id(Some(target));
            for _ in 0..ATTACK_UNREACHABLE_PROMOTION_CHECKS {
                unit.increment_attack_unreachable_checks();
            }
            unit.append_queued_order(OrderIntent::move_to(220.0, 100.0));
        }
        let players = vec![player_state(1), player_state(2)];

        promote_with_players(&map, &mut entities, &players);

        let unit = entities.get(attacker).expect("attacker should exist");
        assert!(
            matches!(unit.order(), Order::Move(_)),
            "bounded unreachable attack should not stall the queued move forever"
        );
        assert!(unit.queued_orders().is_empty());
    }
}
