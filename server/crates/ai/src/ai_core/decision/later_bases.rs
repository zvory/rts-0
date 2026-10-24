//! Jeff's bases beyond the natural.
//!
//! Base 3 is taken once the natural is finished and the army can spare it: five Tanks in all,
//! three of them not committed to a push, no enemy inside the base, and the Depot's cost in the
//! bank. After that, each base whose first oil well runs dry unlocks exactly one more base, under
//! the same conditions. A lost base is retaken, after a cooldown, because the target does not
//! shrink.
//!
//! While a base is being taken, two guard Tanks leave the attack pool and hold a post in front
//! of the new site, and new Tanks and Riflemen rally there. The Depot is ordered once the guards
//! arrive, or after a bounded wait if the site is quiet, so a base is never stalled indefinitely.
//! Guards are not replaced: losing one before the foundation is down means the site is
//! contested, so it is abandoned for a while instead of feeding Tanks into it one at a time.
//! The guards keep covering the finished Depot for a while before rejoining the army.

use super::geometry::{building_center, clamp_to_map, dist2, normalized_direction, squared};
use super::*;

/// Tanks not committed to a push (home Tank included) needed before a base is taken.
const REQUIRED_FREE_TANKS: usize = 3;
/// Tanks owned in total before a base is taken: the home Tank, the two-Tank push and two guards.
/// With fewer, the guards would be the push's Tanks, and a push that never leaves lets the
/// enemy walk into a split army.
const REQUIRED_TOTAL_TANKS: usize = 5;
/// On Crossroads the push waits for six Tanks (see `frontal::CROSSROADS_PUSH_MIN_TANKS`), so a base
/// only needs the home Tank, the two guards and one more Tank to hold the main.
const CROSSROADS_REQUIRED_TOTAL_TANKS: usize = 4;
/// Tanks reserved from the attack pool to cover a new base. The home Tank stays home.
const GUARD_TANKS: usize = 2;
/// How far in front of the new Depot, toward the enemy, the guard post sits. Close enough that the
/// Depot, its builder and the rally stay inside the guards' 14-tile stationary range.
const GUARD_POST_TILES: f32 = 5.0;
/// Guards spread this far apart across the post so one shell lane cannot hit both.
const GUARD_LATERAL_TILES: f32 = 2.0;
const GUARD_ARRIVAL_TILES: f32 = 2.5;
/// Enemy presence within this range of the site delays the Depot order.
const SITE_CONTESTED_TILES: f32 = 11.0;
/// Order the Depot without waiting for the guards once staging has lasted this long.
const STAGING_TIMEOUT_TICKS: u32 = config::TICK_HZ * 30;
/// Abandon a site that has not produced a Depot in this long and look for another one.
const SITE_GIVE_UP_TICKS: u32 = config::TICK_HZ * 120;
/// A Depot order whose builder dropped it this long ago without a foundation has failed.
const BUILD_START_TIMEOUT_TICKS: u32 = config::TICK_HZ * 3;
/// Dropped orders or destroyed Depots tolerated at one site before it is rejected.
const MAX_SITE_FAILURES: u8 = 2;
/// A rejected site rules out every candidate this close to it; the search would otherwise pick
/// the tile next door at the same contested base.
const REJECTED_SITE_RADIUS_TILES: f32 = 6.0;
/// After a site is abandoned or its Depot is destroyed, wait this long before taking a base.
const SITE_RETRY_COOLDOWN_TICKS: u32 = config::TICK_HZ * 60;
/// An en-route guard's move order is refreshed at most this often.
const GUARD_REORDER_TICKS: u32 = config::TICK_HZ * 3;
/// Guards keep covering a finished Depot this long before rejoining the army.
const GUARD_AFTER_COMPLETE_TICKS: u32 = config::TICK_HZ * 45;
/// A pumped oil well at or below this is about to run dry (about 27 seconds of pumping left).
const DRY_WELL_WARNING_REMAINING: u32 = 40;
/// A real well loses 2 per 40 ticks. A larger drop between decisions means the reading was not a
/// live one (an out-of-sight well reports a placeholder), so it must not count as running dry.
const MAX_PLAUSIBLE_WELL_DROP: u32 = 12;
/// Pump Jacks and Steel Mines within this range of a Depot belong to that base.
const BASE_MINING_RANGE_TILES: f32 = 12.0;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct LaterBases {
    /// Last live reading of each oil well under one of Jeff's Pump Jacks.
    well_remaining: BTreeMap<u32, u32>,
    /// Depots whose first oil well has run dry. Each unlocks one more base.
    unlocking_depots: BTreeSet<u32>,
    site: Option<(u32, u32)>,
    staging_since: Option<u32>,
    build_attempt_tick: Option<u32>,
    /// Dropped Depot orders per site. A drop usually means the builder was shot on the way,
    /// not that the site is bad, so a site is only rejected after repeated drops.
    site_failures: BTreeMap<(u32, u32), u8>,
    rejected_sites: BTreeSet<(u32, u32)>,
    /// No new site is taken before this tick, after a failed or destroyed base.
    retry_after_tick: u32,
    /// Whether a Depot has stood on the guarded site, so its disappearance is a loss.
    guard_site_built: bool,
    pub(super) guards: BTreeSet<u32>,
    /// Last move order per guard, so an en-route guard is not re-ordered every decision.
    guard_order_tick: BTreeMap<u32, u32>,
    /// Guards already told to hold at their slot. Holding clears a Tank's target, so it is sent
    /// once per arrival rather than every decision.
    guards_holding: BTreeSet<u32>,
    /// The Depot currently being covered (a site being taken or a Depot that just finished).
    guard_site: Option<(u32, u32)>,
    guard_release_tick: Option<u32>,
}

