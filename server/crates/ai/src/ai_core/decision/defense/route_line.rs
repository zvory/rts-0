//! Jeff's forward picket and warned sealing of the home line.
//!
//! One Rifleman holds a trench on the enemy's ground route into the main, well outside the base,
//! so a raid is seen about twenty seconds before it reaches the Mines instead of a handful. When a
//! raid is seen coming, spare home Riflemen move into slots beside the home pocket, facing the
//! raid, and dig in, so it meets an entrenched line rather than whoever was standing there.
//!
//! Both are deliberately narrow, because moving the home defense on a false alarm costs more than
//! it saves:
//! - the picket only goes out once Entrenchment is researched and the home pocket is full, never
//!   answers raids itself, and is not replaced for ninety seconds after it dies;
//! - an alert needs at least three non-armored enemy units within 26 tiles of the main, none of
//!   them inside the base yet, seen closing in on consecutive sightings, and it lapses 8 seconds
//!   after the last sighting. Sealing slots stay within 10 tiles of the Depot, only Riflemen
//!   already near home are used, and nothing is moved while an enemy is inside the base: local
//!   defense owns that fight.

use super::*;

/// The picket stands where the enemy's route first comes within this range of the main.
const PICKET_ROUTE_TILES: f32 = 26.0;
/// A lost picket is not replaced for this long. It is one Rifleman from the surplus, and even a
/// picket that dies quickly has done its job: it saw what killed it.
const PICKET_RETRY_TICKS: u32 = config::TICK_HZ * 90;
/// The four oldest home Riflemen own the home pocket; the picket and the sealers come from the rest.
const HOME_POCKET_RIFLES: usize = 4;
const PICKET_MIN_HOME_RIFLES: usize = HOME_POCKET_RIFLES + 1;
const ARRIVAL_TILES: f32 = 1.5;
/// A unit on its way is re-ordered at most this often.
const REORDER_TICKS: u32 = config::TICK_HZ * 3;
const ALERT_RADIUS_TILES: f32 = 26.0;
const ALERT_MIN_UNITS: usize = 3;
/// Two sightings this close in time, the second at least this much nearer, count as closing in.
const ALERT_TRACK_WINDOW_TICKS: u32 = config::TICK_HZ * 2;
const ALERT_CLOSING_TENTHS: u32 = 5;
const ALERT_EXPIRY_TICKS: u32 = config::TICK_HZ * 8;
/// Only Riflemen this close to the main are pulled onto the line.
const SEALER_SELECTION_TILES: f32 = 18.0;
/// (forward, lateral) tile offsets from the main, facing the raid. They fill the flanks and rear of
/// the home pocket's shape without landing on its six slots.
const SEAL_SLOTS: [(f32, f32); 6] = [
    (6.4, 5.0),
    (6.4, -5.0),
    (9.4, 3.6),
    (9.4, -3.6),
    (4.6, 7.0),
    (4.6, -7.0),
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct RouteLine {
    picket: Option<u32>,
    picket_lost_tick: Option<u32>,
    picket_order_tick: Option<u32>,
    picket_holding: bool,
    /// Last sighting of a qualifying group: tick and distance to the main in tenths of a tile.
    track: Option<(u32, u32)>,
    alert: Option<RaidAlert>,
    /// Sealing Rifleman -> slot index.
    sealers: BTreeMap<u32, usize>,
    seal_order_tick: BTreeMap<u32, u32>,
    sealers_holding: BTreeSet<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RaidAlert {
    last_seen: u32,
    /// Direction from the main to the raid when it was first confirmed, in thousandths.
    bearing_milli: (i32, i32),
}

impl RouteLine {
    /// Units this plan owns: the picket always, the sealers while an alert lasts. No other system
    /// may order them.
    pub(in crate::ai_core::decision) fn reserved(&self) -> impl Iterator<Item = u32> + '_ {
        self.picket.into_iter().chain(self.sealers.keys().copied())
    }

    pub(in crate::ai_core::decision) fn picket(&self) -> Option<u32> {
        self.picket
    }
}

/// This decision's picket and sealing orders.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct RouteLineOrders {
    /// Units given a move or hold this decision.
    pub(in crate::ai_core::decision) ordered: Vec<u32>,
    /// Sealers handed back after an alert. Their stale staging must be cleared so their normal
    /// posts are sent again.
    pub(in crate::ai_core::decision) released: Vec<u32>,
}

