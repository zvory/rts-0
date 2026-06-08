use crate::config;
use crate::game::entity::EntityStore;
use crate::game::PlayerState;
use crate::rules;

/// Recompute each player's supply cap (from completed City Centres and Depots) and supply used (living
/// units + units still in production queues). Cap is clamped to `SUPPLY_CAP_MAX`.
pub(crate) fn recompute_supply(players: &mut [PlayerState], entities: &EntityStore) {
    for ps in players.iter_mut() {
        let mut cap = 0u32;
        let mut used = 0u32;
        for e in entities.iter() {
            if e.owner != ps.id {
                continue;
            }
            if e.is_building() && !e.under_construction() {
                cap += rules::economy::supply_provided(e.kind);
                // Units queued for production reserve supply too.
                for item in e.prod_queue() {
                    used += rules::economy::supply_cost(item.unit);
                }
            } else if e.is_unit() {
                used += rules::economy::supply_cost(e.kind);
            }
        }
        ps.supply_cap = cap.min(config::SUPPLY_CAP_MAX);
        ps.supply_used = used;
    }
}