/// What the rest of the decision needs from this cycle.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct LaterBaseStatus {
    /// Guard Tanks and their post slots, to be staged after local defense has claimed its units.
    pub(super) guard_posts: Vec<(u32, (f32, f32))>,
    /// Rally point for new Tanks and Riflemen while a base is being taken.
    pub(super) rally: Option<(f32, f32)>,
    pub(super) intents: Vec<AiIntent>,
}

impl LaterBases {
    /// Bases Jeff should currently own: the start, the natural, base 3, plus one per base whose
    /// first oil well has run dry.
    pub(super) fn target_bases(&self) -> usize {
        REQUIRED_BASES_BEFORE_DEPLETION + self.unlocking_depots.len()
    }

    /// The site being taken, or the new Depot its guards still cover.
    pub(super) fn covered_site(&self) -> Option<(u32, u32)> {
        self.guard_site
    }
}

const REQUIRED_BASES_BEFORE_DEPLETION: usize = 3;

#[allow(clippy::too_many_arguments)]
pub(super) fn plan<F>(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
    memory: &mut AiDecisionMemory,
    map_analysis: Option<&AiMapAnalysis>,
    actions: &mut AiActionContext<'_>,
    builder_pools: &[&[u32]],
    factory_due: bool,
    placeable: &mut F,
) -> LaterBaseStatus
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    let mut status = LaterBaseStatus::default();
    if !uses_current_jeffs_ai_policy(profile.id) {
        return status;
    }
    note_dry_wells(observation, &mut memory.later_bases);
    let Some(expansion) = profile.expansion else {
        return status;
    };
    let Some(enemy_base) = facts.nearest_public_enemy_base else {
        return status;
    };
    let enemy = (enemy_base.x, enemy_base.y);
    let tick = observation.tick;

    refresh_guard_site(observation, memory, tick);

    let depots = facts.building_count(EntityKind::ResourceDepot);
    let natural_done = facts.complete_building_count(EntityKind::ResourceDepot) >= 2;
    let base_due = natural_done && depots < memory.later_bases.target_bases();

    if base_due && memory.later_bases.site.is_none() && tick >= memory.later_bases.retry_after_tick
    {
        let free_tanks = free_tank_ids(observation, memory);
        let total_tanks = observation
            .owned
            .iter()
            .filter(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
            .count();
        let (steel, oil) = rts_rules::economy::cost(EntityKind::ResourceDepot);
        // The Crossroads push waits for six Tanks, so no push Tanks need covering first.
        let required_total_tanks =
            if defense::crossroads_wall_aware_approach_direction(observation).is_some() {
                CROSSROADS_REQUIRED_TOTAL_TANKS
            } else {
                REQUIRED_TOTAL_TANKS
            };
        let ready = total_tanks >= required_total_tanks
            // The two guards leave, so the home reserve must still be there after they go.
            && free_tanks.len()
                >= REQUIRED_FREE_TANKS.max(GUARD_TANKS + memory.home_tank_reserve())
            && main_tank_ids(observation, memory).len() >= memory.home_tank_reserve()
            && defense::local_defense_contact(observation).is_none()
            && observation.economy.steel >= steel
            && observation.economy.oil >= oil
            && !factory_due;
        if ready {
            let resources = expansion::expansion_candidate_resources(observation);
            let rejected = memory.later_bases.rejected_sites.clone();
            let route =
                map_analysis.and_then(|analysis| analysis.base_route_tiles(observation.player_id));
            let site = expansion::defensible_expansion_depot_site_adjusted(
                observation,
                map_analysis,
                expansion,
                EntityKind::ResourceDepot,
                &resources,
                &mut |kind, x, y| !site_is_rejected(&rejected, (x, y)) && placeable(kind, x, y),
                |tile| route_exposure_penalty(observation, route, tile),
            );
            let site_center = site.and_then(|site| {
                building_center(site, EntityKind::ResourceDepot, observation.map.tile_size)
            });
            if let Some((site, site_center)) = site.zip(site_center) {
                let state = &mut memory.later_bases;
                state.site = Some(site);
                state.guard_site = Some(site);
                state.guard_site_built = false;
                state.staging_since = Some(tick);
                state.build_attempt_tick = None;
                state.guard_release_tick = None;
                select_guards(observation, memory, site_center);
            }
        }
    }

    let Some(guard_site) = memory.later_bases.guard_site else {
        memory.later_bases.guards.clear();
        return status;
    };
    let Some(site_center) = building_center(
        guard_site,
        EntityKind::ResourceDepot,
        observation.map.tile_size,
    ) else {
        return status;
    };
    if drop_lost_guards(observation, memory) {
        if let Some(site) = memory.later_bases.site {
            // A guard died before the Depot went down, so the site is contested. Replacing it
            // would feed Tanks one at a time into whatever killed it; back off and retry later.
            let state = &mut memory.later_bases;
            let reject = note_site_failure(state, site);
            abandon_site(state, site, tick, reject);
            return status;
        }
    }
    let slots = guard_post_slots(observation, map_analysis, site_center, enemy);
    status.guard_posts = memory
        .later_bases
        .guards
        .iter()
        .copied()
        .zip(slots.iter().copied())
        .collect();

    let Some(site) = memory.later_bases.site else {
        // The Depot exists; the guards only cover it until release.
        return status;
    };
    // New Tanks and Riflemen gather on the guards rather than at home while the site is taken.
    status.rally = slots.first().copied();
    let ts = observation.map.tile_size as f32;
    let staging_since = memory.later_bases.staging_since.unwrap_or(tick);
    let staged_for = tick.saturating_sub(staging_since);

    if staged_for >= SITE_GIVE_UP_TICKS {
        abandon_site(&mut memory.later_bases, site, tick, true);
        return status;
    }
    if let Some(attempt) = memory.later_bases.build_attempt_tick {
        let order_pending = observation.pending_builds.iter().any(|intent| {
            intent.kind == EntityKind::ResourceDepot && (intent.tile_x, intent.tile_y) == site
        });
        if !order_pending && tick.saturating_sub(attempt) >= BUILD_START_TIMEOUT_TICKS {
            let state = &mut memory.later_bases;
            if note_site_failure(state, site) {
                abandon_site(state, site, tick, true);
            } else {
                // Most drops are a builder retreating after taking fire. Keep the site, let the
                // guards settle again, and send the next order under their cover.
                state.build_attempt_tick = None;
                state.staging_since = Some(tick);
            }
            return status;
        }
        if order_pending {
            // The simulation charges the Depot when its foundation goes down, not when it is
            // ordered: keep the cost banked while the builder walks, or it is spent on the way.
            let (steel, oil) = rts_rules::economy::cost(EntityKind::ResourceDepot);
            actions.holdback_resources(steel, oil);
            return status;
        }
    }

    let guards_arrived = status.guard_posts.iter().all(|(guard, slot)| {
        observation.owned.iter().any(|unit| {
            unit.id == *guard
                && dist2(unit.x, unit.y, slot.0, slot.1) <= squared(GUARD_ARRIVAL_TILES * ts)
        })
    });
    let contested = observation.visible_enemies.iter().any(|enemy| {
        enemy.hp > 0
            && (enemy.kind.is_unit() || enemy.kind.is_building())
            && dist2(enemy.x, enemy.y, site_center.0, site_center.1)
                < squared(SITE_CONTESTED_TILES * ts)
    });
    // The builder crosses the base to reach the site, and a worker hit on the way retreats and
    // drops its order, so an enemy inside the base also holds the order back.
    let base_quiet = defense::local_defense_contact(observation).is_none();
    let may_build =
        !contested && base_quiet && (guards_arrived || staged_for >= STAGING_TIMEOUT_TICKS);
    if may_build
        && actions::try_build_at(
            actions,
            builder_pools,
            EntityKind::ResourceDepot,
            site.0,
            site.1,
        )
        .is_some()
    {
        memory.later_bases.build_attempt_tick = Some(tick);
        status.intents.push(AiIntent::Build {
            kind: EntityKind::ResourceDepot,
        });
        return status;
    }
    // Keep the Depot's cost banked while the guards move up, so Riflemen and other spending do
    // not keep pushing the order back.
    let (steel, oil) = rts_rules::economy::cost(EntityKind::ResourceDepot);
    actions.holdback_resources(steel, oil);
    status
}