pub(in crate::ai_core::decision) fn plan_route_line(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    map_analysis: Option<&AiMapAnalysis>,
) -> RouteLineOrders {
    let mut orders = RouteLineOrders::default();
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let anchor = tile_center(observation.own_start_tile, observation.map.tile_size);
    let alive = |id: u32| {
        observation
            .owned
            .iter()
            .any(|unit| unit.id == id && unit.hp > 0)
    };
    {
        let state = &mut memory.route_line;
        if state.picket.is_some_and(|id| !alive(id)) {
            state.picket = None;
            state.picket_lost_tick = Some(tick);
            state.picket_holding = false;
            state.picket_order_tick = None;
        }
        state.sealers.retain(|id, _| alive(*id));
    }
    let contact_inside = local_defense_contact(observation).is_some();

    update_alert(
        observation,
        &mut memory.route_line,
        anchor,
        ts,
        contact_inside,
    );
    if memory.route_line.alert.is_none() && !memory.route_line.sealers.is_empty() {
        let state = &mut memory.route_line;
        orders.released = std::mem::take(&mut state.sealers).into_keys().collect();
        state.seal_order_tick.clear();
        state.sealers_holding.clear();
    }

    let home_rifles = spare_home_riflemen(observation, memory);
    if memory.route_line.picket.is_none() && !contact_inside && memory.route_line.alert.is_none() {
        choose_picket(observation, memory, map_analysis, &home_rifles, tick);
    }
    if let Some(point) = memory
        .route_line
        .picket
        .and_then(|_| picket_point(observation, map_analysis))
    {
        orders.ordered.extend(order_to_post(
            actions,
            observation,
            memory,
            PostOwner::Picket,
            point,
        ));
    }

    if let Some(alert) = memory.route_line.alert {
        if !contact_inside {
            let bearing = (
                alert.bearing_milli.0 as f32 / 1000.0,
                alert.bearing_milli.1 as f32 / 1000.0,
            );
            if memory.route_line.sealers.is_empty() {
                choose_sealers(
                    observation,
                    memory,
                    map_analysis,
                    &home_rifles,
                    anchor,
                    bearing,
                );
            }
            let sealers: Vec<(u32, usize)> = memory
                .route_line
                .sealers
                .iter()
                .map(|(id, slot)| (*id, *slot))
                .collect();
            for (id, slot) in sealers {
                if let Some(point) = seal_slot(observation, map_analysis, anchor, bearing, slot) {
                    orders.ordered.extend(order_to_post(
                        actions,
                        observation,
                        memory,
                        PostOwner::Sealer(id),
                        point,
                    ));
                }
            }
        }
    }
    orders.ordered.sort_unstable();
    orders.ordered.dedup();
    orders
}

/// Track the nearest qualifying enemy group outside the base and raise, refresh or drop the alert.
fn update_alert(
    observation: &AiObservation,
    state: &mut RouteLine,
    anchor: (f32, f32),
    ts: f32,
    contact_inside: bool,
) {
    let tick = observation.tick;
    let radius2 = squared(ALERT_RADIUS_TILES * ts);
    let group: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .filter(|enemy| dist2(enemy.x, enemy.y, anchor.0, anchor.1) <= radius2)
        .collect();
    let armored = group
        .iter()
        .any(|enemy| matches!(enemy.kind, EntityKind::Tank | EntityKind::ScoutCar));
    if !contact_inside && group.len() >= ALERT_MIN_UNITS && !armored {
        let centroid = (
            group.iter().map(|enemy| enemy.x).sum::<f32>() / group.len() as f32,
            group.iter().map(|enemy| enemy.y).sum::<f32>() / group.len() as f32,
        );
        let distance_tenths =
            (dist2(centroid.0, centroid.1, anchor.0, anchor.1).sqrt() / ts * 10.0) as u32;
        let closing = state.track.is_some_and(|(seen, previous)| {
            tick.saturating_sub(seen) <= ALERT_TRACK_WINDOW_TICKS
                && distance_tenths + ALERT_CLOSING_TENTHS <= previous
        });
        state.track = Some((tick, distance_tenths));
        if let Some(alert) = state.alert.as_mut() {
            alert.last_seen = tick;
        } else if closing {
            if let Some(direction) = normalized_direction(anchor, centroid) {
                state.alert = Some(RaidAlert {
                    last_seen: tick,
                    bearing_milli: (
                        (direction.0 * 1000.0).round() as i32,
                        (direction.1 * 1000.0).round() as i32,
                    ),
                });
            }
        }
    } else if contact_inside {
        // The raid has arrived; keep the alert alive while the fight lasts.
        if let Some(alert) = state.alert.as_mut() {
            alert.last_seen = tick;
        }
    }
    if state
        .alert
        .is_some_and(|alert| tick.saturating_sub(alert.last_seen) > ALERT_EXPIRY_TICKS)
    {
        state.alert = None;
        state.track = None;
    }
}

/// Home Riflemen beyond the four that own the pocket, not taken by the push or the natural party,
/// sorted by id. The picket and sealers are chosen from these.
fn spare_home_riflemen(observation: &AiObservation, memory: &AiDecisionMemory) -> Vec<u32> {
    let mut rifles: Vec<u32> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman && unit.is_complete && unit.hp > 0)
        .filter(|unit| {
            !memory.containment.active_riflemen.contains(&unit.id)
                && !memory.expansion_security.riflemen.contains(&unit.id)
                && !memory.opening_rush.is_reserved(unit.id)
        })
        .map(|unit| unit.id)
        .collect();
    rifles.sort_unstable();
    if rifles.len() < PICKET_MIN_HOME_RIFLES {
        return Vec::new();
    }
    rifles.split_off(HOME_POCKET_RIFLES)
}

