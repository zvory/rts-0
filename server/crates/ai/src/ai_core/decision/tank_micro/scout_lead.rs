//! Scout Cars leading Tank marches: out ahead of the front Tank, its sight finds a parked line
//! before the Tanks drive into its reach.

use super::*;

/// A Scout Car marching with Tanks goes this far ahead of the front Tank, so its 15-tile sight
/// finds a parked line before the Tanks walk into its reach (Tanks see only 9).
const SCOUT_LEAD_TILES: f32 = 4.0;
/// Moving Tanks this close to a Scout Car are the march it leads.
const SCOUT_ESCORT_TILES: f32 = 9.0;
/// A Scout Car that sees an enemy this close has found the contact: out in front of the fight it
/// only dies (and a push without its Scout Car stalls), so it goes back to its own orders.
const SCOUT_CONTACT_TILES: f32 = 15.0;
/// The lead order is re-issued only when the lead point moves this far.
const SCOUT_RELEAD_TILES: f32 = 2.0;
/// Only Tanks this far from home are on a march; nearer they are forming up.
const SCOUT_LEAD_FROM_HOME_TILES: f32 = 16.0;
/// The marching Tanks' headings must agree this well (summed unit headings per Tank).
const SCOUT_LEAD_AGREEMENT: f32 = 0.75;

/// Sends each Scout Car marching with two or more moving Tanks ahead of them, stopping it just
/// outside any known reach, and returns the Scout Cars it leads. Scout Cars in `skip` are left alone.
pub(super) fn lead_scouts(
    observation: &AiObservation,
    micro: &mut TankMicro,
    tanks: &BTreeMap<u32, &AiEntitySummary>,
    zones: &[Zone],
    claimed: &BTreeSet<u32>,
    skip: &BTreeSet<u32>,
    extra: &mut Vec<Command>,
) -> BTreeSet<u32> {
    let tile_size = observation.map.tile_size as f32;
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let scouts: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ScoutCar && unit.is_complete && unit.hp > 0)
        .filter(|unit| !claimed.contains(&unit.id) && !skip.contains(&unit.id))
        .collect();
    let mut leading = BTreeMap::new();
    let mut led = BTreeSet::new();
    for scout in scouts {
        let marching: Vec<&AiEntitySummary> = tanks
            .values()
            .copied()
            .filter(|tank| tank.state == AiEntityState::Move && !claimed.contains(&tank.id))
            .filter(|tank| {
                dist2(tank.x, tank.y, home.0, home.1)
                    >= squared(SCOUT_LEAD_FROM_HOME_TILES * tile_size)
            })
            .filter(|tank| {
                dist2(tank.x, tank.y, scout.x, scout.y) <= squared(SCOUT_ESCORT_TILES * tile_size)
            })
            .collect();
        if marching.len() < 2 {
            continue;
        }
        let contact = observation.visible_enemies.iter().any(|enemy| {
            enemy.hp > 0
                && dist2(enemy.x, enemy.y, scout.x, scout.y)
                    <= squared(SCOUT_CONTACT_TILES * tile_size)
        });
        if contact {
            continue;
        }
        // A march: the Tanks head the same way. Tanks shuffling into slots do not.
        let headings: Vec<(f32, f32)> = marching
            .iter()
            .filter_map(|tank| micro.own.get(&tank.id).and_then(|own| own.heading))
            .filter_map(|(dx, dy)| normalized_direction((0.0, 0.0), (dx as f32, dy as f32)))
            .collect();
        if headings.len() < 2 {
            continue;
        }
        let sum = headings
            .iter()
            .fold((0.0, 0.0), |acc, h| (acc.0 + h.0, acc.1 + h.1));
        if (sum.0 * sum.0 + sum.1 * sum.1).sqrt() < SCOUT_LEAD_AGREEMENT * headings.len() as f32 {
            continue;
        }
        let Some(direction) = normalized_direction((0.0, 0.0), sum) else {
            continue;
        };
        let Some(front) = marching.iter().max_by(|a, b| {
            (a.x * direction.0 + a.y * direction.1)
                .total_cmp(&(b.x * direction.0 + b.y * direction.1))
                .then_with(|| a.id.cmp(&b.id))
        }) else {
            continue;
        };
        let mut lead = clamp_to_map(
            (
                front.x + direction.0 * SCOUT_LEAD_TILES * tile_size,
                front.y + direction.1 * SCOUT_LEAD_TILES * tile_size,
            ),
            observation.map,
        );
        for zone in zones {
            if dist2(lead.0, lead.1, zone.center.0, zone.center.1) < squared(zone.radius) {
                if let Some(out) = normalized_direction(zone.center, lead) {
                    let edge = zone.radius + 0.2 * tile_size;
                    lead = (zone.center.0 + out.0 * edge, zone.center.1 + out.1 * edge);
                }
            }
        }
        led.insert(scout.id);
        let previous = micro.scout_leads.get(&scout.id).copied().map(world);
        let moved_on = previous.is_none_or(|previous| {
            dist2(previous.0, previous.1, lead.0, lead.1) > squared(SCOUT_RELEAD_TILES * tile_size)
        });
        let idle_away = scout.state != AiEntityState::Move
            && dist2(scout.x, scout.y, lead.0, lead.1) > squared(3.0 * tile_size);
        if moved_on || idle_away {
            extra.push(Command::Move {
                units: vec![scout.id],
                x: lead.0,
                y: lead.1,
                queued: false,
            });
            leading.insert(scout.id, stored(lead));
        } else if let Some(previous) = micro.scout_leads.get(&scout.id) {
            leading.insert(scout.id, *previous);
        }
    }
    micro.scout_leads = leading;
    led
}
