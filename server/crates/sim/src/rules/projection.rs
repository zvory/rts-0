//! Projection rules for fog-gated entity views and event delivery.
//!
//! This module owns what a player is allowed to see. It does not mutate the world; future
//! last-known-position or partial-reveal rules should grow here.

use std::collections::BTreeSet;

use crate::config;
use crate::game::ability;
use crate::game::ability_runtime::{AbilityObjectPayload, AbilityRuntime};
use crate::game::entity::{
    active_trench_occupation, fires_while_moving, tank_trap_deconstruction_ticks, Entity,
    EntityKind, EntityStore, GatherPhase, Order, OrderIntent, PanzerfaustState,
};
use crate::game::fog::Fog;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::protocol;
use crate::protocol::{AbilityCooldownView, DebugPathPoint, DebugPathView};
use crate::protocol::{EntityView, OrderPlanMarker};

const MAX_DEBUG_PATH_WAYPOINTS: usize = 128;
const TANK_STATIONARY_RANGE_MAX_TILES: f32 = 14.0;
const TANK_STATIONARY_RANGE_RAMP_TICKS: u16 = config::TICK_HZ as u16 * 3;

pub struct EntityProjectionContext<'a> {
    pub fog: &'a Fog,
    pub actionable_fog: Option<&'a Fog>,
    pub private_detail_fog: Option<&'a Fog>,
    pub private_detail_projection: PrivateDetailProjection,
    pub smokes: Option<&'a SmokeCloudStore>,
    pub fogged: bool,
    pub entities: &'a EntityStore,
    pub target: Option<&'a Entity>,
    pub debug_path_projection: DebugPathProjection,
    pub active_construction_sites: Option<&'a BTreeSet<u32>>,
    pub teams: Option<&'a TeamRelations>,
    pub owner_faction_id: Option<&'a str>,
    pub ability_runtime: Option<&'a AbilityRuntime>,
    pub tick: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DebugPathProjection {
    None,
    OwnerOnly,
    AllProjected,
}

