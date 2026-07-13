use crate::game::ability::{self, AbilityKind, AbilityQueuePolicy};
use crate::game::entity::{Entity, EntityKind, EntityStore, Order, OrderIntent, MAX_QUEUED_ORDERS};
use crate::game::map::Map;
use crate::game::services::ability_orders;
use crate::game::services::order_planner as planner;
use crate::rules;

use super::guards::{dedupe_cap_units, unit_can_accept_ground_command};
use super::{ability_from_planner, build_kind_from_code};

pub(super) fn planner_config(max_units_per_command: usize) -> planner::PlannerConfig {
    planner::PlannerConfig {
        max_units_per_command,
        max_queue_len: MAX_QUEUED_ORDERS,
    }
}

pub(super) fn entity_order_intent_from_planner(
    intent: planner::OrderIntent,
) -> Option<OrderIntent> {
    match intent {
        planner::OrderIntent::Move(point) => Some(OrderIntent::move_to(point.x, point.y)),
        planner::OrderIntent::AttackMove(point) => {
            Some(OrderIntent::attack_move_to(point.x, point.y))
        }
        planner::OrderIntent::HoldPosition => Some(OrderIntent::hold_position()),
        planner::OrderIntent::AttackTarget(target) => Some(OrderIntent::attack(target)),
        planner::OrderIntent::Gather(node) => Some(OrderIntent::gather(node)),
        planner::OrderIntent::Deconstruct(target) => Some(OrderIntent::deconstruct(target)),
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

pub(super) fn planner_facts(
    entities: &EntityStore,
    player: u32,
    faction_id: &str,
    units: &[u32],
    ability: Option<AbilityFactInput<'_>>,
    max_units_per_command: usize,
) -> Vec<planner::UnitFacts> {
    dedupe_cap_units(units.to_vec(), max_units_per_command)
        .into_iter()
        .filter_map(|id| {
            let e = entities.get(id)?;
            if !e.is_unit() || e.owner != player {
                return None;
            }
            let mut facts = planner::UnitFacts::new(id);
            facts.pos = planner::Point::new(e.pos_x, e.pos_y);
            facts.queue_len = e.queued_orders().len();
            facts.queue_terminal = matches!(
                e.order(),
                Order::ArtilleryPointFire(_) | Order::ArtilleryBlanketFire(_)
            ) || e.queued_orders().iter().any(|intent| {
                matches!(
                    intent,
                    OrderIntent::PointFire(_)
                        | OrderIntent::BlanketFire(_)
                        | OrderIntent::HoldPosition
                )
            });
            facts.active_build = matches!(e.order(), Order::Build(_) | Order::Deconstruct(_));
            facts.activity = match e.order() {
                Order::Idle | Order::HoldPosition => planner::UnitActivity::Idle,
                Order::Move(_) | Order::AttackMove(_) | Order::Ability(_) => {
                    planner::UnitActivity::Moving
                }
                _ => planner::UnitActivity::Busy,
            };
            facts.can_attack_move = e.kind != EntityKind::ScoutPlane;
            facts.can_attack = e.can_attack();
            facts.can_hold_position = unit_can_accept_ground_command(entities, player, id);
            facts.can_gather = rules::economy::can_gather_for_faction(faction_id, e.kind);
            facts.can_build = rules::faction::catalog_for(faction_id)
                .is_some_and(|catalog| catalog.builders.contains(&e.kind));
            facts.can_setup_anti_tank_gun =
                matches!(e.kind, EntityKind::AntiTankGun | EntityKind::Artillery);
            if let Some(ability) = ability {
                let ready_at_issue =
                    ability_orders::caster_can_accept_order(entities, player, id, ability.kind);
                let queue_admissible_at_issue =
                    queue_admissible_at_issue(entities, player, id, ability.kind, ready_at_issue);
                if (ready_at_issue || queue_admissible_at_issue)
                    && ability_orders::caster_allowed_by_faction(
                        entities,
                        faction_id,
                        id,
                        ability.kind,
                    )
                    && ability.tech_ready
                    && has_unreserved_ability_use(e, ability.kind)
                {
                    facts.abilities.push(planner::AbilityFacts {
                        ability: ability.id,
                        ready_at_issue,
                        queue_admissible_at_issue,
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

fn has_unreserved_ability_use(entity: &Entity, ability: AbilityKind) -> bool {
    match entity.ability_uses_remaining(ability) {
        Some(remaining) => remaining as usize > reserved_ability_uses(entity, ability),
        None => true,
    }
}

fn queue_admissible_at_issue(
    entities: &EntityStore,
    player: u32,
    id: u32,
    ability: AbilityKind,
    ready_at_issue: bool,
) -> bool {
    match ability::definition(ability).queue_policy {
        AbilityQueuePolicy::NotQueueable => false,
        AbilityQueuePolicy::QueueSkipIfNotReady => ready_at_issue,
        AbilityQueuePolicy::QueueWaitUntilReady => {
            ability_orders::caster_can_accept_waiting_order(entities, player, id, ability)
        }
    }
}

fn reserved_ability_uses(entity: &Entity, ability: AbilityKind) -> usize {
    let active = matches!(
        entity.order(),
        Order::Ability(order) if order.intent.ability == ability
    ) as usize;
    let queued = entity
        .queued_orders()
        .iter()
        .filter(|intent| match intent {
            OrderIntent::WorldAbility(intent) => intent.ability == ability,
            OrderIntent::SelfAbility(intent) => intent.ability == ability,
            _ => false,
        })
        .count();
    active.saturating_add(queued)
}

#[derive(Clone, Copy)]
pub(super) struct AbilityFactInput<'a> {
    pub(super) kind: AbilityKind,
    pub(super) id: planner::AbilityId,
    pub(super) tech_ready: bool,
    pub(super) target: Option<(f32, f32)>,
    pub(super) map: &'a Map,
}

fn world_ability_can_execute_without_interrupt(ability: AbilityKind) -> bool {
    matches!(
        ability,
        AbilityKind::Smoke
            | AbilityKind::EkatTeleport
            | AbilityKind::EkatLineShot
            | AbilityKind::EkatMagicAnchor
    )
}

fn world_ability_may_interrupt_active_order(ability: AbilityKind) -> bool {
    matches!(
        ability,
        AbilityKind::MortarFire
            | AbilityKind::EkatTeleport
            | AbilityKind::EkatLineShot
            | AbilityKind::EkatMagicAnchor
    )
}
