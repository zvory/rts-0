//! Jeff's AI V1.1's Panzerfaust pack (`pfpack`). A Panzerfaust's one rocket does a Tank 63 (five
//! kill it) and costs 55 Steel and 10 Oil, so eight of them cost about what one Tank's Oil does.
//! In the Panzerfaust lab (`ai-pf-lab`, 2026-11-24), trading one of four Tanks for eight
//! Panzerfausts turned a 4-Tank fight from +110 to +727, but only when every loaded Panzerfaust
//! went for the same Tank at once: standing still killed next to nothing, and splitting up to come
//! in from the sides lost. In human games, Panzerfausts that stood under a parked Tank's fire fired
//! once for every three that died, and those that closed in fired twice for every one.
//!
//! - **Research**: Panzerfausts once Entrenchment is ordered, from `RESEARCH_TICKS` (4:15), or
//!   sooner with an enemy Tank in sight; until it is ordered its cost is held back. Pressure AI's
//!   first Tank comes out at 5:00-5:30, so waiting to see one put the research at 5:30-6:00.
//!   `pflate` waits for 8:00 and spare Oil instead, `pffloat` for `FLOAT_OIL` banked (see `late`).
//! - **Ready (`pfready`)**: the pack waits at home until it has a charge's worth loaded.
//! - **Count**: once an enemy Tank or Factory has been seen, `BASE_FAUSTS` loaded and
//!   `FAUSTS_PER_TANK` more for each enemy Tank remembered, at most `MAX_FAUSTS`, trained ahead of
//!   the Riflemen (`rifle_line`); spent ones are replaced, but no more than `MAX_TRAINED` are
//!   ever trained. Replacing every loss without a limit, the pack lost 42-71 Panzerfausts a game
//!   against Pressure AI for 0-3 Tanks killed by rockets.
//! - **Rest**: one pack, `BACK_TILES` behind Jeff's biggest group of Tanks, each Panzerfaust on its
//!   own slot; it digs in wherever the group stands. Rested behind the biggest Tank station
//!   instead, it stood 16-60 tiles from every Tank fight of 5 games against Pressure AI. `pfpush`
//!   rests it behind the push's Riflemen while the push is out; `pfsafe` keeps it from walking
//!   past enemy Tanks on its way.
//! - **Charge**: an enemy Tank in sight within `CONTACT_TILES` of a loaded Panzerfaust and
//!   `LEASH_TILES` of the pack's rest place is charged by every loaded Panzerfaust within
//!   `REACH_TILES` of it, when there are `ROCKETS_PER_TANK` of them for every enemy Tank within
//!   `COVER_TILES` of it (their machine guns out-range the rocket) and at least `NEED_CLOSE`
//!   (`NEED_FAR` for a Tank farther than `CLOSE_TILES`), `SUPPORT_DISCOUNT` fewer with a Jeff Tank
//!   near it. An escorted Tank (`MAX_ESCORT` infantry near it) needs `ESCORT_EXTRA` more. Each
//!   charging Panzerfaust goes for the nearest Tank of the charged group still short of the
//!   rockets that kill it, and keeps it while it is in sight within `LEASH_TILES`. Charging any
//!   Tank in contact, 19 Panzerfausts ran into a line of 10 and fired one rocket. `pfwave` sends
//!   the Riflemen near the charged Tanks in first and holds the Panzerfausts until a Tank's cannon
//!   is busy with them.
//! - **Spent**: a Panzerfaust that has fired walks home and stays out of the fighting.
//! - **Ledger (`pfcap`)**: past its first `LEDGER_FAUSTS`, the pack trains only from the Oil and
//!   Steel its rockets have destroyed (see `ledger`).
//!
//! The pack owns its Panzerfausts: it runs after every other system and takes their orders away.
//!
//! **Base guard (`pfdef`)**, in place of the pack: the Panzerfausts guard Jeff's bases against Tank
//! raids. A Tank's cannon takes an enemy Tank in its 5-tile reach over any infantry, so while an
//! enemy Tank trades shots with a Jeff Tank, Panzerfausts running in face only its machine gun.
//! Against Pressure AI (120 games, 2026-11-24), enemy Tanks within 12 tiles of Jeff's Depots killed
//! 124 Jeff Tanks, 1,022 Riflemen and 163 buildings, and 135 enemy Tanks died there.
//! - **Research**: once an enemy Tank has been seen (`pfoil`: only from the Oil past a Tank's,
//!   unless an enemy Tank is at a base).
//! - **Count**: `GUARD_FAUSTS` loaded, spent ones replaced, at most `GUARD_MAX_TRAINED` a game.
//! - **Rest**: behind the Depot an enemy Tank in sight is nearest to (within `THREAT_TILES`, kept
//!   `THREAT_MEMORY_TICKS`), else behind the most central Depot, facing the threat or the enemy.
//! - **Charge**: an enemy Tank within `HOME_RAID_TILES` of a Depot with a Jeff Tank within
//!   `ENGAGE_TILES` of it, and no enemy Tank within `COVER_TILES` of it that is free of Jeff's
//!   Tanks, is charged by the loaded Panzerfausts within `GUARD_REACH_TILES`: the rockets its HP
//!   needs, one more for every other enemy Tank's machine gun near it, one more if escorted, plus a
//!   spare. A charging Panzerfaust drops its Tank once no Jeff Tank is on it, unless it is within
//!   `COMMIT_TILES` already.

use super::*;
use crate::ai_core::decision::geometry::{clamp_to_map, dist2, squared, tile_center};
use crate::ai_core::observation::AiEntityState;
use rts_rules::math::GameplayMath as _;
use rts_sim::game::command::SimCommand as Command;

mod guard;
mod rest;
mod safe;

use guard::{guard_orders, GuardCharge};
use rest::{push_rest, rest_place, slot_point};
use safe::safe_send;

/// A rest place: its centre and the unit vector it faces.
type Rest = ((f32, f32), (f32, f32));
/// `pfdef`: the guarded Depot's position, the enemy Tank's position it was guarded against (world
/// pixels), and the tick.
type GuardPost = ((i32, i32), (i32, i32), u32);