impl DebugPathProjection {
    fn includes(self, viewer: u32, entity: &Entity) -> bool {
        match self {
            Self::None => false,
            Self::OwnerOnly => entity.owner == viewer,
            Self::AllProjected => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrivateDetailProjection {
    ExactViewer,
    AllProjected,
}

impl PrivateDetailProjection {
    fn viewer_for(self, viewer: u32, entity: &Entity) -> Option<u32> {
        match self {
            Self::ExactViewer => (entity.owner == viewer).then_some(viewer),
            Self::AllProjected => (entity.owner != 0).then_some(entity.owner),
        }
    }
}

pub fn entity_visible_to(viewer: u32, entity: &Entity, fog: &Fog) -> bool {
    entity.owner == viewer
        || entity.is_node()
        || fog.is_visible_world(viewer, entity.pos_x, entity.pos_y)
}

pub fn entity_visible_to_with_smoke(
    viewer: u32,
    entity: &Entity,
    fog: &Fog,
    smokes: &SmokeCloudStore,
) -> bool {
    if entity.owner != viewer
        && !entity.is_node()
        && smokes.point_inside(entity.pos_x, entity.pos_y)
    {
        return false;
    }
    entity_visible_to(viewer, entity, fog)
}

pub fn event_visible_to(
    viewer: u32,
    event_origin_x: f32,
    event_origin_y: f32,
    attacker_owner: u32,
    fog: &Fog,
) -> bool {
    viewer == attacker_owner || fog.is_visible_world(viewer, event_origin_x, event_origin_y)
}

pub fn team_visible_world(viewer: u32, x: f32, y: f32, fog: &Fog, teams: &TeamRelations) -> bool {
    teams
        .same_team_player_ids(viewer)
        .into_iter()
        .any(|player_id| fog.is_visible_world(player_id, x, y))
}

pub fn event_visible_to_team(
    viewer: u32,
    event_origin_x: f32,
    event_origin_y: f32,
    owner: u32,
    fog: &Fog,
    teams: &TeamRelations,
) -> bool {
    teams.same_team_or_same_owner(viewer, owner)
        || team_visible_world(viewer, event_origin_x, event_origin_y, fog, teams)
}

pub fn event_visible_to_team_with_smoke(
    viewer: u32,
    event_origin_x: f32,
    event_origin_y: f32,
    owner: u32,
    fog: &Fog,
    teams: &TeamRelations,
    smokes: &SmokeCloudStore,
) -> bool {
    if !teams.same_team_or_same_owner(viewer, owner)
        && smokes.point_inside(event_origin_x, event_origin_y)
    {
        return false;
    }
    event_visible_to_team(viewer, event_origin_x, event_origin_y, owner, fog, teams)
}

#[allow(clippy::too_many_arguments)]
pub fn attack_event_visible_to_team(
    viewer: u32,
    attacker_x: f32,
    attacker_y: f32,
    target_x: f32,
    target_y: f32,
    attacker_owner: u32,
    fog: &Fog,
    teams: &TeamRelations,
) -> bool {
    event_visible_to_team(viewer, attacker_x, attacker_y, attacker_owner, fog, teams)
        || team_visible_world(viewer, target_x, target_y, fog, teams)
}

pub fn event_visible_to_with_smoke(
    viewer: u32,
    event_origin_x: f32,
    event_origin_y: f32,
    attacker_owner: u32,
    fog: &Fog,
    smokes: &SmokeCloudStore,
) -> bool {
    if viewer != attacker_owner && smokes.point_inside(event_origin_x, event_origin_y) {
        return false;
    }
    event_visible_to(viewer, event_origin_x, event_origin_y, attacker_owner, fog)
}

#[allow(dead_code)]
pub fn attack_event_visible_to(
    viewer: u32,
    attacker_x: f32,
    attacker_y: f32,
    target_x: f32,
    target_y: f32,
    attacker_owner: u32,
    fog: &Fog,
) -> bool {
    event_visible_to(viewer, attacker_x, attacker_y, attacker_owner, fog)
        || fog.is_visible_world(viewer, target_x, target_y)
}

#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub fn attack_event_visible_to_with_smoke(
    viewer: u32,
    attacker_x: f32,
    attacker_y: f32,
    target_x: f32,
    target_y: f32,
    attacker_owner: u32,
    fog: &Fog,
    smokes: &SmokeCloudStore,
) -> bool {
    if viewer != attacker_owner && smokes.point_inside(attacker_x, attacker_y) {
        return false;
    }
    event_visible_to_with_smoke(viewer, attacker_x, attacker_y, attacker_owner, fog, smokes)
        || (!smokes.point_inside(target_x, target_y)
            && fog.is_visible_world(viewer, target_x, target_y))
}

pub fn project_entity(
    viewer: u32,
    entity: &Entity,
    context: EntityProjectionContext<'_>,
) -> Option<EntityView> {
    if context.fogged
        && !context
            .smokes
            .map(|smokes| entity_visible_to_with_smoke(viewer, entity, context.fog, smokes))
            .unwrap_or_else(|| entity_visible_to(viewer, entity, context.fog))
    {
        return None;
    }

    let mut view = EntityView::new(
        entity.id,
        entity.owner,
        protocol::kind_to_wire(entity.kind),
        entity.pos_x,
        entity.pos_y,
        entity.hp,
        entity.max_hp,
        entity.state_str(),
    );
    let actionable_fog = context.actionable_fog.unwrap_or(context.fog);
    let private_detail_fog = context.private_detail_fog.unwrap_or(actionable_fog);
    let private_detail_viewer = context.private_detail_projection.viewer_for(viewer, entity);
    let private_detail_owner = private_detail_viewer.is_some();
    let owner_or_ally = context
        .teams
        .map(|teams| teams.same_team_or_same_owner(viewer, entity.owner))
        .unwrap_or(entity.owner == viewer)
        || private_detail_owner;
    let exact_owner = entity.owner == viewer;
    let vision_only = context.fogged
        && !owner_or_ally
        && !entity.is_node()
        && !context
            .smokes
            .map(|smokes| entity_visible_to_with_smoke(viewer, entity, actionable_fog, smokes))
            .unwrap_or_else(|| entity_visible_to(viewer, entity, actionable_fog));
    view.vision_only = vision_only;

    if entity.is_unit() {
        view.facing = Some(entity.facing());
    }
    if let Some(oil_used) = entity.lifetime_oil_used() {
        view.oil_used = Some(oil_used);
    }
    if entity.kind == EntityKind::Tank && owner_or_ally {
        if let Some(stats) = config::unit_stats(entity.kind) {
            view.weapon_range_tiles =
                Some(tank_weapon_range_tiles(entity, stats.range_tiles as f32));
        }
    }
    if entity.kind == EntityKind::Panzerfaust {
        view.panzerfaust_loaded = entity
            .combat
            .as_ref()
            .and_then(|combat| combat.panzerfaust)
            .map(|state| {
                matches!(
                    state,
                    PanzerfaustState::Loaded | PanzerfaustState::Windup { .. }
                )
            });
    }
    let acquired_combat_target = entity.can_attack() && entity.target_id().is_some();
    let active_combat_target =
        matches!(entity.order(), Order::Attack(_) | Order::AttackMove(_)) || acquired_combat_target;
    let target_visible = if let Some(target_id) = entity.target_id() {
        context
            .target
            .filter(|target| target.id == target_id)
            .map(|target| {
                exact_owner
                    || !context.fogged
                    || (!vision_only
                        && actionable_fog.is_visible_world(viewer, target.pos_x, target.pos_y)
                        && !context
                            .smokes
                            .map(|smokes| smokes.point_inside(target.pos_x, target.pos_y))
                            .unwrap_or(false))
            })
            .unwrap_or(false)
    } else {
        false
    };
    let weapon_facing_useful = fires_while_moving(entity.kind)
        || active_combat_target
        || (entity.is_building() && entity.can_attack());
    if weapon_facing_useful {
        if let Some(weapon_facing) = entity.weapon_facing() {
            let weapon_facing_is_safe = exact_owner
                || !context.fogged
                || entity.target_id().is_none()
                || !active_combat_target
                || target_visible;
            if weapon_facing_is_safe {
                view.weapon_facing = Some(weapon_facing);
            }
        }
    }
    if matches!(
        entity.kind,
        EntityKind::MachineGunner
            | EntityKind::AntiTankGun
            | EntityKind::MortarTeam
            | EntityKind::Artillery
    ) {
        view.setup_state = Some(entity.weapon_setup().to_protocol_str().to_string());
    }
    if matches!(entity.kind, EntityKind::AntiTankGun | EntityKind::Artillery) && owner_or_ally {
        view.setup_facing = entity.emplacement_facing();
    }

    if entity.is_building() && !entity.prod_queue().is_empty() {
        if let Some(front) = entity.prod_queue().first() {
            view.prod_kind = Some(protocol::kind_to_wire(front.unit).to_string());
            view.prod_progress = Some(if front.total == 0 {
                0.0
            } else {
                front.progress as f32 / front.total as f32
            });
        }
        if owner_or_ally {
            view.prod_queue = Some(entity.prod_queue().len() as u32);
            view.prod_waiting = entity.prod_queue().first().is_some_and(|item| !item.paid);
            view.prod_scout_plane_queued = entity
                .prod_queue()
                .iter()
                .any(|item| item.unit == EntityKind::ScoutPlane);
        }
    }
    if entity.is_building() && !entity.research_queue().is_empty() {
        if let Some(front) = entity.research_queue().first() {
            view.prod_upgrade = Some(front.upgrade.to_protocol_str().to_string());
            view.prod_progress = Some(if front.total == 0 {
                0.0
            } else {
                front.progress as f32 / front.total as f32
            });
        }
        if owner_or_ally {
            view.prod_queue = Some(entity.research_queue().len() as u32);
            view.prod_waiting = entity
                .research_queue()
                .first()
                .is_some_and(|item| !item.paid);
        }
    }
    if owner_or_ally {
        view.prod_repeat_kinds = entity
            .production
            .as_ref()
            .map(|production| {
                production
                    .repeat_units
                    .iter()
                    .map(|&unit| protocol::kind_to_wire(unit).to_string())
                    .collect()
            })
            .unwrap_or_default();
    }

    // Rally/order/ability details are private in normal projections. Full-world diagnostic
    // projections intentionally inspect each entity through its real owner instead of a fake viewer.
    if let Some(private_viewer) = private_detail_viewer {
        if let Some((rx, ry)) = entity.rally_point() {
            view.rally = Some([rx, ry]);
        }
        view.rally_plan = entity
            .rally_plan()
            .into_iter()
            .map(|stage| OrderPlanMarker {
                kind: stage.kind.to_protocol_str().to_string(),
                x: stage.point.x,
                y: stage.point.y,
            })
            .collect();
        view.order_plan = order_plan(
            entity,
            context.entities,
            private_viewer,
            private_detail_fog,
            context.smokes,
        );
        if let Some((orbit_center, source_command_car)) = entity.scout_plane_private_details() {
            view.scout_plane = Some(protocol::ScoutPlaneStateView {
                orbit_center: Some([orbit_center.0, orbit_center.1]),
                source_command_car,
            });
        }
        let catalog = context
            .owner_faction_id
            .and_then(crate::rules::faction::catalog_for);
        view.abilities = entity
            .ability_cooldowns
            .iter()
            .filter(|(_, cooldown_left)| **cooldown_left > 0)
            .filter(|(kind, _)| {
                catalog.is_some_and(|catalog| {
                    catalog
                        .ability(kind.to_protocol_str())
                        .is_some_and(|entry| {
                            entry.command_card && entry.carriers.contains(&entity.kind)
                        })
                })
            })
            .map(|(kind, cooldown_left)| AbilityCooldownView {
                ability: kind.to_protocol_str().to_string(),
                cooldown_left: *cooldown_left,
                remaining_uses: entity.ability_uses_remaining(*kind),
                autocast_enabled: entity.autocast_enabled(*kind),
                active_object_id: active_return_object_id(&context, entity, *kind),
                available_tick: return_available_tick(&context, entity, *kind),
                lockout_until_tick: entity.ability_lockout_until_tick(*kind, context.tick),
                expires_in: active_ability_object_expires_in(&context, entity, *kind),
            })
            .collect();
        for entry in catalog
            .into_iter()
            .flat_map(|catalog| catalog.abilities_for_carrier(entity.kind))
            .filter(|entry| entry.command_card)
        {
            let Ok(kind) = entry.id.parse::<ability::AbilityKind>() else {
                continue;
            };
            if ability::carried_by(kind, entity.kind)
                && !view
                    .abilities
                    .iter()
                    .any(|cooldown| cooldown.ability == kind.to_protocol_str())
            {
                view.abilities.push(AbilityCooldownView {
                    ability: kind.to_protocol_str().to_string(),
                    cooldown_left: 0,
                    remaining_uses: entity.ability_uses_remaining(kind),
                    autocast_enabled: entity.autocast_enabled(kind),
                    active_object_id: active_return_object_id(&context, entity, kind),
                    available_tick: return_available_tick(&context, entity, kind),
                    lockout_until_tick: entity.ability_lockout_until_tick(kind, context.tick),
                    expires_in: active_ability_object_expires_in(&context, entity, kind),
                });
            }
        }
    }

    if context.debug_path_projection.includes(viewer, entity) {
        view.debug_path = debug_path_view(entity);
    }

    if entity.breakthrough_ticks() > 0 {
        view.breakthrough_ticks = Some(entity.breakthrough_ticks());
    }
    if entity.breakthrough_aura_ticks() > 0 {
        view.breakthrough_aura_ticks = Some(entity.breakthrough_aura_ticks());
    }
    view.occupied_trench_id = active_trench_occupation(entity);

    if let Some(progress) = entity.build_progress_fraction() {
        view.build_progress = Some(progress);
        view.build_active = private_detail_owner
            && context
                .active_construction_sites
                .is_some_and(|sites| sites.contains(&entity.id));
    }
    if entity.kind == EntityKind::TankTrap && !entity.under_construction() && !vision_only {
        view.deconstruct_progress = deconstruct_progress_for_target(entity.id, context.entities);
    }

    // Current behavior exposes static resource amount even through fog.
    if entity.is_node() {
        view.remaining = entity.remaining();
    }

    if matches!(
        entity.kind,
        crate::game::entity::EntityKind::Worker | crate::game::entity::EntityKind::Golem
    ) && entity.gather_phase() == Some(GatherPhase::Harvesting)
    {
        if let Some(node) = entity.order().gather_node() {
            view.latched_node = Some(node);
        }
    }

    if let Some(target_id) = entity.target_id() {
        if active_combat_target {
            if let Some(target) = context.target {
                if target.id == target_id && target_visible {
                    view.target_id = Some(target_id);
                }
            }
        }
    }

    Some(view)
}

fn tank_weapon_range_tiles(entity: &Entity, base_range_tiles: f32) -> f32 {
    let ramp_ticks = TANK_STATIONARY_RANGE_RAMP_TICKS.max(1);
    let ticks = entity
        .combat
        .as_ref()
        .map(|combat| combat.tank_stationary_range_ticks)
        .unwrap_or(0)
        .min(ramp_ticks);
    if ticks == 0 {
        return base_range_tiles;
    }
    let progress = ticks as f32 / ramp_ticks as f32;
    base_range_tiles + (TANK_STATIONARY_RANGE_MAX_TILES - base_range_tiles) * progress
}

fn deconstruct_progress_for_target(target: u32, entities: &EntityStore) -> Option<f32> {
    let required_ticks = tank_trap_deconstruction_ticks();
    let progress = entities
        .iter()
        .filter(|worker| worker.hp > 0 && worker.is_unit())
        .filter(|worker| worker.order().deconstruct_target() == Some(target))
        .filter_map(|worker| worker.deconstruction_progress())
        .max()?;
    let dismantled = (progress as f32 / required_ticks as f32).clamp(0.0, 1.0);
    Some(1.0 - dismantled)
}

fn order_plan(
    entity: &Entity,
    entities: &EntityStore,
    viewer: u32,
    fog: &Fog,
    smokes: Option<&SmokeCloudStore>,
) -> Vec<OrderPlanMarker> {
    let mut plan = Vec::new();
    let mut stage_position = (entity.pos_x, entity.pos_y);
    if let Some(marker) = active_order_plan_marker(entity, entities, viewer, fog, smokes) {
        stage_position = (marker.x, marker.y);
        plan.push(marker);
    }
    plan.extend(entity.queued_orders().iter().filter_map(|intent| {
        let marker = intent_plan_marker(intent, stage_position, entities, viewer, fog, smokes);
        if let Some(marker) = marker.as_ref() {
            stage_position = (marker.x, marker.y);
        }
        marker
    }));
    plan
}

fn active_order_plan_marker(
    entity: &Entity,
    entities: &EntityStore,
    viewer: u32,
    fog: &Fog,
    smokes: Option<&SmokeCloudStore>,
) -> Option<OrderPlanMarker> {
    match entity.order() {
        Order::Move(_) => {
            let (x, y) = entity.path_goal().or_else(|| entity.move_intent())?;
            point_marker("move", x, y)
        }
        Order::AttackMove(_) => {
            let (x, y) = entity.path_goal().or_else(|| entity.move_intent())?;
            point_marker("attackMove", x, y)
        }
        Order::Attack(order) => {
            target_marker("attack", order.intent.target, entities, viewer, fog, smokes)
        }
        Order::Gather(order) => entity_point_marker("gather", order.intent.node, entities),
        Order::Build(order) => {
            build_marker(order.intent.kind, order.intent.tile_x, order.intent.tile_y)
        }
        Order::Deconstruct(order) => target_marker(
            "deconstruct",
            order.intent.target,
            entities,
            viewer,
            fog,
            smokes,
        ),
        Order::Ability(order) => point_marker(
            order.intent.ability.to_protocol_str(),
            order.intent.x,
            order.intent.y,
        ),
        Order::ArtilleryPointFire(order) => point_marker(
            protocol::abilities::POINT_FIRE,
            order.intent.x,
            order.intent.y,
        ),
        Order::ArtilleryBlanketFire(order) => point_marker(
            protocol::abilities::BLANKET_FIRE,
            order.intent.x,
            order.intent.y,
        ),
        Order::Idle | Order::HoldPosition => None,
    }
}

fn intent_plan_marker(
    intent: &OrderIntent,
    stage_position: (f32, f32),
    entities: &EntityStore,
    viewer: u32,
    fog: &Fog,
    smokes: Option<&SmokeCloudStore>,
) -> Option<OrderPlanMarker> {
    match intent {
        OrderIntent::Move(point) => point_marker("move", point.x, point.y),
        OrderIntent::AttackMove(point) => point_marker("attackMove", point.x, point.y),
        OrderIntent::HoldPosition => {
            point_marker("holdPosition", stage_position.0, stage_position.1)
        }
        OrderIntent::Attack(attack) => {
            target_marker("attack", attack.target, entities, viewer, fog, smokes)
        }
        OrderIntent::Gather(gather) => entity_point_marker("gather", gather.node, entities),
        OrderIntent::Build(build) => build_marker(build.kind, build.tile_x, build.tile_y),
        OrderIntent::Deconstruct(intent) => {
            target_marker("deconstruct", intent.target, entities, viewer, fog, smokes)
        }
        OrderIntent::WorldAbility(ability) => {
            point_marker(ability.ability.to_protocol_str(), ability.x, ability.y)
        }
        OrderIntent::PointFire(point) => {
            point_marker(protocol::abilities::POINT_FIRE, point.x, point.y)
        }
        OrderIntent::BlanketFire(point) => {
            point_marker(protocol::abilities::BLANKET_FIRE, point.x, point.y)
        }
        OrderIntent::SelfAbility(ability) => point_marker(
            ability.ability.to_protocol_str(),
            stage_position.0,
            stage_position.1,
        ),
        OrderIntent::SetupAntiTankGuns(point) => {
            point_marker("setupAntiTankGuns", point.x, point.y)
        }
    }
}

fn target_marker(
    kind: &str,
    target: u32,
    entities: &EntityStore,
    viewer: u32,
    fog: &Fog,
    smokes: Option<&SmokeCloudStore>,
) -> Option<OrderPlanMarker> {
    let target = entities.get(target)?;
    let visible = fog.is_visible_world(viewer, target.pos_x, target.pos_y)
        && !smokes
            .map(|smokes| smokes.point_inside(target.pos_x, target.pos_y))
            .unwrap_or(false);
    visible
        .then(|| point_marker(kind, target.pos_x, target.pos_y))
        .flatten()
}

fn entity_point_marker(kind: &str, id: u32, entities: &EntityStore) -> Option<OrderPlanMarker> {
    let entity = entities.get(id)?;
    point_marker(kind, entity.pos_x, entity.pos_y)
}

fn build_marker(kind: EntityKind, tile_x: u32, tile_y: u32) -> Option<OrderPlanMarker> {
    let stats = config::building_stats(kind)?;
    let tile_size = config::TILE_SIZE as f32;
    let x = tile_x as f32 * tile_size + stats.foot_w as f32 * tile_size * 0.5;
    let y = tile_y as f32 * tile_size + stats.foot_h as f32 * tile_size * 0.5;
    point_marker("build", x, y)
}

fn point_marker(kind: &str, x: f32, y: f32) -> Option<OrderPlanMarker> {
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    Some(OrderPlanMarker {
        kind: kind.to_string(),
        x,
        y,
    })
}

fn debug_path_view(entity: &Entity) -> Option<DebugPathView> {
    let movement = entity.movement.as_ref()?;
    if movement.path.is_empty() {
        return None;
    }

    let waypoints = movement
        .path
        .iter()
        .rev()
        .take(MAX_DEBUG_PATH_WAYPOINTS)
        .filter_map(|&(x, y)| debug_path_point(x, y))
        .collect::<Vec<_>>();
    if waypoints.is_empty() {
        return None;
    }

    let goal = movement.path_goal.and_then(|(x, y)| debug_path_point(x, y));
    Some(DebugPathView {
        waypoints,
        goal,
        last_repath_tick: movement.last_repath_tick,
        stuck_ticks: movement.stuck_ticks,
        static_blocked_ticks: movement.static_blocked_ticks,
        total_waypoints: movement.path.len().min(u16::MAX as usize) as u16,
    })
}

fn debug_path_point(x: f32, y: f32) -> Option<DebugPathPoint> {
    (x.is_finite() && y.is_finite()).then_some(DebugPathPoint { x, y })
}

fn active_return_object_id(
    context: &EntityProjectionContext<'_>,
    entity: &Entity,
    ability: ability::AbilityKind,
) -> Option<u32> {
    if ability == ability::AbilityKind::EkatMagicAnchor {
        return context
            .ability_runtime?
            .active_anchor(entity.owner, entity.id, ability, context.tick)
            .map(|object| object.id.get());
    }
    context
        .ability_runtime?
        .active_return_marker(entity.owner, entity.id, ability, None, context.tick)
        .map(|object| object.id.get())
}

fn return_available_tick(
    context: &EntityProjectionContext<'_>,
    entity: &Entity,
    ability: ability::AbilityKind,
) -> Option<u32> {
    match context
        .ability_runtime?
        .active_return_marker(entity.owner, entity.id, ability, None, context.tick)?
        .payload
    {
        AbilityObjectPayload::DashReturn {
            earliest_return_tick,
        } => Some(earliest_return_tick),
        _ => None,
    }
}

fn active_ability_object_expires_in(
    context: &EntityProjectionContext<'_>,
    entity: &Entity,
    ability: ability::AbilityKind,
) -> Option<u16> {
    if ability == ability::AbilityKind::Breakthrough {
        return (entity.breakthrough_aura_ticks() > 0).then_some(entity.breakthrough_aura_ticks());
    }
    if ability == ability::AbilityKind::EkatMagicAnchor {
        return context
            .ability_runtime?
            .active_anchor(entity.owner, entity.id, ability, context.tick)
            .and_then(|object| object.expires_in(context.tick));
    }
    context
        .ability_runtime?
        .active_return_marker(entity.owner, entity.id, ability, None, context.tick)
        .and_then(|object| object.expires_in(context.tick))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{
        tank_trap_deconstruction_ticks, DeconstructPhase, EntityKind, EntityStore, Order,
        OrderIntent,
    };
    use crate::game::map::Map;
    use crate::protocol::terrain;

    fn flat_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![(4, 4)],
            base_sites: Vec::new(),
        }
    }

