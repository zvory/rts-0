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
                if rules::faction::catalog_for_or_default(&ps.faction_id).allows_building(e.kind) {
                    cap += rules::economy::supply_provided(e.kind);
                }
                // Units queued for production reserve supply too.
                for item in e.prod_queue() {
                    if rules::faction::catalog_for_or_default(&ps.faction_id).allows_unit(item.unit)
                    {
                        used += rules::economy::supply_cost(item.unit);
                    }
                }
            } else if e.is_unit()
                && rules::faction::catalog_for_or_default(&ps.faction_id).allows_unit(e.kind)
            {
                used += rules::economy::supply_cost(e.kind);
            }
        }
        ps.set_supply_counts(used, cap);
    }
}
