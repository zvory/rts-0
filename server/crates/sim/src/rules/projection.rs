//! Projection rules for fog-gated entity views and event delivery.
//!
//! This module owns what a player is allowed to see. It does not mutate the world; future
//! last-known-position or partial-reveal rules should grow here.

use std::collections::BTreeSet;

use crate::config;
use crate::game::ability;
use crate::game::ability_runtime::AbilityRuntime;
use crate::game::entity::{
    active_trench_occupation, fires_while_moving, supports_manual_emplacement,
    tank_trap_deconstruction_ticks, Entity, EntityKind, EntityStore, GatherPhase, Order,
    OrderIntent,
};
use crate::game::fog::Fog;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::protocol;
use crate::protocol::{AbilityCooldownView, DebugPathPoint, DebugPathView};
use crate::protocol::{EntityView, OrderPlanMarker};

use super::projection_abilities::{
    active_ability_object_expires_in, active_return_object_id, return_available_tick, team_cooldown,
};
use super::projection_panzerfaust::project_panzerfaust_state;
pub(crate) use super::projection_visibility::entity_hidden_by_concealment_from_team;
pub(crate) use super::projection_visibility::shot_reveals_attacker;
#[allow(unused_imports)]
pub use super::projection_visibility::{
    attack_event_visible_to, attack_event_visible_to_team, attack_event_visible_to_with_smoke,
    event_visible_to, event_visible_to_team, event_visible_to_team_with_smoke,
    event_visible_to_with_smoke, team_visible_world,
};

const MAX_DEBUG_PATH_WAYPOINTS: usize = 128;
const TANK_STATIONARY_RANGE_MAX_TILES: f32 = 14.0;
const TANK_STATIONARY_RANGE_RAMP_TICKS: u16 = config::TICK_HZ as u16 * 3;

pub struct EntityProjectionContext<'a> {
    pub map: Option<&'a crate::game::map::Map>,
    pub fog: &'a Fog,
    pub actionable_fog: Option<&'a Fog>,
    pub private_detail_fog: Option<&'a Fog>,
    /// Raw per-player fog retains firing-reveal provenance that team union views flatten.
    pub concealment_fog: Option<&'a Fog>,
    pub private_detail_projection: PrivateDetailProjection<'a>,
    pub smokes: Option<&'a SmokeCloudStore>,
    pub fogged: bool,
    pub entities: &'a EntityStore,
    pub target: Option<&'a Entity>,
    pub debug_path_projection: DebugPathProjection,
    pub active_construction_sites: Option<&'a BTreeSet<u32>>,
    pub extractor_active: Option<bool>,
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
pub(crate) enum PrivateDetailProjection<'a> {
    ExactViewer,
    SelectedOwners(&'a [u32]),
    AllProjected,
}

impl PrivateDetailProjection<'_> {
    fn viewer_for(self, viewer: u32, entity: &Entity) -> Option<u32> {
        match self {
            Self::ExactViewer => (entity.owner == viewer).then_some(viewer),
            Self::SelectedOwners(player_ids) => {
                player_ids.contains(&entity.owner).then_some(entity.owner)
            }
            Self::AllProjected => (entity.owner != 0).then_some(entity.owner),
        }
    }

    fn includes_owner(self, viewer: u32, owner: u32) -> bool {
        match self {
            Self::ExactViewer => owner == viewer,
            Self::SelectedOwners(player_ids) => player_ids.contains(&owner),
            Self::AllProjected => owner != 0,
        }
    }

    fn includes_owner_or_ally(
        self,
        viewer: u32,
        owner: u32,
        teams: Option<&TeamRelations>,
    ) -> bool {
        match self {
            Self::ExactViewer => teams
                .map(|teams| teams.same_team_or_same_owner(viewer, owner))
                .unwrap_or(owner == viewer),
            Self::SelectedOwners(player_ids) => player_ids.iter().any(|selected| {
                teams
                    .map(|teams| teams.same_team_or_same_owner(*selected, owner))
                    .unwrap_or(owner == *selected)
            }),
            Self::AllProjected => owner != 0,
        }
    }
}

