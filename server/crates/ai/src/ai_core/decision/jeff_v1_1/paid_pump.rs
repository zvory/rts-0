//! Jeff's AI V1.1's paid Pump Jacks (testing). A Resource Depot builds its Pump Jacks for free, one
//! at a time, 36 s each: the main starts with one, so its second is up at 0:36 and its third at 1:12,
//! and a new base's come 36, 72 and 108 s after its Depot. An Engineer builds one for 150 Steel in
//! 10 s on a well the free job has not taken. Since Methamphetamines costs 200 Oil (#1869) Oil holds
//! up Meth (ordered at 1:26), the natural and the Tank tech, while Steel floats by mid-game.
//!
//! - `pumpfirst`: before the first Barracks the Engineer builds one on the main's free well (up
//!   about 0:27 instead of 1:12, ~67 Oil sooner); the Barracks waits for its foundation.
//! - `natpump`: once the natural's Depot stands, the Engineer builds one on its free well farthest
//!   from the Depot (up about 10 s in instead of 108 s, ~147 Oil sooner).
//!
//! The farthest free well is taken: the free job takes the nearest.

use super::*;
use crate::ai_core::decision::geometry::{dist2, squared};

/// `pumpfirst` gives up past this, and the Barracks no longer waits.
const FIRST_GIVE_UP_TICKS: u32 = config::TICK_HZ * 30;
/// `natpump` gives up this long after the natural's Depot is first seen standing.
const NATURAL_GIVE_UP_TICKS: u32 = config::TICK_HZ * 45;
/// A Pump Jack this close to a well stands on it.
const ON_WELL_TILES: f32 = 0.75;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct PaidPumpMemory {
    /// The well each paid Pump Jack was ordered on, and whether its foundation went down.
    first: Option<u32>,
    first_done: bool,
    natural: Option<u32>,
    natural_done: bool,
    /// The natural's Depot and when it was first seen standing.
    natural_depot: Option<(u32, u32)>,
}

/// Orders the paid Pump Jacks. Returns whether the opening Barracks waits for the main's.
pub(in crate::ai_core::decision) fn run(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    facts: &AiFacts,
    memory: &mut AiDecisionMemory,
    profile_id: &str,
    (builder_pools, intents): (&[&[u32]], &mut Vec<AiIntent>),
) -> bool {
    if !is_jeff_v1_1(profile_id) {
        return false;
    }
    let Some(main) = main_depot(observation) else {
        return false;
    };
    let mut barracks_waits = false;
    let state = &mut memory.jeff_v1_1.paid_pump;
    if !off("pumpfirst")
        && !state.first_done
        && facts.building_count(EntityKind::Barracks) == 0
        && observation.tick < FIRST_GIVE_UP_TICKS
    {
        let (waits, done) = keep_ordered(
            actions,
            observation,
            (builder_pools, &mut *intents),
            (main.x, main.y),
            &mut state.first,
        );
        barracks_waits = waits;
        state.first_done = done;
    }
    if !off("natpump") && !state.natural_done {
        let natural = observation
            .owned
            .iter()
            .filter(|unit| {
                unit.kind == EntityKind::ResourceDepot
                    && unit.is_complete
                    && unit.hp > 0
                    && unit.id != main.id
            })
            .min_by_key(|unit| unit.id);
        if let Some(natural) = natural {
            let since = match state.natural_depot {
                Some((id, since)) if id == natural.id => since,
                _ => {
                    state.natural_depot = Some((natural.id, observation.tick));
                    observation.tick
                }
            };
            if observation.tick.saturating_sub(since) < NATURAL_GIVE_UP_TICKS {
                let (_, done) = keep_ordered(
                    actions,
                    observation,
                    (builder_pools, &mut *intents),
                    (natural.x, natural.y),
                    &mut state.natural,
                );
                state.natural_done = done;
            } else {
                state.natural_done = true;
            }
        }
    }
    barracks_waits
}

/// The Depot Jeff started with: the one nearest its start tile.
fn main_depot(observation: &AiObservation) -> Option<&AiEntitySummary> {
    let ts = observation.map.tile_size as f32;
    let start = (
        (observation.own_start_tile.0 as f32 + 0.5) * ts,
        (observation.own_start_tile.1 as f32 + 0.5) * ts,
    );
    observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.hp > 0)
        .min_by(|a, b| {
            dist2(a.x, a.y, start.0, start.1)
                .total_cmp(&dist2(b.x, b.y, start.0, start.1))
                .then(a.id.cmp(&b.id))
        })
}

/// Keeps one paid Pump Jack on order at the Depot at `depot`. Returns whether it is still to go
/// down, and whether it is done (its well has a Pump Jack, or no well is free).
fn keep_ordered(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    (builder_pools, intents): (&[&[u32]], &mut Vec<AiIntent>),
    depot: (f32, f32),
    slot: &mut Option<u32>,
) -> (bool, bool) {
    if let Some(well) = *slot {
        if pump_on(observation, well) {
            return (false, true);
        }
        if observation
            .pending_builds
            .iter()
            .any(|build| build.kind == EntityKind::PumpJack)
        {
            return (true, false);
        }
    }
    let Some(well) = free_well(observation, depot) else {
        return (false, true);
    };
    let ts = observation.map.tile_size as f32;
    let (tile_x, tile_y) = ((well.x / ts).floor() as u32, (well.y / ts).floor() as u32);
    if actions::try_build_at_on_credit(
        actions,
        builder_pools,
        EntityKind::PumpJack,
        tile_x,
        tile_y,
        None,
    )
    .is_none()
    {
        return (false, false);
    }
    *slot = Some(well.id);
    intents.push(AiIntent::Build {
        kind: EntityKind::PumpJack,
    });
    (true, false)
}

/// Whether a Pump Jack, anyone's, stands on the well `well`.
fn pump_on(observation: &AiObservation, well: u32) -> bool {
    let Some(node) = observation.resources.iter().find(|node| node.id == well) else {
        return true;
    };
    taken(observation, node)
}

fn taken(observation: &AiObservation, node: &AiResourceSummary) -> bool {
    let reach = squared(ON_WELL_TILES * observation.map.tile_size as f32);
    observation
        .owned
        .iter()
        .chain(&observation.visible_enemies)
        .chain(&observation.visible_allies)
        .any(|unit| {
            unit.kind == EntityKind::PumpJack
                && unit.hp > 0
                && dist2(unit.x, unit.y, node.x, node.y) <= reach
        })
}

/// The free well with Oil left within the Depot's mining reach farthest from it.
fn free_well(observation: &AiObservation, depot: (f32, f32)) -> Option<&AiResourceSummary> {
    let reach = squared(config::MINING_ANCHOR_RANGE_TILES * observation.map.tile_size as f32);
    observation
        .resources
        .iter()
        .filter(|node| node.kind == EntityKind::Oil && node.remaining > 0)
        .filter(|node| dist2(node.x, node.y, depot.0, depot.1) <= reach)
        .filter(|node| !taken(observation, node))
        .max_by(|a, b| {
            dist2(a.x, a.y, depot.0, depot.1)
                .total_cmp(&dist2(b.x, b.y, depot.0, depot.1))
                .then(b.id.cmp(&a.id))
        })
}