/// Tanks this close to the HQ count toward the main's reserve.
const MAIN_GARRISON_RADIUS_TILES: f32 = 16.0;

/// A later base this close to the enemy's route to the main stands on its way in, and every attack
/// passes it. On Classic the third base went 3 tiles from that route and cost 7 Tanks and the Depot
/// within 1,700 ticks; every other map's third base lies 21-62 tiles off it.
const ROUTE_EXPOSURE_TILES: f32 = 8.0;
/// Taken off such a site's score (tiles of walking head start), so a site off the route is preferred
/// unless it gives up more head start than that.
const ROUTE_EXPOSURE_PENALTY: i32 = 60;

fn route_exposure_penalty(
    observation: &AiObservation,
    route: Option<&[crate::ai_core::map_analysis::AiTile]>,
    tile: (u32, u32),
) -> i32 {
    let Some(center) = building_center(tile, EntityKind::ResourceDepot, observation.map.tile_size)
    else {
        return 0;
    };
    let ts = observation.map.tile_size.max(1) as f32;
    let exposed = route.is_some_and(|route| {
        route.iter().any(|step| {
            dist2(
                (step.x as f32 + 0.5) * ts,
                (step.y as f32 + 0.5) * ts,
                center.0,
                center.1,
            ) <= squared(ROUTE_EXPOSURE_TILES * ts)
        })
    });
    if exposed {
        -ROUTE_EXPOSURE_PENALTY
    } else {
        0
    }
}