pub fn entity_visible_to(viewer: u32, entity: &Entity, fog: &Fog) -> bool {
    (entity.owner != 0 && entity.owner == viewer)
        || entity.is_node()
        || fog.is_visible_world(viewer, entity.pos_x(), entity.pos_y())
}

pub fn entity_visible_to_with_smoke(
    viewer: u32,
    entity: &Entity,
    fog: &Fog,
    smokes: &SmokeCloudStore,
) -> bool {
    if entity.owner != viewer
        && !entity.is_node()
        && smokes.point_inside(entity.pos_x(), entity.pos_y())
    {
        return false;
    }
    entity_visible_to(viewer, entity, fog)
}

pub(crate) fn entity_visible_to_with_smoke_melee(
    viewer: u32,
    entity: &Entity,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    entities: &EntityStore,
    teams: Option<&TeamRelations>,
    private_detail_projection: PrivateDetailProjection<'_>,
) -> bool {
    if entity_visible_to_with_smoke(viewer, entity, fog, smokes) {
        return true;
    }
    if !fog.is_visible_world(viewer, entity.pos_x(), entity.pos_y()) {
        return false;
    }
    entities.iter().any(|spotter| {
        private_detail_projection.includes_owner_or_ally(viewer, spotter.owner, teams)
            && smokes.units_have_melee_visibility(spotter, entity)
    })
}