const RESEARCH_TICKS: u32 = config::TICK_HZ * 255;
const BASE_FAUSTS: usize = 5;
const FAUSTS_PER_TANK: usize = 5;
const MAX_FAUSTS: usize = 16;
const CONTACT_TILES: f32 = 16.0;
const CLOSE_TILES: f32 = 10.0;
const NEED_CLOSE: usize = 5;
const NEED_FAR: usize = 7;
/// A Jeff Tank this close to the target fights it too.
const SUPPORT_TILES: f32 = 12.0;
const SUPPORT_DISCOUNT: usize = 2;
const REACH_TILES: f32 = 18.0;
const LEASH_TILES: f32 = 22.0;
const ESCORT_TILES: f32 = 6.0;
const MAX_ESCORT: usize = 4;
const ESCORT_EXTRA: usize = 5;
/// Enemy Tanks this close to a charged Tank cover it with their machine guns (6 tiles).
const COVER_TILES: f32 = 8.0;
/// Rockets a charge needs for each covering Tank (five kill one).
const ROCKETS_PER_TANK: usize = 5;
/// Panzerfausts trained over the whole game, spent and lost ones included.
const MAX_TRAINED: usize = 24;
/// A charge gathers this far from its Tanks (their machine guns reach 6 tiles)...
const STAGE_TILES: f32 = 8.0;
/// ... and goes in once `need` Panzerfausts stand within this much more of it,
const STAGE_SLACK_TILES: f32 = 1.5;
/// ... a Tank comes this close to one of them,
const LAUNCH_TILES: f32 = 7.0;
/// ... or this long after it began.
const GATHER_TICKS: u32 = config::TICK_HZ * 8;
const ESCORT_MEMORY_TICKS: u32 = config::TICK_HZ * 20;
const ROCKET_DAMAGE: u32 = 63;
/// The pack's first row stands this far behind its centre (its Tank group's).
const BACK_TILES: f32 = 4.0;
const SLOT_SPACING_TILES: f32 = 1.2;
const SLOT_ROW: usize = 4;
const ON_SLOT_TILES: f32 = 1.5;
/// A Panzerfaust walking to its slot is sent again at most this often unless it stands idle.
const REORDER_TICKS: u32 = config::TICK_HZ * 8;
const RESEND_TILES: f32 = 3.0;
/// Spent Panzerfausts stand about this far from the start toward the enemy.
const HOME_TILES: f32 = 4.0;
/// Tanks this close to one another are one group.
const GROUP_TILES: f32 = 10.0;
/// The pack keeps to the group nearest its last place within this many tiles...
const FOLLOW_TILES: f32 = 15.0;
/// ... until another group holds this many more Tanks.
const SWITCH_MARGIN: usize = 2;
/// The pack faces the nearest enemy Tank in sight within this many tiles of its group.
const FACE_TILES: f32 = 25.0;
/// A resting Panzerfaust shooting at an enemy this close is left to it.
const FIGHTING_TILES: f32 = 7.0;
/// With no Tank yet, the pack's first row stands this far from the forward Depot toward the
/// enemy.
const DEPOT_AHEAD_TILES: f32 = 5.0;

/// `pfdef`: loaded Panzerfausts the base guard keeps.
const GUARD_FAUSTS: usize = 7;
/// `pfdef`: Panzerfausts the base guard trains over the whole game, spent and lost ones included.
const GUARD_MAX_TRAINED: usize = 15;
/// `pfdef`: an enemy Tank this close to a Depot of Jeff's is raiding his base.
const HOME_RAID_TILES: f32 = 18.0;
/// `pfdef`: a Jeff Tank this close to an enemy Tank draws its cannon (5 tiles).
const ENGAGE_TILES: f32 = 5.5;
/// `pfdef`: loaded Panzerfausts this close to an engaged enemy Tank run in.
const GUARD_REACH_TILES: f32 = 26.0;
/// `pfdef`: a charging Panzerfaust lets its Tank go beyond this.
const GUARD_LEASH_TILES: f32 = 32.0;
/// `pfdef`: a charging Panzerfaust this close to its Tank keeps going once no Jeff Tank is on it.
const COMMIT_TILES: f32 = 7.0;
/// `pfdef`: an enemy Tank this close to a Depot makes it the guarded base...
const THREAT_TILES: f32 = 30.0;
/// ... for this long after it was last seen there.
const THREAT_MEMORY_TICKS: u32 = config::TICK_HZ * 30;
/// `pfdef`: a charge keeps an enemy Tank this much farther from the Depots.
const RAID_SLACK_TILES: f32 = 4.0;
/// `pfdef`: a charge gathers this far from its Tank: ranges count from the bodies' edges, so a
/// Tank's machine gun (6 tiles) reaches about 7 tiles between centres...
const GUARD_STAGE_TILES: f32 = 9.0;
/// ... and goes in once one of them is this close to it.
const GUARD_LAUNCH_TILES: f32 = 7.5;
/// `pfjoin`: Jeff Tanks this close to a raiding Tank go in with the Panzerfausts...
const JOIN_TILES: f32 = 14.0;
/// ... unless more enemy infantry than this stand by it...
const MAX_JOIN_ESCORT: usize = 3;
/// ... and the Panzerfausts give up waiting for them after this long.
const JOIN_TICKS: u32 = config::TICK_HZ * 15;
/// `pfdef`: an enemy Tank a Jeff Tank holds with this few rockets left is left to it...
const FINISH_ROCKETS: usize = 2;
/// ... and one with more enemy Tanks than this within `DANGER_TILES` is not charged: Tanks 8-12
/// tiles off closed in at 2 tiles/s and shot the charge down (5-8 Tank groups on Afrika, 3 Player Map).
const MAX_NEAR_TANKS: usize = 2;
const DANGER_TILES: f32 = 12.0;
/// `pfdef`: a resting Panzerfaust with an enemy Tank this close walks this far away from it...
const SAFE_TILES: f32 = 9.0;
const FLEE_TILES: f32 = 5.0;
/// ... and one whose way to its slot passes this close to an enemy Tank stays where it is.
const PATH_CLEAR_TILES: f32 = 8.0;
/// `pflock`: at most this many other enemy Tanks within `LOCK_NEAR_TILES` of a locked raider (their
/// machine guns reach the run-in).
const LOCK_NEAR_TILES: f32 = 8.0;
const MAX_LOCK_NEAR: usize = 1;
/// `pfdef`: enemy Tanks seen this recently near a charge's target count as near it.
const HIDDEN_TANK_TICKS: u32 = config::TICK_HZ * 10;
/// `pfdef`: the guard stands and gathers this far apart (a Tank shell splashes 1.4 tiles).
const GUARD_SPACING_TILES: f32 = 3.0;

/// `pfscreen`: Riflemen the pack keeps in front of it. A Tank's cannon takes the nearest infantry
/// (Riflemen and Panzerfausts alike), so Riflemen running in ahead draw its shells. In ai-pf-lab
/// (2026-11-25) two Riflemen leading by 1.5-2.5 s doubled a 2-Tank fight (net +378 -> +785, Tanks
/// killed 0.7 -> 1.7) and took the shell splash off the Panzerfausts; a 5 s lead lost the screen
/// first.
const SCREEN_RIFLES: usize = 2;
/// ... recruited from this close to the pack's rest place while it has `NEED_CLOSE` loaded,
const SCREEN_RECRUIT_TILES: f32 = 20.0;
/// ... resting this far in front of its first row, this far apart,
const SCREEN_AHEAD_TILES: f32 = 1.5;
const SCREEN_SPACING_TILES: f32 = 1.5;
/// ... running in to this far short of the charged Tank on the Panzerfausts' side,
const SCREEN_SHORT_TILES: f32 = 2.0;
/// ... and a charging Panzerfaust waits until the screen is this much closer to its Tank than it
/// is (or the screen is in place). A fixed 2 s lead let Panzerfausts 10 tiles out run in ahead of
/// a screen 16 tiles out (Afrika, 16 of 16 lost).
const SCREEN_LEAD_TILES: f32 = 3.0;
/// A Panzerfaust this close to its Tank (in its machine gun's reach) goes in at once...
const SCREEN_UNDER_FIRE_TILES: f32 = 7.0;
/// ... and none waits longer than this.
const SCREEN_WAIT_TICKS: u32 = config::TICK_HZ * 5;
/// A screen Rifleman is sent again once its Tank has moved this far.
const SCREEN_RESEND_TILES: f32 = 3.0;

/// `pfdef`: the Panzerfausts guard Jeff's bases instead of following his Tanks.
fn guard() -> bool {
    !super::off("pfdef")
}

/// `pfscreen`: see `SCREEN_RIFLES`.
fn screen() -> bool {
    !super::off("pfscreen")
}

