use crate::config;
use crate::game::entity::EntityStore;
use crate::game::PlayerState;
use crate::rules;

/// Recompute each player's supply used from living units plus units still in production queues and
/// restore the intrinsic player cap.
pub(crate) fn recompute_supply(players: &mut [PlayerState], entities: &EntityStore) {
    for ps in players.iter_mut() {
        let catalog = rules::faction::catalog_for(&ps.faction_id);
        let mut used = 0u32;
        for e in entities.iter() {
            if e.owner != ps.id {
                continue;
            }
            if e.is_building() && !e.under_construction() {
                // Only paid production items reserve supply; manual waiting entries do not.
                for item in e.prod_queue().iter().filter(|item| item.paid) {
                    if catalog.is_some_and(|catalog| catalog.allows_unit(item.unit)) {
                        used += rules::economy::supply_cost(item.unit);
                    }
                }
            } else if e.is_unit() && catalog.is_some_and(|catalog| catalog.allows_unit(e.kind)) {
                used += rules::economy::supply_cost(e.kind);
            }
        }
        ps.set_supply_counts(used, config::PLAYER_SUPPLY_CAP);
    }
}