/// Tanks at the main: near the HQ or at the home post, and neither pushing nor guarding a new base.
/// The home Tank counts. Tanks left at other bases do not: in one lost game nine Tanks were "home"
/// by count, two of them at the main, and four AI 2.1 Tanks walked in to the HQ.
pub(super) fn main_tank_ids(observation: &AiObservation, memory: &AiDecisionMemory) -> Vec<u32> {
    let ts = observation.map.tile_size as f32;
    let hq = geometry::tile_center(observation.own_start_tile, observation.map.tile_size);
    let radius2 = squared(MAIN_GARRISON_RADIUS_TILES * ts);
    free_tank_ids(observation, memory)
        .into_iter()
        .filter(|id| !memory.later_bases.guards.contains(id))
        .filter(|id| {
            observation.owned.iter().any(|unit| {
                unit.id == *id
                    && (dist2(unit.x, unit.y, hq.0, hq.1) <= radius2
                        || memory
                            .home_post
                            .is_some_and(|post| post.contains(observation, (unit.x, unit.y))))
            })
        })
        .collect()
}

/// While the main is short of its Tank reserve, idle Tanks elsewhere that are not pushing,
/// guarding a new base, covering the natural (`keep_out`) or answering a raid (`claimed`) head
/// back to it, nearest first. Released guards and Tanks rallied to a new base otherwise stay there.
pub(super) fn recall_tanks_to_main(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &AiDecisionMemory,
    claimed: &BTreeSet<u32>,
    keep_out: Option<u32>,
) -> Vec<u32> {
    let at_main = main_tank_ids(observation, memory);
    let short = memory.home_tank_reserve().saturating_sub(at_main.len());
    if short == 0 {
        return Vec::new();
    }
    let hq = geometry::tile_center(observation.own_start_tile, observation.map.tile_size);
    // They go to the home post, where the home Tanks stand, rather than to the HQ.
    let home = memory.home_post.map_or(hq, |post| post.center());
    let mut away: Vec<&AiEntitySummary> = free_tank_ids(observation, memory)
        .into_iter()
        .filter(|id| {
            !at_main.contains(id)
                && !memory.later_bases.guards.contains(id)
                && Some(*id) != keep_out
                && !claimed.contains(id)
        })
        .filter_map(|id| observation.owned.iter().find(|unit| unit.id == id))
        .filter(|unit| unit.is_complete && unit.hp > 0 && unit.state == AiEntityState::Idle)
        .collect();
    away.sort_by(|left, right| {
        dist2(left.x, left.y, hq.0, hq.1)
            .total_cmp(&dist2(right.x, right.y, hq.0, hq.1))
            .then_with(|| left.id.cmp(&right.id))
    });
    actions::move_units(
        actions,
        away.iter().take(short).map(|unit| unit.id),
        home.0,
        home.1,
    )
    .unwrap_or_default()
}