/// `pfcap`: the pack trains only from the Oil it has destroyed. Its Oil account (and Steel, `pfcaps`) starts at
/// `LEDGER_FAUSTS` Panzerfausts' cost, pay each Panzerfaust trained, and earn `ROCKET_OIL` and
/// `ROCKET_STEEL` for each rocket fired. Against Pressure AI (240 games, 2026-11-25) the pack's
/// Panzerfausts returned 1.2 times their Oil but 0.48 times their Steel, and a third of the
/// Panzerfausts in a wipe died without firing.
fn ledger() -> bool {
    !super::off("pfcap")
}

/// `pfcaps`: the Steel account counts too. A rocket's Steel (`ROCKET_STEEL`) is less than a
/// Panzerfaust's (55), so the Steel account only ever shrinks: in 24 games (2026-11-25) the pack
/// charged once and trained 3.3 Panzerfausts a game.
fn steel_ledger() -> bool {
    !super::off("pfcaps")
}

/// `pflate`: the research waits for `LATE_TICKS` and the Oil past its own and a Tank's, with no
/// early exception for an enemy Tank in sight and nothing held back meanwhile; Panzerfausts are
/// trained only from the Steel and Oil past a Tank's and their own. Against Pressure AI (120
/// games, 2026-11-25) the research at a median 4:30, with Jeff's Oil at 160-280, cost the pack
/// more Oil than its rockets destroyed, and games where no Panzerfaust ever fought still lost
/// from minute 4-6; from minute 9 on Jeff banked 480-1,000 Oil.
fn late() -> bool {
    !super::off("pflate")
}

/// `pffloat`, in place of `pflate`: the research waits only for `FLOAT_OIL` banked Oil.
fn float() -> bool {
    !super::off("pffloat")
}

/// `pfready`: the pack waits at home, charging nothing, until `READY_FAUSTS` are loaded, and goes
/// back once fewer than `NEED_CLOSE` are; no charge needs fewer than `NEED_CLOSE` (no
/// `SUPPORT_DISCOUNT`). In 409 pack fights against Pressure AI (2026-11-25), fights with 0-4 of
/// Jeff's Panzerfausts alive returned 0.47 times their Oil (105 of 143 died before firing), with 5-8
/// alive 1.27 times; late research filled the pack to 5 only 2 minutes after the first.
fn ready_mode() -> bool {
    !super::off("pfready")
}

/// `pffill`: with `pflate` or `pffloat`, Panzerfausts are trained from the Oil past a Tank's
/// and their own, whatever Steel is left: from 8:00 Jeff had the Oil 87-100% of the time but the
/// Steel for a Tank and a Panzerfaust only 45-67%.
fn fill() -> bool {
    !super::off("pffill")
}

/// `pfwave`: a charge sends the Riflemen near its Tanks in first, and its Panzerfausts hold where
/// they gathered until the Tank's cannon is on a Rifleman (or a Jeff Tank, or a building) or a
/// Rifleman is at it. A Tank's cannon takes the nearest infantry and keeps it, so a Rifleman at
/// point blank draws the shell and its splash that killed 61% of the HP Jeff's Panzerfausts lost
/// in the late pack's fights against Pressure AI (2026-11-25: 451 of 775 died before firing).
fn wave() -> bool {
    !super::off("pfwave")
}

/// `pfpush`: while Jeff's push is out, the pack rests behind its Riflemen (instead of behind
/// Jeff's biggest group of Tanks) and, with `pfready`, joins it with `NEED_CLOSE` loaded.
fn push_mode() -> bool {
    !super::off("pfpush")
}

/// `pfsafe`: the pack's resting and spent Panzerfausts never walk past an enemy Tank (the base
/// guard's safe moves, `SAFE_TILES`/`FLEE_TILES`/`PATH_CLEAR_TILES`). 52 of the 73 Panzerfausts
/// the late pack lost outside its charges in 8 traced wipes (2026-11-26) died within 30 s of its
/// rest place jumping 20-140 tiles, to another Tank group or home, walking past enemy Tanks that a
/// charge could not take (out of `LEASH_TILES` of the new rest place).
fn safe() -> bool {
    !super::off("pfsafe")
}

/// `pfwave`: Riflemen sent in per enemy Tank of the charge, at least `WAVE_MIN`, at most
/// `WAVE_MAX`, taken from those within `WAVE_RECRUIT_TILES` of its Tanks, nearest first.
const WAVE_PER_TANK: usize = 2;
const WAVE_MIN: usize = 3;
const WAVE_MAX: usize = 8;
const WAVE_RECRUIT_TILES: f32 = 12.0;
/// `pfwave`: a Tank with a Rifleman of the wave this close is at grips with it.
const WAVE_AT_TILES: f32 = 3.0;
/// `pfwave`: the Panzerfausts wait for the wave at most this long.
const WAVE_WAIT_TICKS: u32 = config::TICK_HZ * 8;

/// `pfready`: loaded Panzerfausts the pack waits for before it leaves home.
const READY_FAUSTS: usize = NEED_FAR;
/// `pfready`: the waiting pack rests this far past `HOME_TILES` toward the enemy.
const WAIT_AHEAD_TILES: f32 = 3.0;

/// `pflate`: the research is not ordered before this (8:00).
const LATE_TICKS: u32 = config::TICK_HZ * 480;
/// `pffloat`: the research is ordered once this much Oil is free.
const FLOAT_OIL: u32 = 400;

/// `pfcap`: the Panzerfausts the accounts start with.
const LEDGER_FAUSTS: i64 = BASE_FAUSTS as i64;
/// `pfcap`: a rocket's worth: on average a rocket fired took 39 HP off a Tank (2,592 rockets,
/// overkill and misses counted), 13% of its 300 HP, so 18 of its 145 Oil and 43 of its 350 Steel.
const ROCKET_OIL: i64 = 18;
const ROCKET_STEEL: i64 = 43;