fn picket_point(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
) -> Option<(f32, f32)> {
    let point = map_analysis?.base_route_point(observation.player_id, PICKET_ROUTE_TILES)?;
    clear_mobile_defensive_position(observation, map_analysis, point)
}

fn choose_picket(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    map_analysis: Option<&AiMapAnalysis>,
    spare_rifles: &[u32],
    tick: u32,
) {
    let state = &memory.route_line;
    if !observation.upgrades.contains(&UpgradeKind::Entrenchment)
        || state
            .picket_lost_tick
            .is_some_and(|lost| tick.saturating_sub(lost) < PICKET_RETRY_TICKS)
    {
        return;
    }
    let Some(point) = picket_point(observation, map_analysis) else {
        return;
    };
    let picket = spare_rifles
        .iter()
        .filter(|id| !state.sealers.contains_key(id))
        .filter_map(|id| observation.owned.iter().find(|unit| unit.id == *id))
        .min_by(|left, right| {
            dist2(left.x, left.y, point.0, point.1)
                .total_cmp(&dist2(right.x, right.y, point.0, point.1))
                .then_with(|| left.id.cmp(&right.id))
        })
        .map(|unit| unit.id);
    let state = &mut memory.route_line;
    state.picket = picket;
    state.picket_holding = false;
    state.picket_order_tick = None;
}

fn seal_slot(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    anchor: (f32, f32),
    bearing: (f32, f32),
    slot: usize,
) -> Option<(f32, f32)> {
    let (forward, lateral) = *SEAL_SLOTS.get(slot)?;
    let ts = observation.map.tile_size as f32;
    let perpendicular = (-bearing.1, bearing.0);
    let desired = clamp_to_map(
        (
            anchor.0 + (bearing.0 * forward + perpendicular.0 * lateral) * ts,
            anchor.1 + (bearing.1 * forward + perpendicular.1 * lateral) * ts,
        ),
        observation.map,
    );
    clear_mobile_defensive_position(observation, map_analysis, desired)
}

fn choose_sealers(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    map_analysis: Option<&AiMapAnalysis>,
    spare_rifles: &[u32],
    anchor: (f32, f32),
    bearing: (f32, f32),
) {
    let ts = observation.map.tile_size as f32;
    let near2 = squared(SEALER_SELECTION_TILES * ts);
    let picket = memory.route_line.picket;
    let mut candidates: Vec<&AiEntitySummary> = spare_rifles
        .iter()
        .filter(|id| Some(**id) != picket)
        .filter_map(|id| observation.owned.iter().find(|unit| unit.id == *id))
        .filter(|unit| dist2(unit.x, unit.y, anchor.0, anchor.1) <= near2)
        .collect();
    let mut sealers = BTreeMap::new();
    for slot in 0..SEAL_SLOTS.len() {
        let Some(point) = seal_slot(observation, map_analysis, anchor, bearing, slot) else {
            continue;
        };
        let Some(index) = candidates
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                dist2(left.x, left.y, point.0, point.1)
                    .total_cmp(&dist2(right.x, right.y, point.0, point.1))
                    .then_with(|| left.id.cmp(&right.id))
            })
            .map(|(index, _)| index)
        else {
            break;
        };
        sealers.insert(candidates.remove(index).id, slot);
    }
    memory.route_line.sealers = sealers;
}

#[derive(Clone, Copy)]
enum PostOwner {
    Picket,
    Sealer(u32),
}

/// Walk a unit to its post and hold it there once, so it digs in. Holding again every decision
/// would clear its target, so a hold is only repeated if something moved the unit off.
fn order_to_post(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    owner: PostOwner,
    point: (f32, f32),
) -> Option<u32> {
    let state = &mut memory.route_line;
    let (id, holding, order_tick) = match owner {
        PostOwner::Picket => (state.picket?, state.picket_holding, state.picket_order_tick),
        PostOwner::Sealer(id) => (
            id,
            state.sealers_holding.contains(&id),
            state.seal_order_tick.get(&id).copied(),
        ),
    };
    let unit = observation.owned.iter().find(|unit| unit.id == id)?;
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    let arrived = dist2(unit.x, unit.y, point.0, point.1) <= squared(ARRIVAL_TILES * ts);
    let (now_holding, ordered) = if arrived {
        if !holding || unit.state == AiEntityState::Move {
            (true, actions::hold_position_units(actions, [id]).is_some())
        } else {
            (true, false)
        }
    } else if unit.state != AiEntityState::Move
        || order_tick.is_none_or(|last| tick.saturating_sub(last) >= REORDER_TICKS)
    {
        (
            false,
            actions::move_units(actions, [id], point.0, point.1).is_some(),
        )
    } else {
        (false, false)
    };
    match owner {
        PostOwner::Picket => {
            state.picket_holding = now_holding;
            if ordered && !now_holding {
                state.picket_order_tick = Some(tick);
            }
        }
        PostOwner::Sealer(id) => {
            if now_holding {
                state.sealers_holding.insert(id);
            } else {
                state.sealers_holding.remove(&id);
            }
            if ordered && !now_holding {
                state.seal_order_tick.insert(id, tick);
            }
        }
    }
    ordered.then_some(id)
}

#[cfg(test)]
mod tests;
