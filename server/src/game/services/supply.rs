use crate::config;
use crate::game::entity::EntityStore;
use crate::game::PlayerState;

/// Recompute each player's supply cap (from completed Industrial Centers/Depots) and supply used (living
/// units + units still in production queues). Cap is clamped to `SUPPLY_CAP_MAX`.
pub(crate) fn recompute_supply(players: &mut [PlayerState], entities: &EntityStore) {
    for ps in players.iter_mut() {
        let mut cap = 0u32;
        let mut used = 0u32;
        for e in entities.iter() {
            if e.owner != ps.id {
                continue;
            }
            if e.is_building() && !e.under_construction {
                if let Some(s) = config::building_stats(e.kind) {
                    cap += s.provides_supply;
                }
                // Units queued for production reserve supply too.
                for item in &e.prod_queue {
                    if let Some(us) = config::unit_stats(item.unit) {
                        used += us.supply;
                    }
                }
            } else if e.is_unit() {
                if let Some(us) = config::unit_stats(e.kind) {
                    used += us.supply;
                }
            }
        }
        ps.supply_cap = cap.min(config::SUPPLY_CAP_MAX);
        ps.supply_used = used;
    }
}