/// `pfcap`: the Panzerfausts the accounts still pay for, none when that would not bring the pack's
/// loaded Panzerfausts up to `NEED_CLOSE` (too few to charge, they would only stand).
fn affordable(pack: &FaustPackMemory) -> usize {
    let (steel, oil) = rts_rules::economy::cost(EntityKind::Panzerfaust);
    let (steel, oil) = (i64::from(steel.max(1)), i64::from(oil.max(1)));
    let trained = pack.trained as i64;
    let fired = pack.fired as i64;
    let oil_left = (LEDGER_FAUSTS * oil + ROCKET_OIL * fired - oil * trained).max(0);
    let steel_left = (LEDGER_FAUSTS * steel + ROCKET_STEEL * fired - steel * trained).max(0);
    let afford = if steel_ledger() {
        (oil_left / oil).min(steel_left / steel)
    } else {
        oil_left / oil
    } as usize;
    let loaded = pack.alive.saturating_sub(pack.spent);
    if loaded + afford < NEED_CLOSE {
        0
    } else {
        afford
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct FaustPackMemory {
    /// An enemy Tank or Factory has been seen.
    armor_seen: bool,
    /// The centre of the Tank group the pack rested behind at the last decision.
    last: Option<(i32, i32)>,
    /// Each loaded Panzerfaust's slot in the pack.
    slots: BTreeMap<u32, usize>,
    /// When each Panzerfaust was last sent to its rest place, and the place.
    sent: BTreeMap<u32, (u32, (i32, i32))>,
    /// The Tank each charging Panzerfaust goes for.
    charging: BTreeMap<u32, u32>,
    /// Spent Panzerfausts alive at the last decision.
    spent: usize,
    /// Panzerfausts alive at the last decision.
    alive: usize,
    /// The Panzerfausts alive at the last decision, by id.
    known: BTreeSet<u32>,
    /// Panzerfausts trained so far.
    trained: usize,
    /// Whether the charge under way has gone in (`pfgather`).
    launched: bool,
    /// When the charge under way began, and the rockets it needed.
    charge_start: u32,
    need: usize,
    /// `pfdef`: an enemy Tank has been seen.
    tank_seen: bool,
    /// `pfdef`: an enemy Tank is in sight within `HOME_RAID_TILES` of a Depot of Jeff's.
    raid: bool,
    /// `pfdef`: the guarded Depot, the enemy Tank it was guarded against, and when.
    guard: Option<GuardPost>,
    /// `pfdef`: whether the guarded Depot is raided (an enemy Tank within `HOME_RAID_TILES`).
    guard_raid: bool,
    /// `pfdef`: the charges under way, by enemy Tank.
    charges: BTreeMap<u32, GuardCharge>,
    /// `pfjoin`: Jeff Tanks going in with a charge, and its enemy Tank.
    joined: BTreeMap<u32, u32>,
    /// `pfscreen`: the pack's screen Riflemen.
    screen: BTreeSet<u32>,
    /// `pfscreen`: when the screen went in for the charge under way.
    screen_go: Option<u32>,
    /// `pfscreen`: each screen Rifleman's Tank, and where it was when the Rifleman was sent.
    screen_to: BTreeMap<u32, (u32, (i32, i32))>,
    /// `pfcap`: the loaded Panzerfausts at the last decision, and the rockets fired so far.
    loaded_last: BTreeSet<u32>,
    fired: usize,
    /// `pfready`: the pack has had `READY_FAUSTS` loaded since it last fell below `NEED_CLOSE`.
    ready: bool,
    /// `pfwave`: the Riflemen sent in first for the charge under way, and whether a Tank of it has
    /// been at grips with them (the Panzerfausts go from then on).
    wave: BTreeSet<u32>,
    wave_open: bool,
}

/// Whether the pack (or the `pfdef` base guard) runs for this profile.
pub(in crate::ai_core::decision) fn active(profile_id: &str) -> bool {
    is_jeff_v1_1(profile_id) && (!super::off("pfpack") || guard())
}

/// Jeff's complete Depots.
fn depots(observation: &AiObservation) -> Vec<(f32, f32)> {
    observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.is_complete && unit.hp > 0)
        .map(|unit| (unit.x, unit.y))
        .collect()
}

/// Whether an enemy Tank in sight is within `HOME_RAID_TILES` of a Depot of Jeff's.
fn home_raid(observation: &AiObservation) -> bool {
    let ts = observation.map.tile_size as f32;
    let depots = depots(observation);
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::Tank && enemy.hp > 0)
        .any(|tank| {
            depots.iter().any(|depot| {
                dist2(tank.x, tank.y, depot.0, depot.1) <= squared(HOME_RAID_TILES * ts)
            })
        })
}

/// Orders the Panzerfaust research when it is due (see the module docs).
pub(in crate::ai_core::decision) fn research(
    (actions, facts, memory, intents): (
        &mut AiActionContext<'_>,
        &AiFacts,
        &mut AiDecisionMemory,
        &mut Vec<AiIntent>,
    ),
    observation: &AiObservation,
    profile_id: &str,
) {
    if !active(profile_id) {
        return;
    }
    let tank_in_sight = observation
        .visible_enemies
        .iter()
        .any(|enemy| enemy.hp > 0 && enemy.kind == EntityKind::Tank);
    let factory_in_sight = observation
        .visible_enemies
        .iter()
        .any(|enemy| enemy.hp > 0 && enemy.kind == EntityKind::Factory);
    if tank_in_sight || factory_in_sight || super::push::remembered_tanks(memory) > 0 {
        memory.jeff_v1_1.faust_pack.armor_seen = true;
    }
    let upgrade = UpgradeKind::Panzerfausts;
    if guard() {
        let seen = tank_in_sight || super::push::remembered_tanks(memory) > 0;
        let raid = home_raid(observation);
        let pack = &mut memory.jeff_v1_1.faust_pack;
        pack.tank_seen |= seen;
        pack.raid = raid;
        if !memory.jeff_v1_1.faust_pack.tank_seen
            || super::ordered(facts, memory, upgrade)
            || !super::ordered(facts, memory, UpgradeKind::Entrenchment)
            || facts.complete_building_count(EntityKind::TrainingCentre) == 0
        {
            return;
        }
        let definition = upgrade::definition(upgrade);
        let (_, tank_oil) = rts_rules::economy::cost(EntityKind::Tank);
        if !off("pfoil") && !raid && actions.budget().oil() < definition.cost_oil + tank_oil {
            return;
        }
        if !super::research(actions, facts, memory, intents, upgrade) {
            actions.holdback_resources(definition.cost_steel, definition.cost_oil);
        }
        return;
    }
    if super::ordered(facts, memory, upgrade)
        || !super::ordered(facts, memory, UpgradeKind::Entrenchment)
        || facts.complete_building_count(EntityKind::TrainingCentre) == 0
    {
        return;
    }
    let definition = upgrade::definition(upgrade);
    if late() || float() {
        let (_, tank_oil) = rts_rules::economy::cost(EntityKind::Tank);
        let oil = actions.budget().oil();
        let due = if float() {
            oil >= FLOAT_OIL
        } else {
            observation.tick >= LATE_TICKS && oil >= definition.cost_oil.saturating_add(tank_oil)
        };
        if due {
            super::research(actions, facts, memory, intents, upgrade);
        }
        return;
    }
    let due = tank_in_sight || observation.tick >= RESEARCH_TICKS;
    // `pfoil`: with no enemy Tank in sight, the research waits for the Oil past a Tank's, and holds
    // nothing back meanwhile. Researched at 4:15 with Jeff's Oil at 0-20, its 100 Oil and the
    // Panzerfausts' came out of Tanks: Jeff pushed at 9:00 with 6 Tanks instead of 9.
    let (_, tank_oil) = rts_rules::economy::cost(EntityKind::Tank);
    if !off("pfoil")
        && !tank_in_sight
        && actions.budget().oil() < definition.cost_oil.saturating_add(tank_oil)
    {
        return;
    }
    if !due || super::research(actions, facts, memory, intents, upgrade) {
        return;
    }
    actions.holdback_resources(definition.cost_steel, definition.cost_oil);
}

/// The Panzerfausts the pack trains toward, spent ones included so they are replaced.
pub(in crate::ai_core::decision) fn target(
    profile_id: &str,
    facts: &AiFacts,
    memory: &AiDecisionMemory,
) -> usize {
    let pack = &memory.jeff_v1_1.faust_pack;
    if !active(profile_id)
        || !(pack.armor_seen || pack.tank_seen)
        || !facts
            .completed_upgrades()
            .contains(&UpgradeKind::Panzerfausts)
    {
        return 0;
    }
    if guard() {
        return (GUARD_FAUSTS + pack.spent)
            .min(pack.alive + GUARD_MAX_TRAINED.saturating_sub(pack.trained));
    }
    let tanks = super::push::remembered_tanks(memory);
    let want = (BASE_FAUSTS + FAUSTS_PER_TANK * tanks)
        .min(MAX_FAUSTS)
        .max(if ready_mode() { READY_FAUSTS } else { 0 })
        + pack.spent;
    let want = want.min(pack.alive + MAX_TRAINED.saturating_sub(pack.trained));
    if ledger() {
        want.min(pack.alive + affordable(pack))
    } else {
        want
    }
}

