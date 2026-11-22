//! Tournament admission and winner specialization. Durable state stays on entity state groups.
use crate::config;
use crate::game::ability::AbilityKind;
use crate::game::entity::{EntityKind, EntityStore, ProdItem};
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::upgrade::UpgradeKind;
use crate::game::PlayerState;
use rand::Rng;

pub(crate) const CAPACITY: u8 = 4;
pub(crate) const DURATION: u32 = 150;

/// A contextual move onto the exact center of an owned, complete arena means enter.
/// Ordinary move commands and replay command recording retain their existing wire vocabulary.
pub(super) fn entry_target(entities: &EntityStore, owner: u32, x: f32, y: f32) -> Option<u32> {
    entities
        .iter()
        .find(|e| {
            matches!(
                e.kind,
                EntityKind::TournamentGrounds | EntityKind::ProvenGrounds
            ) && e.owner == owner
                && e.hp > 0
                && !e.under_construction()
                && e.pos_x() == x
                && e.pos_y() == y
        })
        .map(|e| e.id)
}

pub(super) fn order_entry(
    entities: &mut EntityStore,
    coordinator: &MoveCoordinator<'_>,
    owner: u32,
    units: &[u32],
    target: u32,
) {
    for id in units {
        let Some(unit) = entities.get(*id).filter(|e| {
            e.owner == owner && e.kind == EntityKind::Warrior && e.dao.is_none() && e.hp > 0
        }) else {
            continue;
        };
        let approach = (unit.pos_x(), unit.pos_y());
        let point =
            coordinator.find_spawn_point(entities, target, EntityKind::Warrior, Some(approach));
        if let Some(e) = entities.get_mut(*id) {
            e.clear_orders();
            if let Some(point) = point {
                e.replace_active_order(crate::game::entity::Order::move_to(point.0, point.1));
            }
            e.set_path_goal(point);
            e.reset_stuck(approach.0, approach.1);
            if let Some(m) = e.movement.as_mut() {
                m.tournament_target = Some(target);
            }
        }
    }
}

/// Admit at the physical perimeter before movement can finish/clear an arrival order.
pub(in crate::game) fn advance(
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    rng: &mut impl Rng,
    coordinator: &MoveCoordinator<'_>,
) {
    for id in entities.ids() {
        let Some((owner, target, x, y, eligible)) = entities.get(id).and_then(|e| {
            Some((
                e.owner,
                e.movement.as_ref()?.tournament_target?,
                e.pos_x(),
                e.pos_y(),
                e.hp > 0 && e.kind == EntityKind::Warrior && e.dao.is_none(),
            ))
        }) else {
            continue;
        };
        let Some(building) = entities.get(target).filter(|e| {
            eligible
                && e.owner == owner
                && e.hp > 0
                && matches!(
                    e.kind,
                    EntityKind::TournamentGrounds | EntityKind::ProvenGrounds
                )
                && !e.under_construction()
        }) else {
            if let Some(e) = entities.get_mut(id) {
                e.clear_orders();
            }
            continue;
        };
        let Some(p) = building.production.as_ref() else {
            continue;
        };
        if !p.queue.is_empty() || p.tournament_entrants >= CAPACITY {
            // Portal rallies keep their entry intent while this tournament is running.
            // Units stage outside and enter automatically when the next intake opens.
            continue;
        }
        // Rectangle distance, not center distance: corner approaches must reach the perimeter too.
        let Some(stats) = config::building_stats(building.kind) else {
            continue;
        };
        let half_w = config::TILE_SIZE as f32 * stats.foot_w as f32 * 0.5;
        let half_h = config::TILE_SIZE as f32 * stats.foot_h as f32 * 0.5;
        let dx = ((x - building.pos_x()).abs() - half_w).max(0.0);
        let dy = ((y - building.pos_y()).abs() - half_h).max(0.0);
        if dx * dx + dy * dy > 24.0 * 24.0 {
            // Spawn search may stage outside the admission radius when the perimeter is
            // crowded. Reconsider an exhausted approach after those blockers move away;
            // retain entry intent even when no legal staging point currently exists.
            if entities.get(id).is_some_and(|e| e.path_is_empty()) {
                order_entry(entities, coordinator, owner, &[id], target);
            }
            continue;
        }
        if building.kind == EntityKind::ProvenGrounds {
            let Some(choice) = p.dao_training_queue.first().copied() else {
                continue;
            };
            let Some(player) = players.iter_mut().find(|p| {
                p.id == owner
                    && p.faction_id == crate::rules::faction::CULTIVATORS_FACTION_ID
                    && p.has_upgrade(choice)
            }) else {
                continue;
            };
            if player.spend_cost(crate::rules::economy::ResourceCost::new(
                0,
                config::DAO_TRAINING_COST_OIL,
            )) {
                super::dao_training::admit(entities, target, id);
            }
            continue;
        }
        entities.remove(id);
        if let Some(player) = players.iter_mut().find(|p| p.id == owner) {
            player.release_supply(crate::rules::economy::supply_cost(EntityKind::Warrior));
        }
        if let Some(p) = entities.get_mut(target).and_then(|e| e.production.as_mut()) {
            p.tournament_entrants = p.tournament_entrants.saturating_add(1);
            if p.tournament_entrants == CAPACITY {
                p.tournament_winners = vec![match rng.gen_range(0..3) {
                    0 => UpgradeKind::DaoOfLightness,
                    1 => UpgradeKind::DaoOfHeaviness,
                    _ => UpgradeKind::DaoOfMagic,
                }];
                p.queue.push(ProdItem {
                    unit: EntityKind::Warrior,
                    progress: 0,
                    total: DURATION,
                    paid: true,
                });
                if let Some(player) = players.iter_mut().find(|p| p.id == owner) {
                    // Four consumed Disciples free eight supply. The winner reservation
                    // persist in paid production items and are rebuilt by the supply system.
                    player.reserve_supply(2);
                }
            }
        }
    }
}

pub(super) fn upgrade_winner(entities: &mut EntityStore, building: u32, winner: u32) {
    let choice = entities
        .get_mut(building)
        .filter(|e| e.kind == EntityKind::TournamentGrounds)
        .and_then(|e| e.production.as_mut())
        .and_then(|p| {
            let choice = p.tournament_winners.pop();
            if p.tournament_winners.is_empty() {
                p.tournament_entrants = 0;
            }
            choice
        });
    let Some(choice) = choice else {
        return;
    };
    if let Some(e) = entities.get_mut(winner) {
        e.insight = config::DISCIPLE_INSIGHT_REQUIRED;
        e.evolve_dao(match choice {
            UpgradeKind::DaoOfLightness => AbilityKind::EvolveLightness,
            UpgradeKind::DaoOfHeaviness => AbilityKind::EvolveWar,
            _ => AbilityKind::EvolveMagic,
        });
    }
}