/// Tanks that are not part of a push. The home Tank counts: it is at home, not pushing.
fn free_tank_ids(observation: &AiObservation, memory: &AiDecisionMemory) -> Vec<u32> {
    observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
        .filter(|unit| {
            !memory.containment.active_tanks.contains(&unit.id)
                && !memory.containment.opening_tanks.contains(&unit.id)
                // The trap breach's Tanks hold their concave; they are not called home.
                && !memory.trap_breach.is_reserved(unit.id)
        })
        .map(|unit| unit.id)
        .collect()
}

/// Once a Depot stands on the guarded site, stop taking it and start the release countdown.
fn refresh_guard_site(observation: &AiObservation, memory: &mut AiDecisionMemory, tick: u32) {
    let state = &mut memory.later_bases;
    let Some(guard_site) = state.guard_site else {
        return;
    };
    let depot = observation.owned.iter().find(|entity| {
        entity.kind == EntityKind::ResourceDepot
            && entity.hp > 0
            && building_center(
                guard_site,
                EntityKind::ResourceDepot,
                observation.map.tile_size,
            )
            .is_some_and(|center| {
                dist2(entity.x, entity.y, center.0, center.1)
                    <= squared(observation.map.tile_size as f32)
            })
    });
    if let Some(depot) = depot {
        // The foundation exists: the order worked, so stop staging this site.
        state.site = None;
        state.staging_since = None;
        state.build_attempt_tick = None;
        state.guard_site_built = true;
        if depot.is_complete && state.guard_release_tick.is_none() {
            state.guard_release_tick = Some(tick.saturating_add(GUARD_AFTER_COMPLETE_TICKS));
        }
    } else if state.guard_site_built {
        // The Depot was destroyed while still covered. The base is due again, but not at once:
        // whatever killed it is probably still there, and a second loss rules the site out.
        if note_site_failure(state, guard_site) {
            state.rejected_sites.insert(guard_site);
        }
        state.retry_after_tick = tick.saturating_add(SITE_RETRY_COOLDOWN_TICKS);
        state.guard_site_built = false;
    }
    if state.site.is_none()
        && (depot.is_none()
            || state
                .guard_release_tick
                .is_some_and(|release| tick >= release))
    {
        // Released after cover time, or the Depot was lost; a lost base is simply due again.
        state.guard_site = None;
        state.guard_release_tick = None;
        state.guard_site_built = false;
        state.guards.clear();
    }
}