/// Caps Panzerfaust training, queued ones counted, at `target`'s count: the trainer counts only
/// Panzerfausts out of the Barracks, so every free Barracks queued more while some trained.
pub(in crate::ai_core::decision) fn cap(
    (actions, facts, memory): (&AiActionContext<'_>, &AiFacts, &AiDecisionMemory),
    profile_id: &str,
    fausts: usize,
    max_counts: &mut Vec<(EntityKind, usize)>,
) {
    if !active(profile_id) {
        return;
    }
    let (steel, oil) = (actions.budget().steel(), actions.budget().oil());
    let built = facts.unit_count(EntityKind::Panzerfaust);
    // `pfoil`: Panzerfausts are trained only from the Steel and Oil past a Tank's and their own
    // (the base guard's also while an enemy Tank raids a base).
    let (tank_steel, tank_oil) = rts_rules::economy::cost(EntityKind::Tank);
    let (faust_steel, faust_oil) = rts_rules::economy::cost(EntityKind::Panzerfaust);
    let spare = if fill() && (late() || float()) {
        steel >= faust_steel && oil >= tank_oil.saturating_add(faust_oil)
    } else {
        steel >= tank_steel.saturating_add(faust_steel) && oil >= tank_oil.saturating_add(faust_oil)
    } || (guard() && memory.jeff_v1_1.faust_pack.raid);
    let spare_only = !off("pfoil") || late() || float();
    let cap = if !spare_only || spare {
        fausts
    } else {
        fausts.min(built)
    };
    max_counts.retain(|(kind, _)| *kind != EntityKind::Panzerfaust);
    max_counts.push((EntityKind::Panzerfaust, cap));
}

fn unit_vector(from: (f32, f32), to: (f32, f32)) -> Option<(f32, f32)> {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length = (dx * dx + dy * dy).det_sqrt();
    (length > 1.0).then(|| (dx / length, dy / length))
}

/// The nearest living enemy start, in world pixels.
fn enemy_start(observation: &AiObservation) -> Option<(f32, f32)> {
    let own_team = observation
        .players
        .iter()
        .find(|player| player.id == observation.player_id)
        .map(|player| player.team_id);
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    observation
        .players
        .iter()
        .filter(|player| player.id != observation.player_id && player.is_alive)
        .filter(|player| own_team.is_none_or(|team| player.team_id != team))
        .map(|player| tile_center(player.start_tile, observation.map.tile_size))
        .min_by(|a, b| dist2(a.0, a.1, home.0, home.1).total_cmp(&dist2(b.0, b.1, home.0, home.1)))
}

/// Enemy infantry near `at`: in sight now, or remembered there within `ESCORT_MEMORY_TICKS`.
fn escort_near(observation: &AiObservation, memory: &AiDecisionMemory, at: (f32, f32)) -> usize {
    let ts = observation.map.tile_size as f32;
    let reach = squared(ESCORT_TILES * ts);
    let infantry = |kind: EntityKind| {
        matches!(
            kind,
            EntityKind::Rifleman | EntityKind::MachineGunner | EntityKind::Panzerfaust
        )
    };
    let seen: BTreeSet<u32> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && infantry(enemy.kind))
        .filter(|enemy| dist2(enemy.x, enemy.y, at.0, at.1) <= reach)
        .map(|enemy| enemy.id)
        .collect();
    let remembered = memory
        .jeff_v1_1
        .push
        .remembered()
        .filter(|(id, kind, _, place, tick)| {
            !seen.contains(id)
                && infantry(*kind)
                && observation.tick.saturating_sub(*tick) <= ESCORT_MEMORY_TICKS
                && dist2(place.0, place.1, at.0, at.1) <= reach
        })
        .count();
    seen.len() + remembered
}