    fn project_for_test(
        viewer: u32,
        entity: &Entity,
        fog: &Fog,
        fogged: bool,
        entities: &EntityStore,
        target: Option<&Entity>,
        include_debug_path: bool,
    ) -> Option<EntityView> {
        project_for_test_with_debug_projection(
            viewer,
            entity,
            fog,
            fogged,
            entities,
            target,
            if include_debug_path {
                DebugPathProjection::OwnerOnly
            } else {
                DebugPathProjection::None
            },
        )
    }

    fn project_for_test_with_debug_projection(
        viewer: u32,
        entity: &Entity,
        fog: &Fog,
        fogged: bool,
        entities: &EntityStore,
        target: Option<&Entity>,
        debug_path_projection: DebugPathProjection,
    ) -> Option<EntityView> {
        project_for_test_with_active_sites(
            viewer,
            entity,
            fog,
            fogged,
            entities,
            target,
            debug_path_projection,
            None,
        )
    }

    fn project_for_test_with_active_sites(
        viewer: u32,
        entity: &Entity,
        fog: &Fog,
        fogged: bool,
        entities: &EntityStore,
        target: Option<&Entity>,
        debug_path_projection: DebugPathProjection,
        active_construction_sites: Option<&BTreeSet<u32>>,
    ) -> Option<EntityView> {
        project_entity(
            viewer,
            entity,
            EntityProjectionContext {
                fog,
                actionable_fog: Some(fog),
                private_detail_fog: Some(fog),
                private_detail_projection: PrivateDetailProjection::ExactViewer,
                smokes: None,
                fogged,
                entities,
                target,
                debug_path_projection,
                active_construction_sites,
                teams: None,
                owner_faction_id: Some(crate::rules::faction::DEFAULT_FACTION_ID),
                ability_runtime: None,
                tick: 0,
            },
        )
    }