/// Count a dropped order or destroyed Depot at a site; true once the site should be rejected.
fn note_site_failure(state: &mut LaterBases, site: (u32, u32)) -> bool {
    let failures = state.site_failures.entry(site).or_insert(0);
    *failures = failures.saturating_add(1);
    *failures >= MAX_SITE_FAILURES
}

fn site_is_rejected(rejected: &BTreeSet<(u32, u32)>, tile: (u32, u32)) -> bool {
    rejected.iter().any(|site| {
        let dx = site.0 as f32 - tile.0 as f32;
        let dy = site.1 as f32 - tile.1 as f32;
        dx * dx + dy * dy <= squared(REJECTED_SITE_RADIUS_TILES)
    })
}

/// Move each guard to its slot and hold it there. These go out as ordinary moves, not staging:
/// the live adapter sends a unit only its first staging order, which would strand a Tank that
/// was staged elsewhere earlier. Guards local defense claimed this decision are left to it.
pub(super) fn issue_guard_orders(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    guard_posts: &[(u32, (f32, f32))],
    claimed_by_defense: &BTreeSet<u32>,
) -> Vec<u32> {
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    let state = &mut memory.later_bases;
    let posted: BTreeSet<u32> = guard_posts.iter().map(|(guard, _)| *guard).collect();
    state.guards_holding.retain(|guard| posted.contains(guard));
    state
        .guard_order_tick
        .retain(|guard, _| posted.contains(guard));
    let mut ordered = Vec::new();
    for &(guard, slot) in guard_posts {
        let Some(unit) = observation.owned.iter().find(|unit| unit.id == guard) else {
            continue;
        };
        if claimed_by_defense.contains(&guard) {
            // Whatever defense ordered replaces the hold; re-hold after it lets go.
            state.guards_holding.remove(&guard);
            continue;
        }
        let at_slot = dist2(unit.x, unit.y, slot.0, slot.1) <= squared(GUARD_ARRIVAL_TILES * ts);
        if at_slot {
            if !state.guards_holding.contains(&guard) || unit.state == AiEntityState::Move {
                if let Some(units) = actions::hold_position_units(actions, [guard]) {
                    ordered.extend(units);
                }
                state.guards_holding.insert(guard);
            }
            continue;
        }
        state.guards_holding.remove(&guard);
        let refresh_due = state
            .guard_order_tick
            .get(&guard)
            .is_none_or(|last| tick.saturating_sub(*last) >= GUARD_REORDER_TICKS);
        if unit.state != AiEntityState::Move || refresh_due {
            if let Some(units) = actions::move_units(actions, [guard], slot.0, slot.1) {
                ordered.extend(units);
            }
            state.guard_order_tick.insert(guard, tick);
        }
    }
    ordered
}

/// Give up on a site for now: release the guards and wait out the cooldown. A rejected site is
/// never chosen again.
fn abandon_site(state: &mut LaterBases, site: (u32, u32), tick: u32, reject: bool) {
    if reject {
        state.rejected_sites.insert(site);
    }
    state.retry_after_tick = tick.saturating_add(SITE_RETRY_COOLDOWN_TICKS);
    state.site = None;
    state.guard_site = None;
    state.guard_site_built = false;
    state.staging_since = None;
    state.build_attempt_tick = None;
    state.guard_release_tick = None;
    state.guards.clear();
}

/// Drop guards that died or became the home Tank; true when one died.
fn drop_lost_guards(observation: &AiObservation, memory: &mut AiDecisionMemory) -> bool {
    let alive: BTreeSet<u32> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && unit.hp > 0)
        .map(|unit| unit.id)
        .collect();
    let home_tank = memory.home_defensive_tank;
    let guards = &mut memory.later_bases.guards;
    let before = guards.len();
    guards.retain(|id| alive.contains(id));
    let died = guards.len() < before;
    guards.retain(|id| Some(*id) != home_tank);
    died
}