/// Runs the pack over V1.1's finished decision (see the module docs).
pub(in crate::ai_core::decision) fn finish(
    decision: &mut crate::ai_core::decision::AiDecision,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
) {
    if !active(decision.profile_id) {
        return;
    }
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    let fausts: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Panzerfaust && unit.is_complete && unit.hp > 0)
        .collect();
    let ids: BTreeSet<u32> = fausts.iter().map(|unit| unit.id).collect();
    let spent: BTreeSet<u32> = observation
        .spent_panzerfausts
        .iter()
        .copied()
        .filter(|id| ids.contains(id))
        .collect();
    let mut pack = std::mem::take(&mut memory.jeff_v1_1.faust_pack);
    for id in &ids {
        if pack.known.insert(*id) {
            pack.trained += 1;
        }
    }
    pack.known.retain(|id| ids.contains(id));
    pack.alive = ids.len();
    pack.spent = spent.len();
    // `pfcap`: a Panzerfaust loaded at the last decision and spent now fired (one that fired and
    // died in between goes uncounted).
    pack.fired += pack
        .loaded_last
        .iter()
        .filter(|id| spent.contains(id))
        .count();
    pack.loaded_last = ids.difference(&spent).copied().collect();
    let rifles: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman && unit.is_complete && unit.hp > 0)
        .map(|unit| (unit.id, unit))
        .collect();
    pack.screen
        .retain(|id| screen() && !guard() && rifles.contains_key(id));
    pack.wave
        .retain(|id| wave() && !guard() && rifles.contains_key(id));
    pack.screen_to
        .retain(|id, _| pack.screen.contains(id) || pack.wave.contains(id));
    pack.sent
        .retain(|id, _| ids.contains(id) || pack.screen.contains(id) || pack.wave.contains(id));
    pack.slots
        .retain(|id, _| ids.contains(id) && !spent.contains(id));
    pack.charging
        .retain(|id, _| ids.contains(id) && !spent.contains(id));
    if fausts.is_empty() {
        pack.screen.clear();
        pack.wave.clear();
        pack.screen_to.clear();
        pack.screen_go = None;
        memory.jeff_v1_1.faust_pack = pack;
        return;
    }
    let loaded: Vec<&AiEntitySummary> = fausts
        .iter()
        .copied()
        .filter(|unit| !spent.contains(&unit.id))
        .collect();
    if ready_mode() {
        if loaded.len() >= READY_FAUSTS {
            pack.ready = true;
        } else if loaded.len() < NEED_CLOSE && pack.charging.is_empty() {
            pack.ready = false;
        }
    }
    // `pfpush`: where the push out stands, and which way it faces.
    let pushing = if push_mode() {
        push_rest(observation, memory)
    } else {
        None
    };
    let waiting_home = ready_mode()
        && !pack.ready
        && pack.charging.is_empty()
        && !(pushing.is_some() && loaded.len() >= NEED_CLOSE);
    let rest = if waiting_home {
        let start = tile_center(observation.own_start_tile, observation.map.tile_size);
        let forward = enemy_start(observation)
            .and_then(|enemy| unit_vector(start, enemy))
            .unwrap_or((1.0, 0.0));
        let ahead = (HOME_TILES + WAIT_AHEAD_TILES + BACK_TILES) * ts;
        Some((
            clamp_to_map(
                (start.0 + forward.0 * ahead, start.1 + forward.1 * ahead),
                observation.map,
            ),
            forward,
        ))
    } else if pushing.is_some() {
        pushing
    } else {
        rest_place(observation, &mut pack.last)
    };
    // `pfscreen`: between charges, the pack keeps `SCREEN_RIFLES` Riflemen while it has a charge's
    // worth of loaded Panzerfausts.
    if screen() && !guard() && pack.charging.is_empty() {
        if loaded.len() < NEED_CLOSE || waiting_home {
            pack.screen.clear();
            pack.screen_to.clear();
        } else if let Some((center, _)) = rest {
            let mut free: Vec<&AiEntitySummary> = rifles
                .values()
                .copied()
                .filter(|unit| !pack.screen.contains(&unit.id))
                .filter(|unit| {
                    dist2(unit.x, unit.y, center.0, center.1) <= squared(SCREEN_RECRUIT_TILES * ts)
                })
                .collect();
            free.sort_by(|a, b| {
                dist2(a.x, a.y, center.0, center.1)
                    .total_cmp(&dist2(b.x, b.y, center.0, center.1))
                    .then(a.id.cmp(&b.id))
            });
            let want = SCREEN_RIFLES.saturating_sub(pack.screen.len());
            for unit in free.into_iter().take(want) {
                pack.screen.insert(unit.id);
            }
        }
    }
    // The pack owns its Panzerfausts (and screen): every other system's order for them goes.
    let mut held = ids.clone();
    held.extend(pack.screen.iter().copied());
    decision
        .commands
        .retain_mut(|command| super::matchup::strip(command, &held));
    let start = tile_center(observation.own_start_tile, observation.map.tile_size);
    let to_enemy = enemy_start(observation)
        .and_then(|enemy| unit_vector(start, enemy))
        .unwrap_or((1.0, 0.0));
    let home = (
        start.0 + to_enemy.0 * HOME_TILES * ts,
        start.1 + to_enemy.1 * HOME_TILES * ts,
    );
    let mut commands: Vec<Command> = Vec::new();
    if guard() {
        guard_orders(
            decision,
            observation,
            memory,
            &mut pack,
            (&fausts, &loaded, &spent),
        );
        memory.jeff_v1_1.faust_pack = pack;
        return;
    }

    // Spent Panzerfausts walk home, each to its own place there.
    let spent_units: Vec<&AiEntitySummary> = fausts
        .iter()
        .copied()
        .filter(|unit| spent.contains(&unit.id))
        .collect();
    let visible_tanks: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::Tank && enemy.hp > 0)
        .collect();
    for (index, unit) in spent_units.iter().enumerate() {
        let at = slot_point(observation, (home, to_enemy), index);
        safe_send(
            &mut commands,
            &mut pack,
            observation,
            unit,
            at,
            &visible_tanks,
            false,
        );
    }

    // The charge.
    let within = |unit: &AiEntitySummary, tank: &AiEntitySummary, tiles: f32| {
        dist2(unit.x, unit.y, tank.x, tank.y) <= squared(tiles * ts)
    };
    // A charging Panzerfaust keeps its Tank while it is in sight within `LEASH_TILES` of it.
    pack.charging.retain(|id, target| {
        fausts
            .iter()
            .find(|unit| unit.id == *id)
            .is_some_and(|unit| {
                visible_tanks
                    .iter()
                    .any(|tank| tank.id == *target && within(unit, tank, LEASH_TILES))
            })
    });
    // The Tanks covering `tank` with their machine guns, itself included.
    let covering = |tank: &AiEntitySummary| {
        visible_tanks
            .iter()
            .filter(|other| within(other, tank, COVER_TILES))
            .count()
    };
    let mut recruits: Vec<&AiEntitySummary> = Vec::new();
    // The Tanks the charge goes for: the charged ones and those covering them.
    let mut zone: Vec<&AiEntitySummary> = Vec::new();
    let mut note = String::new();
    if pack.charging.is_empty() {
        // A new charge at the Tank in contact nearest the loaded Panzerfausts.
        let nearest = |tank: &AiEntitySummary| {
            loaded
                .iter()
                .map(|unit| dist2(unit.x, unit.y, tank.x, tank.y))
                .fold(f32::INFINITY, f32::min)
        };
        let primary = visible_tanks
            .iter()
            .copied()
            .filter(|tank| {
                rest.is_none_or(|(center, _)| {
                    dist2(tank.x, tank.y, center.0, center.1) <= squared(LEASH_TILES * ts)
                })
            })
            .filter(|tank| nearest(tank) <= squared(CONTACT_TILES * ts))
            .filter(|_| !waiting_home)
            .min_by(|a, b| nearest(a).total_cmp(&nearest(b)).then(a.id.cmp(&b.id)));
        if let Some(primary) = primary {
            let chargers: Vec<&AiEntitySummary> = loaded
                .iter()
                .copied()
                .filter(|unit| within(unit, primary, REACH_TILES))
                .collect();
            let close = nearest(primary) <= squared(CLOSE_TILES * ts);
            let supported = observation.owned.iter().any(|unit| {
                unit.kind == EntityKind::Tank
                    && unit.is_complete
                    && unit.hp > 0
                    && within(unit, primary, SUPPORT_TILES)
            });
            let cover = covering(primary);
            let escort = escort_near(observation, memory, (primary.x, primary.y));
            let need = if close { NEED_CLOSE } else { NEED_FAR }
                .max(ROCKETS_PER_TANK * cover)
                .saturating_sub(if supported && !ready_mode() {
                    SUPPORT_DISCOUNT
                } else {
                    0
                })
                + if escort >= MAX_ESCORT {
                    ESCORT_EXTRA
                } else {
                    0
                };
            note = format!(
                " contact=t{}:{}/{}{}:c{}",
                primary.id,
                chargers.len(),
                need,
                if escort >= MAX_ESCORT {
                    ":escorted"
                } else {
                    ""
                },
                cover
            );
            // `pfhold`: the pack never charges; it only fires at Tanks that come into reach.
            if chargers.len() >= need && off("pfhold") {
                recruits = chargers;
                pack.charge_start = tick;
                pack.need = need;
                pack.launched = false;
                zone = visible_tanks
                    .iter()
                    .copied()
                    .filter(|tank| within(tank, primary, COVER_TILES))
                    .collect();
            }
        }
    } else {
        // A charge under way: every loaded Panzerfaust within reach of a Tank of it joins it.
        zone = visible_tanks
            .iter()
            .copied()
            .filter(|tank| {
                pack.charging.values().any(|target| {
                    visible_tanks
                        .iter()
                        .any(|charged| charged.id == *target && within(tank, charged, COVER_TILES))
                })
            })
            .collect();
        recruits = loaded
            .iter()
            .copied()
            .filter(|unit| !pack.charging.contains_key(&unit.id))
            .filter(|unit| zone.iter().any(|tank| within(unit, tank, REACH_TILES)))
            .collect();
    }
    // Each recruit goes for the nearest Tank still short of the rockets that kill it, else the
    // nearest one.
    let mut booked: BTreeMap<u32, usize> = BTreeMap::new();
    for target in pack.charging.values() {
        *booked.entry(*target).or_default() += 1;
    }
    for unit in recruits {
        let mut reach: Vec<&AiEntitySummary> = zone
            .iter()
            .copied()
            .filter(|tank| within(unit, tank, REACH_TILES))
            .collect();
        reach.sort_by(|a, b| {
            dist2(unit.x, unit.y, a.x, a.y)
                .total_cmp(&dist2(unit.x, unit.y, b.x, b.y))
                .then(a.id.cmp(&b.id))
        });
        let short = |tank: &&AiEntitySummary| {
            booked.get(&tank.id).copied().unwrap_or(0)
                < tank.hp.div_ceil(ROCKET_DAMAGE) as usize + 1
        };
        let Some(target) = reach.iter().find(|tank| short(tank)).or(reach.first()) else {
            continue;
        };
        pack.charging.insert(unit.id, target.id);
        *booked.entry(target.id).or_default() += 1;
    }
    // Until the charge is launched, its Panzerfausts gather `STAGE_TILES` from their Tanks, out of
    // the machine guns' reach, and go in together: coming in one at a time from up to 18 tiles,
    // they were shot down before they fired.
    let charging: Vec<(u32, u32)> = pack
        .charging
        .iter()
        .map(|(id, target)| (*id, *target))
        .collect();
    let tank_of = |id: u32| visible_tanks.iter().copied().find(|tank| tank.id == id);
    if charging.is_empty() {
        pack.launched = false;
    } else if !pack.launched && !off("pfgather") {
        let (mut ready, mut threatened) = (0, false);
        for (id, target) in &charging {
            let (Some(unit), Some(tank)) =
                (fausts.iter().find(|unit| unit.id == *id), tank_of(*target))
            else {
                continue;
            };
            ready += usize::from(within(unit, tank, STAGE_TILES + STAGE_SLACK_TILES));
            threatened |= within(unit, tank, LAUNCH_TILES);
        }
        pack.launched = threatened
            || tick.saturating_sub(pack.charge_start) >= GATHER_TICKS
            || ready >= pack.need.min(charging.len());
    } else {
        pack.launched = true;
    }
    // `pfwave`: once the charge goes in, the Riflemen near its Tanks are sent in first (topped up
    // until the Panzerfausts are let go).
    if charging.is_empty() {
        pack.wave.clear();
        pack.wave_open = false;
    } else if wave() && pack.launched && !pack.wave_open {
        let charged: Vec<&AiEntitySummary> = charging
            .iter()
            .filter_map(|(_, target)| tank_of(*target))
            .collect();
        let tanks_in = zone.len().max(charged.len()).max(1);
        let want = (WAVE_PER_TANK * tanks_in + 1)
            .clamp(WAVE_MIN, WAVE_MAX)
            .saturating_sub(pack.wave.len() + pack.screen.len());
        let nearest = |unit: &AiEntitySummary| {
            charged
                .iter()
                .map(|tank| dist2(unit.x, unit.y, tank.x, tank.y))
                .fold(f32::INFINITY, f32::min)
        };
        let mut free: Vec<&AiEntitySummary> = rifles
            .values()
            .copied()
            .filter(|unit| !pack.wave.contains(&unit.id) && !pack.screen.contains(&unit.id))
            .filter(|unit| nearest(unit) <= squared(WAVE_RECRUIT_TILES * ts))
            .collect();
        free.sort_by(|a, b| nearest(a).total_cmp(&nearest(b)).then(a.id.cmp(&b.id)));
        let joined: BTreeSet<u32> = free.into_iter().take(want).map(|unit| unit.id).collect();
        if !joined.is_empty() {
            decision
                .commands
                .retain_mut(|command| super::matchup::strip(command, &joined));
            pack.wave.extend(joined);
        }
    }
    // Every Rifleman going in ahead of the Panzerfausts: the screen and the wave.
    let runners: Vec<u32> = pack
        .screen
        .iter()
        .chain(pack.wave.iter())
        .copied()
        .collect();
    // `pfscreen`: once the charge goes in, the screen runs at its Tanks first and the Panzerfausts
    // follow `SCREEN_LEAD_TICKS` later.
    let mut hold_fausts = false;
    if charging.is_empty() || !pack.launched {
        pack.screen_go = None;
        pack.screen_to.clear();
    } else if !runners.is_empty() {
        let go = *pack.screen_go.get_or_insert(tick);
        let (mut sx, mut sy, mut sn) = (0.0_f32, 0.0_f32, 0.0_f32);
        for (id, _) in &charging {
            if let Some(unit) = fausts.iter().find(|unit| unit.id == *id) {
                sx += unit.x;
                sy += unit.y;
                sn += 1.0;
            }
        }
        let centroid = (sx / sn.max(1.0), sy / sn.max(1.0));
        let n = runners.len() as f32;
        let mut leading = 0;
        for (index, rid) in runners.iter().enumerate() {
            let Some(rifle) = rifles.get(rid).copied() else {
                continue;
            };
            let Some(tank) = charging
                .iter()
                .filter_map(|(_, target)| tank_of(*target))
                .filter(|tank| within(rifle, tank, REACH_TILES))
                .min_by(|a, b| {
                    dist2(rifle.x, rifle.y, a.x, a.y)
                        .total_cmp(&dist2(rifle.x, rifle.y, b.x, b.y))
                        .then(a.id.cmp(&b.id))
                })
            else {
                continue;
            };
            leading += 1;
            let resend = pack.screen_to.get(rid).is_none_or(|(sent_tank, at)| {
                *sent_tank != tank.id
                    || dist2(at.0 as f32, at.1 as f32, tank.x, tank.y)
                        > squared(SCREEN_RESEND_TILES * ts)
            });
            if !resend {
                continue;
            }
            let dir = unit_vector((tank.x, tank.y), centroid)
                .or_else(|| unit_vector((tank.x, tank.y), (rifle.x, rifle.y)))
                .unwrap_or((0.0, 1.0));
            let side = (-dir.1, dir.0);
            let off = (index as f32 - (n - 1.0) / 2.0) * SCREEN_SPACING_TILES;
            let at = clamp_to_map(
                (
                    tank.x + (dir.0 * SCREEN_SHORT_TILES + side.0 * off) * ts,
                    tank.y + (dir.1 * SCREEN_SHORT_TILES + side.1 * off) * ts,
                ),
                observation.map,
            );
            commands.push(Command::Move {
                units: vec![*rid],
                x: at.0,
                y: at.1,
                queued: false,
            });
            commands.push(Command::AttackMove {
                units: vec![*rid],
                x: tank.x,
                y: tank.y,
                queued: true,
            });
            pack.screen_to
                .insert(*rid, (tank.id, (tank.x as i32, tank.y as i32)));
            pack.sent.remove(rid);
        }
        let wait = if wave() {
            WAVE_WAIT_TICKS
        } else {
            SCREEN_WAIT_TICKS
        };
        hold_fausts = leading > 0 && tick.saturating_sub(go) < wait;
    }
    // Tiles from the screen's nearest Rifleman to `tank`.
    let screen_at: Vec<(f32, f32)> = runners
        .iter()
        .filter_map(|rid| rifles.get(rid))
        .map(|rifle| (rifle.x, rifle.y))
        .collect();
    // `pfwave`: a Tank whose cannon is on one of Jeff's units other than a Panzerfaust, or with a
    // Rifleman of the wave at it, is at grips; from then on the Panzerfausts go.
    let gripped = |tank: &AiEntitySummary| {
        tank.target_id.is_some_and(|target| {
            observation
                .owned
                .iter()
                .any(|unit| unit.id == target && unit.kind != EntityKind::Panzerfaust)
        }) || screen_at
            .iter()
            .any(|at| dist2(at.0, at.1, tank.x, tank.y) <= squared(WAVE_AT_TILES * ts))
    };
    if wave() && hold_fausts && !pack.wave_open {
        pack.wave_open = charging
            .iter()
            .filter_map(|(_, target)| tank_of(*target))
            .any(gripped);
    }
    let wave_hold = wave() && hold_fausts && !pack.wave_open;
    let screen_tiles = |tank: &AiEntitySummary| {
        screen_at
            .iter()
            .map(|at| dist2(at.0, at.1, tank.x, tank.y))
            .fold(f32::INFINITY, f32::min)
            .det_sqrt()
            / ts
    };
    let mut waiting = 0;
    for (id, target) in &charging {
        let Some(unit) = fausts.iter().find(|unit| unit.id == *id) else {
            continue;
        };
        if pack.launched {
            if wave_hold && unit.target_id != Some(*target) {
                // `pfwave`: out of the machine guns' reach, wait for the wave to reach the Tank.
                if let Some(tank) = tank_of(*target) {
                    let own = dist2(unit.x, unit.y, tank.x, tank.y).det_sqrt() / ts;
                    if own > SCREEN_UNDER_FIRE_TILES {
                        waiting += 1;
                        continue;
                    }
                }
            } else if hold_fausts && !wave() && unit.target_id != Some(*target) {
                if let Some(tank) = tank_of(*target) {
                    let own = dist2(unit.x, unit.y, tank.x, tank.y).det_sqrt() / ts;
                    let screen = screen_tiles(tank);
                    if screen > SCREEN_SHORT_TILES + 1.0
                        && own > SCREEN_UNDER_FIRE_TILES
                        && own < screen + SCREEN_LEAD_TILES
                    {
                        waiting += 1;
                        continue;
                    }
                }
            }
            if unit.target_id != Some(*target) {
                commands.push(Command::Attack {
                    units: vec![*id],
                    target: *target,
                    queued: false,
                });
            }
            pack.sent.remove(id);
            continue;
        }
        let Some(tank) = tank_of(*target) else {
            continue;
        };
        if within(unit, tank, STAGE_TILES) {
            // At the gathering ring: wait there.
            if unit.state != AiEntityState::Idle {
                commands.push(Command::HoldPosition {
                    units: vec![*id],
                    queued: false,
                });
            }
            pack.sent
                .insert(*id, (tick, (unit.x as i32, unit.y as i32)));
            continue;
        }
        let away = unit_vector((tank.x, tank.y), (unit.x, unit.y)).unwrap_or((0.0, 1.0));
        let at = clamp_to_map(
            (
                tank.x + away.0 * STAGE_TILES * ts,
                tank.y + away.1 * STAGE_TILES * ts,
            ),
            observation.map,
        );
        send(&mut commands, &mut pack, unit, at, tick, ts);
    }

    // The loaded Panzerfausts not charging rest on their slots.
    if let Some(rest) = rest {
        let mut taken: BTreeSet<usize> = pack.slots.values().copied().collect();
        for unit in &loaded {
            if pack.slots.contains_key(&unit.id) {
                continue;
            }
            let free = (0..).find(|index| !taken.contains(index)).unwrap_or(0);
            taken.insert(free);
            pack.slots.insert(unit.id, free);
        }
        for unit in &loaded {
            if pack.charging.contains_key(&unit.id) {
                continue;
            }
            let fighting = unit.target_id.is_some_and(|target| {
                observation.visible_enemies.iter().any(|enemy| {
                    enemy.id == target
                        && enemy.hp > 0
                        && dist2(unit.x, unit.y, enemy.x, enemy.y) <= squared(FIGHTING_TILES * ts)
                })
            });
            if fighting {
                continue;
            }
            let Some(slot) = pack.slots.get(&unit.id).copied() else {
                continue;
            };
            let at = slot_point(observation, rest, slot);
            safe_send(
                &mut commands,
                &mut pack,
                observation,
                unit,
                at,
                &visible_tanks,
                true,
            );
        }
        // `pfscreen`: between charges the screen stands `SCREEN_AHEAD_TILES` in front of the
        // pack's first row.
        if pack.screen_go.is_none() {
            let (center, forward) = rest;
            let side = (-forward.1, forward.0);
            let screen_ids: Vec<u32> = pack.screen.iter().copied().collect();
            let n = screen_ids.len() as f32;
            for (index, rid) in screen_ids.iter().enumerate() {
                let Some(unit) = rifles.get(rid).copied() else {
                    continue;
                };
                let fighting = unit.target_id.is_some_and(|target| {
                    observation.visible_enemies.iter().any(|enemy| {
                        enemy.id == target
                            && enemy.hp > 0
                            && dist2(unit.x, unit.y, enemy.x, enemy.y)
                                <= squared(FIGHTING_TILES * ts)
                    })
                });
                if fighting {
                    continue;
                }
                let back = (BACK_TILES - SCREEN_AHEAD_TILES) * ts;
                let across = (index as f32 - (n - 1.0) / 2.0) * SCREEN_SPACING_TILES * ts;
                let at = clamp_to_map(
                    (
                        center.0 - forward.0 * back + side.0 * across,
                        center.1 - forward.1 * back + side.1 * across,
                    ),
                    observation.map,
                );
                send(&mut commands, &mut pack, unit, at, tick, ts);
            }
        }
    }
    decision.trace.notes.push(format!(
        "pfpack loaded={} spent={} rest={} charging={}{} trained={} fired={} afford={} screen={}{}{}{}",
        loaded.len(),
        spent.len(),
        rest.map_or_else(
            || "-".to_string(),
            |(at, _)| format!("{:.0},{:.0}", at.0 / ts, at.1 / ts)
        ),
        pack.charging.len(),
        note,
        pack.trained,
        pack.fired,
        if ledger() { affordable(&pack).to_string() } else { "-".to_string() },
        pack.screen.len(),
        if waiting > 0 { format!(":wait{waiting}") } else if pack.screen_go.is_some() { ":go".to_string() } else { String::new() },
        if !ready_mode() { "" } else if waiting_home { " home" } else { " ready" },
        if !wave() && !push_mode() { String::new() } else { format!(" wave={}{}{}", pack.wave.len(), if pack.wave_open { ":open" } else { "" }, if pushing.is_some() { " push" } else { "" }) }
    ));
    decision.commands.extend(commands);
    memory.jeff_v1_1.faust_pack = pack;
}