pub fn project_entity(
    viewer: u32,
    entity: &Entity,
    context: EntityProjectionContext<'_>,
) -> Option<EntityView> {
    let selected_owner = context
        .private_detail_projection
        .includes_owner(viewer, entity.owner);
    let owner_or_ally = context.private_detail_projection.includes_owner_or_ally(
        viewer,
        entity.owner,
        context.teams,
    );
    let concealment_fog = context.concealment_fog.unwrap_or(context.fog);
    let hidden_by_concealment = context.map.zip(context.teams).is_some_and(|(map, teams)| {
        context
            .private_detail_projection
            .entity_hidden_by_concealment(viewer, entity, map, concealment_fog, teams)
    });
    if context.fogged
        && !selected_owner
        && (hidden_by_concealment
            || !context
                .smokes
                .map(|smokes| {
                    entity_visible_to_with_smoke_melee(
                        viewer,
                        entity,
                        context.fog,
                        smokes,
                        context.entities,
                        context.teams,
                        context.private_detail_projection,
                    )
                })
                .unwrap_or_else(|| entity_visible_to(viewer, entity, context.fog)))
    {
        return None;
    }

    let mut view = EntityView::new(
        entity.id,
        entity.owner,
        protocol::kind_to_wire(entity.kind),
        entity.pos_x(),
        entity.pos_y(),
        entity.hp,
        entity.max_hp,
        entity.state_str(),
    );
    let actionable_fog = context.actionable_fog.unwrap_or(context.fog);
    let private_detail_fog = context.private_detail_fog.unwrap_or(actionable_fog);
    let private_detail_viewer = context.private_detail_projection.viewer_for(viewer, entity);
    let private_detail_owner = private_detail_viewer.is_some();
    let exact_owner = private_detail_owner;
    let concealment_reveal_only = context.fogged
        && !owner_or_ally
        && entity.is_unit()
        && context
            .map
            .is_some_and(|map| map.world_point_is_concealed(entity.pos_x(), entity.pos_y()))
        && !context.teams.is_some_and(|teams| {
            context.private_detail_projection.has_concealment_detection(
                viewer,
                entity.id,
                concealment_fog,
                teams,
            )
        });
    let vision_only = concealment_reveal_only
        || (context.fogged
            && !owner_or_ally
            && !entity.is_node()
            && !context
                .smokes
                .map(|smokes| {
                    entity_visible_to_with_smoke_melee(
                        viewer,
                        entity,
                        actionable_fog,
                        smokes,
                        context.entities,
                        context.teams,
                        context.private_detail_projection,
                    )
                })
                .unwrap_or_else(|| entity_visible_to(viewer, entity, actionable_fog)));
    view.vision_only = vision_only;

    if entity.is_unit() {
        view.units_killed = Some(entity.units_killed());
    }

    if entity.kind.is_resource_extractor() && !entity.under_construction() {
        view.extractor_active = context.extractor_active;
    }

    if entity.is_unit() {
        view.facing = Some(entity.facing());
    }
    if entity.kind == EntityKind::Tank && owner_or_ally {
        if let Some(stats) = config::unit_stats(entity.kind) {
            view.weapon_range_tiles = Some(tank_weapon_range_tiles(entity, stats.range_tiles));
        }
    }
    project_panzerfaust_state(entity, &mut view);
    if entity.kind == EntityKind::RocketLauncher {
        view.rocket_rack_count = Some(config::rocket_rack_count(
            entity.ability_cooldown_ticks(ability::AbilityKind::Barrage),
        ));
    }
    let acquired_combat_target = entity.can_attack() && entity.target_id().is_some();
    let active_combat_target =
        matches!(entity.order(), Order::Attack(_) | Order::AttackMove(_)) || acquired_combat_target;
    let combat_target_visible = |target: &Entity| {
        exact_owner
            || !context.fogged
            || (!vision_only
                && !context.map.zip(context.teams).is_some_and(|(map, teams)| {
                    context
                        .private_detail_projection
                        .entity_hidden_by_concealment(viewer, target, map, concealment_fog, teams)
                })
                && context
                    .smokes
                    .map(|smokes| {
                        entity_visible_to_with_smoke_melee(
                            viewer,
                            target,
                            actionable_fog,
                            smokes,
                            context.entities,
                            context.teams,
                            context.private_detail_projection,
                        )
                    })
                    .unwrap_or_else(|| {
                        actionable_fog.is_visible_world(viewer, target.pos_x(), target.pos_y())
                    }))
    };
    let target_visible = entity
        .target_id()
        .and_then(|id| context.target.filter(|t| t.id == id))
        .is_some_and(combat_target_visible);
    let weapon_facing_is_safe = exact_owner
        || !context.fogged
        || entity.target_id().is_none()
        || !active_combat_target
        || target_visible;
    let commander_target_visible = entity
        .combat
        .as_ref()
        .and_then(|c| c.commander_target_id)
        .and_then(|id| context.entities.get(id))
        .is_some_and(combat_target_visible);
    if entity.kind == EntityKind::Tank && !vision_only {
        if let Some(combat) = entity.combat.as_ref() {
            if exact_owner
                || !context.fogged
                // Idle MG aim follows the cannon, so it inherits the cannon's fog gate.
                || (combat.commander_target_id.is_none() && weapon_facing_is_safe)
                || commander_target_visible
            {
                view.commander_facing = Some(combat.commander_facing);
            }
        }
    }
    let weapon_facing_useful = fires_while_moving(entity.kind)
        || active_combat_target
        || (entity.is_building() && entity.can_attack());
    if weapon_facing_useful {
        if let Some(weapon_facing) = entity.weapon_facing() {
            if weapon_facing_is_safe {
                view.weapon_facing = Some(weapon_facing);
            }
        }
    }
    if entity.kind == EntityKind::MachineGunner || supports_manual_emplacement(entity.kind) {
        view.setup_state = Some(entity.weapon_setup().to_protocol_str().to_string());
    }
    // A deployed enemy AT gun's setup direction is already exposed by its visible warning cone.
    // Project that fixed direction with the live entity so clients never need to infer the cone
    // from the dynamically tracking body or weapon facing. Other setup-capable units retain the
    // owner/allied-detail boundary.
    let public_deployed_anti_tank_facing = entity.kind == EntityKind::AntiTankGun
        && entity.weapon_setup() == crate::game::entity::WeaponSetup::Deployed
        && !vision_only;
    if supports_manual_emplacement(entity.kind)
        && (owner_or_ally || public_deployed_anti_tank_facing)
    {
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
            view.prod_upgrade_queue = entity
                .research_queue()
                .iter()
                .map(|item| item.upgrade.to_protocol_str().to_string())
                .collect();
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
                building_kind: None,
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
                    catalog.ability(**kind).is_some_and(|entry| {
                        entry.command_card && entry.carriers.contains(&entity.kind)
                    })
                })
            })
            .map(|(kind, cooldown_left)| AbilityCooldownView {
                ability: kind.to_protocol_str().to_string(),
                cooldown_left: *cooldown_left,
                remaining_uses: entity.ability_uses_remaining(*kind),
                autocast_enabled: None,
                active_object_id: active_return_object_id(&context, entity, *kind),
                available_tick: return_available_tick(&context, entity, *kind),
                lockout_until_tick: entity.ability_lockout_until_tick(*kind, context.tick),
                expires_in: active_ability_object_expires_in(&context, entity, *kind),
                charge_recharge_left: entity.ability_charge_recharge_ticks(*kind),
            })
            .collect();
        for entry in catalog
            .into_iter()
            .flat_map(|catalog| catalog.abilities_for_carrier(entity.kind))
            .filter(|entry| entry.command_card)
        {
            let kind = entry.kind;
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
                    autocast_enabled: None,
                    active_object_id: active_return_object_id(&context, entity, kind),
                    available_tick: return_available_tick(&context, entity, kind),
                    lockout_until_tick: entity.ability_lockout_until_tick(kind, context.tick),
                    expires_in: active_ability_object_expires_in(&context, entity, kind),
                    charge_recharge_left: entity.ability_charge_recharge_ticks(kind),
                });
            }
        }
    }

    // Share only Mortar Fire's timer with allies, never other private details.
    if private_detail_viewer.is_none() && entity.kind == EntityKind::MortarTeam && owner_or_ally {
        view.abilities
            .push(team_cooldown(entity, ability::AbilityKind::MortarFire));
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
    let mut stage_position = (entity.pos_x(), entity.pos_y());
    if let Some(marker) = active_order_plan_marker(entity, entities, viewer, fog, smokes) {
        stage_position = (marker.x, marker.y);
        plan.push(marker);
    }
    plan.extend(entity.queued_orders().iter().filter_map(|intent| {
        let marker = if entity.kind == EntityKind::MortarTeam
            && matches!(intent, OrderIntent::SetupAntiTankGuns(_))
        {
            point_marker("setupAntiTankGuns", stage_position.0, stage_position.1)
        } else {
            intent_plan_marker(intent, stage_position, entities, viewer, fog, smokes)
        };
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
        Order::ArtilleryBlanketFire { order, .. } => point_marker(
            protocol::abilities::BLANKET_FIRE,
            order.intent.x,
            order.intent.y,
        ),
        Order::Idle | Order::HoldPosition | Order::Point { .. } => None,
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
        OrderIntent::Point { .. } => None,
        OrderIntent::Move(point) => point_marker("move", point.x, point.y),
        OrderIntent::AttackMove(point) => point_marker("attackMove", point.x, point.y),
        OrderIntent::ClearObstacleArea(intent) => {
            point_marker("attackMove", intent.center_x, intent.center_y)
        }
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
        OrderIntent::BlanketFire { point, .. } => {
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
    let visible = fog.is_visible_world(viewer, target.pos_x(), target.pos_y())
        && !smokes
            .map(|smokes| smokes.point_inside(target.pos_x(), target.pos_y()))
            .unwrap_or(false);
    visible
        .then(|| point_marker(kind, target.pos_x(), target.pos_y()))
        .flatten()
}

fn entity_point_marker(kind: &str, id: u32, entities: &EntityStore) -> Option<OrderPlanMarker> {
    let entity = entities.get(id)?;
    point_marker(kind, entity.pos_x(), entity.pos_y())
}

fn build_marker(kind: EntityKind, tile_x: u32, tile_y: u32) -> Option<OrderPlanMarker> {
    let stats = config::building_stats(kind)?;
    let tile_size = config::TILE_SIZE as f32;
    let x = tile_x as f32 * tile_size + stats.foot_w as f32 * tile_size * 0.5;
    let y = tile_y as f32 * tile_size + stats.foot_h as f32 * tile_size * 0.5;
    point_marker("build", x, y).map(|mut marker| {
        marker.building_kind = Some(kind.stable_id().to_string());
        marker
    })
}

fn point_marker(kind: &str, x: f32, y: f32) -> Option<OrderPlanMarker> {
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    Some(OrderPlanMarker {
        kind: kind.to_string(),
        x,
        y,
        building_kind: None,
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

#[cfg(test)]
mod tests;