/// Pick the two Tanks nearest the site that neither the push nor home defense is using. Guards are
/// chosen once per site: losses are not replaced, because that would send Tanks in one at a time.
fn select_guards(observation: &AiObservation, memory: &mut AiDecisionMemory, site: (f32, f32)) {
    let home_tank = memory.home_defensive_tank;
    let mut candidates: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
        .filter(|unit| {
            Some(unit.id) != home_tank
                && !memory.containment.active_tanks.contains(&unit.id)
                && !memory.containment.opening_tanks.contains(&unit.id)
        })
        .collect();
    candidates.sort_by(|left, right| {
        dist2(left.x, left.y, site.0, site.1)
            .total_cmp(&dist2(right.x, right.y, site.0, site.1))
            .then_with(|| left.id.cmp(&right.id))
    });
    memory.later_bases.guards = candidates
        .iter()
        .take(GUARD_TANKS)
        .map(|unit| unit.id)
        .collect();
}

/// Guard slots in front of the site, pulled back toward the site until the ground is open.
fn guard_post_slots(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    site: (f32, f32),
    enemy: (f32, f32),
) -> Vec<(f32, f32)> {
    let ts = observation.map.tile_size as f32;
    let direction = normalized_direction(site, enemy).unwrap_or((0.0, 1.0));
    let lateral = (-direction.1, direction.0);
    let mut slots = Vec::new();
    for side in [1.0, -1.0] {
        let mut chosen = None;
        let mut distance = GUARD_POST_TILES;
        while distance >= 2.0 {
            let point = clamp_to_map(
                (
                    site.0 + (direction.0 * distance + lateral.0 * side * GUARD_LATERAL_TILES) * ts,
                    site.1 + (direction.1 * distance + lateral.1 * side * GUARD_LATERAL_TILES) * ts,
                ),
                observation.map,
            );
            if defense::defensive_position_is_open(observation, map_analysis, point.0, point.1) {
                chosen = Some(point);
                break;
            }
            distance -= 1.0;
        }
        slots.push(chosen.unwrap_or(site));
    }
    slots
}

/// Record live readings of the wells under Jeff's Pump Jacks, and mark a base the first time one
/// of its wells is about to run dry.
fn note_dry_wells(observation: &AiObservation, state: &mut LaterBases) {
    let ts = observation.map.tile_size as f32;
    let depots: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|entity| entity.kind == EntityKind::ResourceDepot && entity.is_complete)
        .filter(|entity| entity.hp > 0)
        .collect();
    for pump in observation
        .owned
        .iter()
        .filter(|entity| entity.kind == EntityKind::PumpJack && entity.is_complete && entity.hp > 0)
    {
        let Some(well) = observation.resources.iter().find(|resource| {
            resource.kind == EntityKind::Oil
                && dist2(resource.x, resource.y, pump.x, pump.y) <= squared(ts)
        }) else {
            continue;
        };
        // Only trust a reading that continues the well's own steady decline. A well that drops out
        // of sight reports a placeholder (1), which must neither count as nearly dry nor replace
        // the last live reading; a first reading is trusted only while the well is still healthy.
        let trusted = match state.well_remaining.get(&well.id) {
            Some(&previous) => {
                previous >= well.remaining && previous - well.remaining <= MAX_PLAUSIBLE_WELL_DROP
            }
            None => well.remaining > DRY_WELL_WARNING_REMAINING,
        };
        if !trusted {
            continue;
        }
        state.well_remaining.insert(well.id, well.remaining);
        if well.remaining > DRY_WELL_WARNING_REMAINING {
            continue;
        }
        let owner = depots
            .iter()
            .filter(|depot| {
                dist2(depot.x, depot.y, pump.x, pump.y) <= squared(BASE_MINING_RANGE_TILES * ts)
            })
            .min_by(|left, right| {
                dist2(left.x, left.y, pump.x, pump.y)
                    .total_cmp(&dist2(right.x, right.y, pump.x, pump.y))
                    .then_with(|| left.id.cmp(&right.id))
            });
        if let Some(depot) = owner {
            state.unlocking_depots.insert(depot.id);
        }
    }
}

#[cfg(test)]
mod tests;