    #[test]
    fn weapon_facing_is_omitted_when_target_direction_is_hidden() {
        let mut entities = EntityStore::new();
        entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("viewer spotter should spawn");
        let tank_id = entities
            .spawn_unit(2, EntityKind::Tank, 120.0, 100.0)
            .expect("tank should spawn");
        let hidden_target_id = entities
            .spawn_unit(3, EntityKind::Rifleman, 700.0, 700.0)
            .expect("hidden target should spawn");
        {
            let tank = entities.get_mut(tank_id).expect("tank should exist");
            tank.set_order(Order::attack(hidden_target_id));
            tank.set_target_id(Some(hidden_target_id));
            tank.set_weapon_facing(1.2);
        }
        let map = Map {
            size: 64,
            terrain: vec![terrain::GRASS; 64 * 64],
            starts: vec![(1, 1)],
            base_sites: Vec::new(),
        };
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2, 3], &entities, &map);
        let tank = entities.get(tank_id).expect("tank should exist");
        let hidden_target = entities
            .get(hidden_target_id)
            .expect("hidden target should exist");

        let enemy_view =
            project_for_test(1, tank, &fog, true, &entities, Some(hidden_target), false)
                .expect("viewer should see nearby tank");
        assert_eq!(enemy_view.target_id, None);
        assert_eq!(enemy_view.weapon_facing, None);

        let owner_view =
            project_for_test(2, tank, &fog, true, &entities, Some(hidden_target), false)
                .expect("owner should see own tank");
        assert_eq!(owner_view.target_id, Some(hidden_target_id));
        assert_eq!(owner_view.weapon_facing, Some(1.2));
    }

    #[test]
    fn moving_tank_projects_visible_turret_target() {
        let mut entities = EntityStore::new();
        entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("viewer spotter should spawn");
        let tank_id = entities
            .spawn_unit(2, EntityKind::Tank, 120.0, 100.0)
            .expect("tank should spawn");
        let target_id = entities
            .spawn_unit(3, EntityKind::Rifleman, 140.0, 100.0)
            .expect("target should spawn");
        {
            let tank = entities.get_mut(tank_id).expect("tank should exist");
            tank.set_order(Order::move_to(300.0, 100.0));
            tank.set_target_id(Some(target_id));
            tank.set_weapon_facing(0.0);
        }
        let map = Map {
            size: 16,
            terrain: vec![terrain::GRASS; 16 * 16],
            starts: vec![(1, 1)],
            base_sites: Vec::new(),
        };
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2, 3], &entities, &map);
        let tank = entities.get(tank_id).expect("tank should exist");
        let target = entities.get(target_id).expect("target should exist");

        let viewer_view = project_for_test(1, tank, &fog, true, &entities, Some(target), false)
            .expect("viewer should see tank");

        assert_eq!(viewer_view.target_id, Some(target_id));
        assert_eq!(viewer_view.weapon_facing, Some(0.0));
    }

    #[test]
    fn idle_combat_unit_projects_visible_acquired_target() {
        let mut entities = EntityStore::new();
        entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("viewer spotter should spawn");
        let gunner_id = entities
            .spawn_unit(2, EntityKind::MachineGunner, 120.0, 100.0)
            .expect("machine gunner should spawn");
        let target_id = entities
            .spawn_unit(3, EntityKind::Rifleman, 140.0, 100.0)
            .expect("target should spawn");
        {
            let gunner = entities.get_mut(gunner_id).expect("gunner should exist");
            gunner.set_target_id(Some(target_id));
            gunner.set_weapon_facing(0.0);
        }
        let map = Map {
            size: 16,
            terrain: vec![terrain::GRASS; 16 * 16],
            starts: vec![(1, 1)],
            base_sites: Vec::new(),
        };
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2, 3], &entities, &map);
        let gunner = entities.get(gunner_id).expect("gunner should exist");
        let target = entities.get(target_id).expect("target should exist");

        let viewer_view = project_for_test(1, gunner, &fog, true, &entities, Some(target), false)
            .expect("viewer should see gunner");

        assert_eq!(viewer_view.state, "idle");
        assert_eq!(viewer_view.target_id, Some(target_id));
        assert_eq!(viewer_view.weapon_facing, Some(0.0));
    }

    #[test]
    fn idle_combat_unit_omits_hidden_acquired_target() {
        let mut entities = EntityStore::new();
        entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("viewer spotter should spawn");
        let gunner_id = entities
            .spawn_unit(2, EntityKind::MachineGunner, 120.0, 100.0)
            .expect("machine gunner should spawn");
        let hidden_target_id = entities
            .spawn_unit(3, EntityKind::Rifleman, 700.0, 700.0)
            .expect("hidden target should spawn");
        {
            let gunner = entities.get_mut(gunner_id).expect("gunner should exist");
            gunner.set_target_id(Some(hidden_target_id));
            gunner.set_weapon_facing(1.2);
        }
        let map = Map {
            size: 64,
            terrain: vec![terrain::GRASS; 64 * 64],
            starts: vec![(1, 1)],
            base_sites: Vec::new(),
        };
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2, 3], &entities, &map);
        let gunner = entities.get(gunner_id).expect("gunner should exist");
        let hidden_target = entities
            .get(hidden_target_id)
            .expect("hidden target should exist");

        let viewer_view =
            project_for_test(1, gunner, &fog, true, &entities, Some(hidden_target), false)
                .expect("viewer should see nearby gunner");

        assert_eq!(viewer_view.state, "idle");
        assert_eq!(viewer_view.target_id, None);
        assert_eq!(viewer_view.weapon_facing, None);
    }

    #[test]
    fn tank_projects_lifetime_oil_used() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 120.0, 100.0)
            .expect("tank should spawn");
        {
            let tank = entities.get_mut(tank_id).expect("tank should exist");
            if let Some(movement) = tank.movement.as_mut() {
                movement.lifetime_oil_used = 3.25;
            }
        }
        let map = Map {
            size: 64,
            terrain: vec![terrain::GRASS; 64 * 64],
            starts: vec![(1, 1)],
            base_sites: Vec::new(),
        };
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1], &entities, &map);
        let tank = entities.get(tank_id).expect("tank should exist");

        let view = project_for_test(1, tank, &fog, true, &entities, None, false)
            .expect("tank should be visible");
        assert_eq!(view.oil_used, Some(3.25));
    }

    #[test]
    fn tank_weapon_range_tiles_are_owner_only() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 120.0, 100.0)
            .expect("tank should spawn");
        entities
            .spawn_unit(2, EntityKind::Rifleman, 100.0, 100.0)
            .expect("enemy spotter should spawn");
        {
            let tank = entities.get_mut(tank_id).expect("tank should exist");
            if let Some(combat) = tank.combat.as_mut() {
                combat.tank_stationary_range_ticks = TANK_STATIONARY_RANGE_RAMP_TICKS / 2;
            }
        }
        let map = Map {
            size: 64,
            terrain: vec![terrain::GRASS; 64 * 64],
            starts: vec![(1, 1)],
            base_sites: Vec::new(),
        };
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2], &entities, &map);
        let tank = entities.get(tank_id).expect("tank should exist");

        let owner_view = project_for_test(1, tank, &fog, true, &entities, None, false)
            .expect("owner should see tank");
        assert_eq!(owner_view.weapon_range_tiles, Some(9.5));

        let enemy_view = project_for_test(2, tank, &fog, true, &entities, None, false)
            .expect("enemy should see nearby tank");
        assert_eq!(enemy_view.weapon_range_tiles, None);
    }

    #[test]
    fn order_plan_is_owner_only_and_projects_safe_stages() {
        let mut entities = EntityStore::new();
        let unit_id = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        let hidden_enemy = entities
            .spawn_unit(2, EntityKind::Rifleman, 700.0, 700.0)
            .expect("enemy should spawn");
        let hidden_node = entities
            .spawn_node(EntityKind::Steel, 720.0, 720.0)
            .expect("node should spawn");
        {
            let unit = entities.get_mut(unit_id).expect("unit should exist");
            unit.set_order(Order::attack_move_to(120.0, 130.0));
            unit.append_queued_order(OrderIntent::move_to(140.0, 160.0));
            unit.append_queued_order(OrderIntent::attack(hidden_enemy));
            unit.append_queued_order(OrderIntent::gather(hidden_node));
            unit.append_queued_order(OrderIntent::attack_move_to(180.0, 200.0));
        }

        let map = Map {
            size: 64,
            terrain: vec![terrain::GRASS; 64 * 64],
            starts: vec![(1, 1), (40, 40)],
            base_sites: Vec::new(),
        };
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2], &entities, &map);
        let unit = entities.get(unit_id).expect("unit should exist");

        let owner_view = project_for_test(1, unit, &fog, true, &entities, None, false)
            .expect("owner should see own unit");
        assert_eq!(
            owner_view.order_plan,
            vec![
                OrderPlanMarker {
                    kind: "attackMove".to_string(),
                    x: 120.0,
                    y: 130.0,
                },
                OrderPlanMarker {
                    kind: "move".to_string(),
                    x: 140.0,
                    y: 160.0,
                },
                OrderPlanMarker {
                    kind: "gather".to_string(),
                    x: 720.0,
                    y: 720.0,
                },
                OrderPlanMarker {
                    kind: "attackMove".to_string(),
                    x: 180.0,
                    y: 200.0,
                },
            ]
        );

        let enemy_view = project_for_test(2, unit, &fog, false, &entities, None, false)
            .expect("full view should include unit");
        assert!(enemy_view.order_plan.is_empty());
    }

    #[test]
    fn active_build_marker_uses_building_footprint_center() {
        let mut entities = EntityStore::new();
        let worker_id = entities
            .spawn_unit(1, EntityKind::Worker, 100.0, 100.0)
            .expect("worker should spawn");
        {
            let worker = entities.get_mut(worker_id).expect("worker should exist");
            worker.set_order(Order::build(EntityKind::Depot, 4, 5));
            worker.append_queued_order(OrderIntent::move_to(320.0, 352.0));
        }

        let map = Map {
            size: 64,
            terrain: vec![terrain::GRASS; 64 * 64],
            starts: vec![(1, 1), (40, 40)],
            base_sites: Vec::new(),
        };
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2], &entities, &map);
        let worker = entities.get(worker_id).expect("worker should exist");

        let owner_view = project_for_test(1, worker, &fog, true, &entities, None, false)
            .expect("owner should see own worker");
        assert_eq!(
            owner_view.order_plan,
            vec![
                OrderPlanMarker {
                    kind: "build".to_string(),
                    x: 160.0,
                    y: 192.0,
                },
                OrderPlanMarker {
                    kind: "move".to_string(),
                    x: 320.0,
                    y: 352.0,
                },
            ]
        );

        let enemy_view = project_for_test(2, worker, &fog, false, &entities, None, false)
            .expect("full view should include worker");
        assert!(enemy_view.order_plan.is_empty());
    }

    #[test]
    fn queued_hold_position_projects_at_the_preceding_stage_destination() {
        let mut entities = EntityStore::new();
        let unit_id = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        {
            let unit = entities.get_mut(unit_id).expect("unit should exist");
            unit.set_order(Order::move_to(120.0, 130.0));
            unit.append_queued_order(OrderIntent::move_to(180.0, 200.0));
            unit.append_queued_order(OrderIntent::hold_position());
        }

        let map = Map {
            size: 64,
            terrain: vec![terrain::GRASS; 64 * 64],
            starts: vec![(1, 1)],
            base_sites: Vec::new(),
        };
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1], &entities, &map);
        let unit = entities.get(unit_id).expect("unit should exist");

        let owner_view = project_for_test(1, unit, &fog, true, &entities, None, false)
            .expect("owner should see own unit");
        assert_eq!(
            owner_view.order_plan,
            vec![
                OrderPlanMarker {
                    kind: "move".to_string(),
                    x: 120.0,
                    y: 130.0,
                },
                OrderPlanMarker {
                    kind: "move".to_string(),
                    x: 180.0,
                    y: 200.0,
                },
                OrderPlanMarker {
                    kind: "holdPosition".to_string(),
                    x: 180.0,
                    y: 200.0,
                },
            ]
        );
    }

    #[test]
    fn debug_path_is_owner_only_and_in_movement_order() {
        let mut entities = EntityStore::new();
        let unit_id = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        {
            let unit = entities.get_mut(unit_id).expect("unit should exist");
            unit.set_order(Order::move_to(300.0, 300.0));
            unit.set_path(vec![(300.0, 300.0), (200.0, 200.0), (120.0, 120.0)]);
            unit.set_path_goal(Some((300.0, 300.0)));
            unit.set_last_repath_tick(7);
            if let Some(movement) = unit.movement.as_mut() {
                movement.stuck_ticks = 2;
                movement.static_blocked_ticks = 3;
            }
        }

        let map = Map {
            size: 64,
            terrain: vec![terrain::GRASS; 64 * 64],
            starts: vec![(1, 1), (40, 40)],
            base_sites: Vec::new(),
        };
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2], &entities, &map);
        let unit = entities.get(unit_id).expect("unit should exist");

        let standard_owner_view = project_for_test(1, unit, &fog, true, &entities, None, false)
            .expect("owner should see own unit");
        assert_eq!(standard_owner_view.debug_path, None);

        let owner_view = project_for_test(1, unit, &fog, true, &entities, None, true)
            .expect("owner should see own unit");
        let debug_path = owner_view
            .debug_path
            .expect("moving own unit should expose debug path when runtime debug mode is enabled");
        assert_eq!(
            debug_path.waypoints,
            vec![
                DebugPathPoint { x: 120.0, y: 120.0 },
                DebugPathPoint { x: 200.0, y: 200.0 },
                DebugPathPoint { x: 300.0, y: 300.0 },
            ]
        );
        assert_eq!(debug_path.goal, Some(DebugPathPoint { x: 300.0, y: 300.0 }));
        assert_eq!(debug_path.last_repath_tick, 7);
        assert_eq!(debug_path.stuck_ticks, 2);
        assert_eq!(debug_path.static_blocked_ticks, 3);
        assert_eq!(debug_path.total_waypoints, 3);

        let enemy_view = project_for_test(2, unit, &fog, false, &entities, None, true)
            .expect("full view should include unit");
        assert_eq!(enemy_view.debug_path, None);

        let full_world_view = project_for_test_with_debug_projection(
            2,
            unit,
            &fog,
            false,
            &entities,
            None,
            DebugPathProjection::AllProjected,
        )
        .expect("full-world diagnostics should include projected units");
        assert!(
            full_world_view.debug_path.is_some(),
            "full-world diagnostic policy may expose movement paths for every projected entity"
        );
    }

    #[test]
    fn legacy_charge_cooldown_is_not_projected() {
        let mut entities = EntityStore::new();
        let rifle_id = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        entities
            .get_mut(rifle_id)
            .expect("rifleman should exist")
            .start_ability_cooldown(ability::AbilityKind::Charge, 42);

        let map = Map {
            size: 64,
            terrain: vec![terrain::GRASS; 64 * 64],
            starts: vec![(1, 1), (40, 40)],
            base_sites: Vec::new(),
        };
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2], &entities, &map);
        let rifle = entities.get(rifle_id).expect("rifleman should exist");

        let owner_view = project_for_test(1, rifle, &fog, true, &entities, None, false)
            .expect("owner should see own rifleman");
        assert_eq!(owner_view.charge_cooldown_left, None);

        let enemy_view = project_for_test(2, rifle, &fog, false, &entities, None, false)
            .expect("full view should include rifleman");
        assert_eq!(enemy_view.charge_cooldown_left, None);
    }

    #[test]
    fn construction_active_signal_is_owner_only() {
        let mut entities = EntityStore::new();
        let scaffold_id = entities
            .spawn_building(1, EntityKind::Barracks, 160.0, 160.0, false)
            .expect("scaffold should spawn");
        let scaffold = entities.get(scaffold_id).expect("scaffold should exist");
        let mut fog = Fog::new(16);
        fog.recompute(&[1, 2], &entities, &flat_map(16));
        let mut active_sites = BTreeSet::new();
        active_sites.insert(scaffold_id);

        let owner_view = project_for_test_with_active_sites(
            1,
            scaffold,
            &fog,
            true,
            &entities,
            None,
            DebugPathProjection::None,
            Some(&active_sites),
        )
        .expect("owner should see scaffold");
        assert_eq!(owner_view.build_progress, Some(0.0));
        assert!(owner_view.build_active);

        let enemy_view = project_for_test_with_active_sites(
            2,
            scaffold,
            &fog,
            false,
            &entities,
            None,
            DebugPathProjection::None,
            Some(&active_sites),
        )
        .expect("non-owner visible scaffold should project");
        assert_eq!(enemy_view.build_progress, Some(0.0));
        assert!(!enemy_view.build_active);
    }

    #[test]
    fn tank_trap_deconstruction_projects_reverse_progress() {
        let mut entities = EntityStore::new();
        let trap_id = entities
            .spawn_building(2, EntityKind::TankTrap, 160.0, 160.0, true)
            .expect("tank trap should spawn");
        let worker_id = entities
            .spawn_unit(1, EntityKind::Worker, 192.0, 160.0)
            .expect("worker should spawn");
        {
            let worker = entities.get_mut(worker_id).expect("worker should exist");
            worker.set_order(Order::deconstruct(trap_id));
            worker.mark_deconstruct_phase(DeconstructPhase::Deconstructing);
            for _ in 0..(tank_trap_deconstruction_ticks() / 2) {
                worker.tick_deconstruction();
            }
        }

        let fog = Fog::new(16);
        let trap = entities.get(trap_id).expect("trap should exist");
        let view = project_for_test(1, trap, &fog, false, &entities, None, false)
            .expect("viewer should see trap");

        assert_eq!(view.build_progress, None);
        assert!(
            (view
                .deconstruct_progress
                .expect("deconstruct progress should project")
                - 0.5)
                .abs()
                < 0.001,
            "deconstruction progress should be the remaining reverse fraction"
        );
    }
}
