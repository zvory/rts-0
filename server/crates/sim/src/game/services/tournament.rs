//! Tournament admission and winner specialization. Durable state stays on entity state groups.
use crate::config;
use crate::game::ability::AbilityKind;
use crate::game::entity::{EntityKind, EntityStore, ProdItem};
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::upgrade::UpgradeKind;
use crate::game::PlayerState;
use rand::Rng;

pub(crate) const CAPACITY: u8 = 8;
pub(crate) const DURATION: u32 = 300;

/// A contextual move onto the exact center of an owned, complete arena means enter.
/// Ordinary move commands and replay command recording retain their existing wire vocabulary.
pub(super) fn entry_target(entities: &EntityStore, owner: u32, x: f32, y: f32) -> Option<u32> {
    entities
        .iter()
        .find(|e| {
            e.kind == EntityKind::TournamentGrounds
                && e.owner == owner
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
        let Some(point) =
            coordinator.find_spawn_point(entities, target, EntityKind::Warrior, Some(approach))
        else {
            continue;
        };
        if let Some(e) = entities.get_mut(*id) {
            e.clear_orders();
            e.replace_active_order(crate::game::entity::Order::move_to(point.0, point.1));
            e.set_path_goal(Some(point));
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
                && e.kind == EntityKind::TournamentGrounds
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
            if let Some(e) = entities.get_mut(id) {
                e.clear_orders();
            }
            continue;
        }
        // Rectangle distance, not center distance: corner approaches must reach the perimeter too.
        let half = config::TILE_SIZE as f32 * 1.5;
        let dx = ((x - building.pos_x()).abs() - half).max(0.0);
        let dy = ((y - building.pos_y()).abs() - half).max(0.0);
        if dx * dx + dy * dy > 24.0 * 24.0 {
            continue;
        }
        entities.remove(id);
        if let Some(player) = players.iter_mut().find(|p| p.id == owner) {
            player.release_supply(crate::rules::economy::supply_cost(EntityKind::Warrior));
        }
        if let Some(p) = entities.get_mut(target).and_then(|e| e.production.as_mut()) {
            p.tournament_entrants = p.tournament_entrants.saturating_add(1);
            if p.tournament_entrants == CAPACITY {
                p.tournament_winners = (0..3)
                    .map(|_| match rng.gen_range(0..3) {
                        0 => UpgradeKind::DaoOfLightness,
                        1 => UpgradeKind::DaoOfHeaviness,
                        _ => UpgradeKind::DaoOfMagic,
                    })
                    .collect();
                for index in 0..3 {
                    p.queue.push(ProdItem {
                        unit: EntityKind::Warrior,
                        progress: 0,
                        total: if index == 0 { DURATION } else { 0 },
                        paid: true,
                    });
                }
                if let Some(player) = players.iter_mut().find(|p| p.id == owner) {
                    // Eight consumed Disciples already freed sixteen supply. Winner reservations
                    // persist in paid production items and are rebuilt by the supply system.
                    player.reserve_supply(6);
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