/// Sends `unit` to `at` to hold there, unless it stands there already or is on its way.
fn send(
    commands: &mut Vec<Command>,
    pack: &mut FaustPackMemory,
    unit: &AiEntitySummary,
    at: (f32, f32),
    tick: u32,
    ts: f32,
) {
    if dist2(unit.x, unit.y, at.0, at.1) <= squared(ON_SLOT_TILES * ts) {
        return;
    }
    let place = (at.0 as i32, at.1 as i32);
    // A slot that follows a moving Tank group drifts: a Panzerfaust is sent again only once its
    // slot has moved `RESEND_TILES`.
    if let Some((when, sent_to)) = pack.sent.get(&unit.id) {
        let near = (RESEND_TILES * ts) as i32;
        let same = (sent_to.0 - place.0).abs() <= near && (sent_to.1 - place.1).abs() <= near;
        if same && tick.saturating_sub(*when) < REORDER_TICKS && unit.state != AiEntityState::Idle {
            return;
        }
    }
    commands.push(Command::Move {
        units: vec![unit.id],
        x: at.0,
        y: at.1,
        queued: false,
    });
    commands.push(Command::HoldPosition {
        units: vec![unit.id],
        queued: true,
    });
    pack.sent.insert(unit.id, (tick, place));
}

/// Squared distance from `point` to the segment `from`-`to`.
fn segment_dist2(from: (f32, f32), to: (f32, f32), point: (f32, f32)) -> f32 {
    let (vx, vy) = (to.0 - from.0, to.1 - from.1);
    let len2 = vx * vx + vy * vy;
    let t = if len2 <= f32::EPSILON {
        0.0
    } else {
        (((point.0 - from.0) * vx + (point.1 - from.1) * vy) / len2).clamp(0.0, 1.0)
    };
    dist2(point.0, point.1, from.0 + vx * t, from.1 + vy * t)
}
