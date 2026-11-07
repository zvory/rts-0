//! Tanks sent to clear enemy infantry off Jeff's natural (`TankMicro::natural_clear`).

use super::*;

/// Tanks sent to clear enemy infantry off the natural attack-move onto them. The order goes out once
/// per spot, and again to a Tank found standing idle short of it.
pub(super) fn clear_natural(
    micro: &mut TankMicro,
    tanks: &BTreeMap<u32, &AiEntitySummary>,
    claimed: &BTreeSet<u32>,
    extra: &mut Vec<Command>,
    tile_size: f32,
    observation: &AiObservation,
) -> BTreeSet<u32> {
    let mut clearing = BTreeSet::new();
    let Some((ids, point)) = micro.natural_clear.clone() else {
        micro.natural_clear_sent.clear();
        micro.siege_orders.clear();
        return clearing;
    };
    micro.natural_clear_sent.retain(|id, _| ids.contains(id));
    micro.siege_orders.retain(|id, _| ids.contains(id));
    let at = world(point);
    for id in ids {
        if claimed.contains(&id) {
            continue;
        }
        let Some(tank) = tanks.get(&id) else {
            continue;
        };
        clearing.insert(id);
        // Jeff's AI V1.1 (`natsiege`) sieges from out of the infantry's reach.
        if micro.natural_siege {
            let last = micro.siege_orders.get(&id).copied();
            if let Some((order, command)) =
                super::jeff_v1_1::natsiege::order(observation, tank, at, last)
            {
                extra.push(command);
                micro.siege_orders.insert(id, order);
            }
            continue;
        }
        let short_and_idle = tank.state == AiEntityState::Idle
            && dist2(tank.x, tank.y, at.0, at.1) > squared(NATURAL_CLEAR_ARRIVED_TILES * tile_size);
        if micro.natural_clear_sent.get(&id) != Some(&point) || short_and_idle {
            extra.push(Command::AttackMove {
                units: vec![id],
                x: at.0,
                y: at.1,
                queued: false,
            });
            micro.natural_clear_sent.insert(id, point);
        }
    }
    clearing
}

/// A Tank clearing the natural this close to the enemy it was sent at has arrived.
const NATURAL_CLEAR_ARRIVED_TILES: f32 = 3.0;
