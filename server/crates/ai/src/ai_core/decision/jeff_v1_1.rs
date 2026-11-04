//! Jeff's AI V1.1: what it does differently from V1.0. Every function here acts only for V1.1
//! (`is_jeff_v1_1`); for V1.0 each returns without touching the decision.

use super::*;
use crate::ai_core::decision::geometry::{dist2, squared};
use crate::ai_core::facts::{OpeningOrder, ProductionBuildingFact};

pub(super) mod base_line;
pub(super) mod charge;
pub(super) mod deny;
pub(super) mod front_line;
pub(super) mod home_guard;
pub(super) mod meth_strike;
pub(super) mod hunt;
pub(super) mod natclear;
pub(super) mod natural;
pub(super) mod push;
pub(super) mod reveal;
pub(super) mod rovers;
pub(super) mod scout;
pub(super) mod smoke_cover;
pub(super) mod screen;
pub(super) mod strike;
pub(super) mod tank_pause;
pub(super) mod tank_raid;
pub(super) mod third_base;

/// What V1.1 remembers between decisions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct V1_1Memory {
    /// Entrenchment waits for Methamphetamines this decision: Meth's Oil comes in before
    /// Entrenchment's research would end, so the Training Centre stays free for Meth.
    pub(super) entrenchment_waits: bool,
    /// Each base-line unit's post (`base_line`).
    pub(super) base_lines: base_line::LineMemory,
    /// Since when Steel has stood at `third_base::BANK_STEEL` or more.
    pub(super) banked_since: Option<u32>,
    /// Since when Steel has stood at `third_base::BANK_STEEL` or more, Tank or not.
    pub(super) base_bank_since: Option<u32>,
    pub(super) push: push::PushMemory,
    pub(super) strike: strike::StrikeMemory,
    pub(super) scout: scout::ScoutMemory,
    pub(super) hunt: hunt::HuntMemory,
    pub(super) natural: natural::NaturalMemory,
    pub(super) natclear: natclear::NatClearMemory,
    pub(super) charge: charge::ChargeMemory,
    pub(super) reveal: reveal::RevealMemory,
    pub(super) tank_raid: tank_raid::TankRaidMemory,
    pub(super) front_line: front_line::FrontLineMemory,
    pub(super) home_guard: home_guard::HomeGuardMemory,
    pub(super) rovers: rovers::RoverMemory,
    pub(super) smoke: smoke_cover::SmokeMemory,
    pub(super) deny: deny::DenyMemory,
    pub(super) meth_strike: meth_strike::MethStrikeMemory,
    pub(super) tank_pause: tank_pause::TankPauseMemory,
    /// Enemy fighters seen dying and Jeff's fighters lost so far (`AiDecisionMemory::note_losses`).
    pub(super) kills: u32,
    pub(super) deaths: u32,
    /// The unit value killed and lost so far, Oil counted twice (`tradeexpand`).
    pub(super) value_killed: u32,
    pub(super) value_lost: u32,
    /// The opening order once decided (`ecafter`), and whether the first Scout Car was ordered.
    pub(super) opening: Option<OpeningOrder>,
    pub(super) car_ordered: bool,
}

/// Methamphetamines comes first. It is ordered the moment the Training Centre is free and its cost
/// is in the bank, ahead of the Engineering Complex and every other Oil purchase, and until then this
/// decision's budget holds that cost back. Meth Riflemen fire twice as fast, move a quarter faster and
/// shoot on the move; Rush AI researches it at 0:30 and Pressure AI at 1:05, where V1.0 got it at
/// 4:14 against Pressure and in 27 of 40 games against Rush.
///
/// Entrenchment costs no Oil, so it takes the Training Centre first only while its research ends
/// before Meth's Oil is in: it then costs Meth nothing. Otherwise it follows right after Meth.
pub(super) fn early_research(
    (actions, facts, memory, intents): (
        &mut AiActionContext<'_>,
        &AiFacts,
        &mut AiDecisionMemory,
        &mut Vec<AiIntent>,
    ),
    profile_id: &str,
    plan: Option<&spend_plan::SpendPlan>,
) {
    memory.jeff_v1_1.entrenchment_waits = false;
    if !is_jeff_v1_1(profile_id)
        || off("meth")
        || facts.complete_building_count(EntityKind::TrainingCentre) == 0
    {
        return;
    }
    let meth = UpgradeKind::Methamphetamines;
    let entrenchment = UpgradeKind::Entrenchment;
    if !ordered(facts, memory, meth) {
        if research(actions, facts, memory, intents, meth) {
            return;
        }
        let definition = upgrade::definition(meth);
        if !ordered(facts, memory, entrenchment) {
            let oil_short = definition.cost_oil.saturating_sub(actions.budget().oil());
            let research_ticks = upgrade::definition(entrenchment).research_ticks;
            let oil_in_time = plan.map_or(0.0, |plan| oil_within(plan, research_ticks));
            if oil_in_time < oil_short as f32 {
                research(actions, facts, memory, intents, entrenchment);
            } else {
                memory.jeff_v1_1.entrenchment_waits = true;
            }
        }
        actions.holdback_resources(definition.cost_steel, definition.cost_oil);
        return;
    }
    if !ordered(facts, memory, entrenchment) {
        research(actions, facts, memory, intents, entrenchment);
    }
}

/// Tanks in the field before Tank+ comes ahead of the next one.
const TANK_PLUS_TANKS: usize = 4;

/// Tank+ (Engineering Complex, 150 Steel and 150 Oil, 20 s: Tanks turn half as fast again, move a
/// tenth faster and see 2 tiles farther) comes before a fifth Tank. With `TANK_PLUS_TANKS` Tanks
/// out it is ordered the moment the Engineering Complex is free and its cost is in the bank; until
/// then this decision's budget holds its Oil back and no Tank is trained. Returns whether Tanks
/// wait for it.
pub(super) fn tank_plus_first(
    (actions, facts, memory, intents): (
        &mut AiActionContext<'_>,
        &AiFacts,
        &mut AiDecisionMemory,
        &mut Vec<AiIntent>,
    ),
    observation: &AiObservation,
    profile_id: &str,
) -> bool {
    let tank_plus = UpgradeKind::TankPlus;
    if !is_jeff_v1_1(profile_id)
        || off("tankplus")
        || ordered(facts, memory, tank_plus)
        || facts.complete_building_count(EntityKind::EngineeringComplex) == 0
    {
        return false;
    }
    let tanks = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
        .count();
    if tanks < TANK_PLUS_TANKS {
        return false;
    }
    if research(actions, facts, memory, intents, tank_plus) {
        return false;
    }
    actions.holdback_resources(0, upgrade::definition(tank_plus).cost_oil);
    true
}

/// Factories V1.1 builds toward on a floating bank.
const FLOAT_FACTORIES: usize = 4;

/// Another Factory while every finished one is training, none is going up, and the bank holds a
/// Factory and a Tank besides. V1.0 keeps to two, the second after three Tanks and three Depots:
/// taking bases early against Pressure AI, V1.1 ran four Depots on one Factory from 4:00 to 5:30
/// with 800-1,300 Steel and 300-500 Oil in the bank, and 2-3 Tanks to hold them. One Factory turns
/// at most 1,020 Steel and 420 Oil a minute into Tanks; Pressure AI builds four Vehicle Works.
pub(super) fn factory_for_float(
    profile_id: &str,
    observation: &AiObservation,
    facts: &AiFacts,
) -> bool {
    let factories = facts.building_count(EntityKind::Factory);
    if !is_jeff_v1_1(profile_id)
        || off("factories")
        || factories == 0
        || factories >= FLOAT_FACTORIES
        || facts.complete_building_count(EntityKind::Factory) < factories
    {
        return false;
    }
    let (factory_steel, factory_oil) = rts_rules::economy::cost(EntityKind::Factory);
    let (tank_steel, tank_oil) = rts_rules::economy::cost(EntityKind::Tank);
    facts
        .production_buildings(EntityKind::Factory)
        .iter()
        .all(|factory| factory.queue_len > 0)
        && observation.economy.steel >= factory_steel + tank_steel
        && observation.economy.oil >= factory_oil + tank_oil
}

/// The Steel and Oil V1.1's opening research still has to pay for, charged at once in the spend
/// plan so the natural, the Factory and the first Tank are timed after it.
pub(super) fn research_ahead(
    profile_id: &str,
    facts: &AiFacts,
    memory: &AiDecisionMemory,
) -> (u32, u32) {
    if !is_jeff_v1_1(profile_id) || off("meth") {
        return (0, 0);
    }
    [UpgradeKind::Methamphetamines, UpgradeKind::Entrenchment]
        .into_iter()
        .filter(|upgrade| !ordered(facts, memory, *upgrade))
        .map(upgrade::definition)
        .fold((0, 0), |(steel, oil), definition| {
            (steel + definition.cost_steel, oil + definition.cost_oil)
        })
}

/// The guns hold their trenches: no home-defense response moves V1.1's Machine Gunners. A set-up
/// gun has to pack up (1 s) and set up again (half that with Meth) around every move, and V1.0's
/// raid responses re-sent theirs every decision toward a shifting point: against Rush AI its guns
/// packed up under fire 138-248 times before 6:00 in the losses. They fire at what comes into reach
/// of their posts, and the posts cover the base's ways in. Before the first Tank the guns answer as
/// V1.0's do: kept on their posts, the guns out at the natural watched Pressure AI's five opening
/// Riflemen shoot the main's Depot down at 3:08 on Crossroads.
pub(super) fn keep_guns_on_posts(
    profile_id: &str,
    observation: &AiObservation,
    local_defenders: &mut Vec<u32>,
) {
    if !is_jeff_v1_1(profile_id) || off("gunposts") {
        return;
    }
    if !observation
        .owned
        .iter()
        .any(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
    {
        return;
    }
    local_defenders.retain(|id| {
        observation
            .owned
            .iter()
            .all(|unit| unit.id != *id || unit.kind != EntityKind::MachineGunner)
    });
}

/// A Machine Gunner with an enemy this close may answer it; the rest hold their posts (`mghome`).
const GUN_LEASH_TILES: f32 = 8.0;

/// Jeff's AI V1.1's Machine Gunners hold where they stand (`mghome`). A gun sets up for a second
/// before it fires and cannot fire on the move: against Pressure AI half the guns lost died
/// walking, sent by home defense to a fight or to the natural, and a gun parked at home fired 31
/// shots for each one lost, one walking out 11. Home defense takes a gun only with an enemy within
/// `GUN_LEASH_TILES` of it; the Riflemen, which fire on the move, run to the rest.
pub(super) fn guns_hold(
    profile_id: &str,
    observation: &AiObservation,
    local_defenders: &mut Vec<u32>,
) {
    if !is_jeff_v1_1(profile_id) || off("mghome") {
        return;
    }
    let reach = crate::ai_core::decision::geometry::squared(
        GUN_LEASH_TILES * observation.map.tile_size as f32,
    );
    local_defenders.retain(|id| {
        observation
            .owned
            .iter()
            .find(|unit| unit.id == *id)
            .is_none_or(|gun| {
                gun.kind != EntityKind::MachineGunner
                    || observation.visible_enemies.iter().any(|enemy| {
                        enemy.hp > 0
                            && enemy.kind.is_unit()
                            && crate::ai_core::decision::geometry::dist2(
                                enemy.x, enemy.y, gun.x, gun.y,
                            ) <= reach
                    })
            })
    });
}

/// The Barracks leave each Factory's next Tank's Steel in the bank, from the moment that Factory has
/// one Tank or none in hand, while the Oil for it is in: the Factories train first each decision
/// but, with Jeff queuing one Tank at a time, stood idle 37-66% of the time against Pressure AI,
/// nearly always with the Oil for a Tank and under its 425 Steel, as three Barracks spent the bank
/// while each Tank was built. Saving only once a Factory stood idle still left each next Tank
/// waiting 15-25 s for its Steel.
pub(super) fn reserve_tank_steel(
    profile_id: &str,
    building_kind: EntityKind,
    facts: &AiFacts,
    observation: &AiObservation,
    actions: &mut AiActionContext<'_>,
) {
    if !is_jeff_v1_1(profile_id) || building_kind != EntityKind::Barracks || off("tankreserve") {
        return;
    }
    if !facts.completed_upgrades().contains(&UpgradeKind::TankUnlock) {
        return;
    }
    let (steel, oil) = rts_rules::economy::cost(EntityKind::Tank);
    let waiting = facts
        .production_buildings(EntityKind::Factory)
        .iter()
        .filter(|factory| factory.queue_len <= 1 || off("nexttank"))
        .filter(|factory| factory.queue_len == 0 || !off("nexttank"))
        .count() as u32;
    let fundable = observation.economy.oil / oil.max(1);
    let tanks = waiting.min(fundable);
    if tanks > 0 {
        actions.holdback_resources(steel * tanks, 0);
    }
}

/// The early fights must reach this many kills, and this many for each death, for the natural to
/// come before the Factory (`ecafter`). At 3 the natural never came first in 16 games against
/// Pressure AI and Rush AI (the trade at the choice ran 0/4 to 6/4).
const NATURAL_FIRST_TRADE: u32 = 2;

/// Jeff's AI V1.1 (`ecafter`), Factory first: once the Factory's first Scout Car is ordered, the
/// Engineering Complex no longer waits for the natural's order, and until it is placed it goes
/// ahead of the natural's money (the natural's cost is not held back meanwhile, unless its order is
/// already out). Behind the natural's order it came at 2:54-3:09 against Pressure AI and Tank Unlock
/// at 3:25-3:52, the first Tank 20-40 s later than V1.0's; waiting for the bank to hold both left
/// it as late.
pub(super) fn complex_alongside_natural(facts: &AiFacts) -> bool {
    facts.v1_1_opening == Some(OpeningOrder::FactoryFirst)
        && facts.building_count(EntityKind::Factory) > 0
        && facts.v1_1_car_ordered
}

/// Unit value killed for each unit value lost (Oil counted twice) from which V1.1 takes its later
/// bases on fewer Tanks (`tradeexpand`), and the share of the Tanks those bases otherwise wait for
/// that then do, rounded down.
const EXPAND_TRADE: f32 = 1.5;
const EXPAND_TANK_SHARE: f32 = 0.7;

/// Whether V1.1 is ahead on the trade (`tradeexpand`): it has killed at least `EXPAND_TRADE` times
/// the unit value it has lost, Oil counted twice. Ahead, its army can spare the Tanks a later base
/// waits for sooner, and base 4 comes before any of its wells runs dry.
pub(super) fn trading_ahead(memory: &AiDecisionMemory) -> bool {
    let state = &memory.jeff_v1_1;
    memory.profile_id().is_some_and(is_jeff_v1_1)
        && !off("tradeexpand")
        && state.value_killed > 0
        && state.value_killed as f32 >= EXPAND_TRADE * state.value_lost as f32
}

/// The Tanks a later base waits for: `required`, or `EXPAND_TANK_SHARE` of it rounded down while V1.1
/// is ahead on the trade (`trading_ahead`).
pub(super) fn expansion_tanks(memory: &AiDecisionMemory, required: usize) -> usize {
    if trading_ahead(memory) {
        (required as f32 * EXPAND_TANK_SHARE).floor() as usize
    } else {
        required
    }
}

/// Jeff's AI V1.1's opening order (`ecafter`). The Engineering Complex comes after the Factory: it
/// only researches Tank Unlock, so ahead of the natural or the Factory it bought nothing yet (V1.0
/// placed it at 1:31 against Pressure AI, with the natural at 2:09 and the Factory at 2:29). Whether
/// the natural or the Factory comes first is decided once, when the opening guns are all ordered:
/// with the early fights at `NATURAL_FIRST_TRADE` kills or more and as many for each death, the
/// natural goes then; otherwise the Factory goes first, so its first Scout Car comes out sooner, and
/// the natural after that car. Until then neither goes down.
pub(super) fn opening_order(
    profile_id: &str,
    observation: &AiObservation,
    facts: &AiFacts,
    memory: &mut AiDecisionMemory,
) -> Option<OpeningOrder> {
    if !is_jeff_v1_1(profile_id) || (off("ecafter") && !rifle_opening(profile_id)) {
        memory.jeff_v1_1.opening = None;
        memory.jeff_v1_1.car_ordered = false;
        return None;
    }
    // Kept once seen, so a first car lost early holds nothing up.
    memory.jeff_v1_1.car_ordered |= expansion_security::first_scout_car_ordered(observation, facts);
    // `riflenat`: always the natural first, held by Riflemen.
    if rifle_opening(profile_id) {
        memory.jeff_v1_1.opening = Some(OpeningOrder::NaturalFirst);
    }
    if let Some(order) = memory.jeff_v1_1.opening {
        return Some(order);
    }
    let decided = facts.building_count(EntityKind::Factory) > 0
        || facts.building_count(EntityKind::ResourceDepot) >= 2
        || expansion_security::guns_ordered(
            observation,
            facts,
            profile_id,
            expansion_security::OPENING_GUNS,
        );
    if !decided {
        return Some(OpeningOrder::Undecided);
    }
    let state = &mut memory.jeff_v1_1;
    let order = if state.kills >= NATURAL_FIRST_TRADE
        && state.kills >= state.deaths.saturating_mul(NATURAL_FIRST_TRADE)
    {
        OpeningOrder::NaturalFirst
    } else {
        OpeningOrder::FactoryFirst
    };
    state.opening = Some(order);
    Some(order)
}

/// The natural's guards V1.1 trains before its opening guns.
const NATURAL_GUARDS: usize = 2;
/// New Riflemen the opening trains, from the Training Centre's placing, before the guns.
pub(super) const OPENING_RIFLES: usize = 4;

/// `nomg`: no Machine Gunners; two Riflemen stand in for every gun V1.1 would train. A gun fires
/// only set up and stood idle on its post, where a Meth Rifleman fires on the move.
pub(super) const RIFLES_PER_GUN: usize = 2;
/// The Riflemen past which, under `nomg`, two count as one gun: the four starters and the
/// `OPENING_RIFLES` trained first.
pub(super) const NOMG_RIFLE_BASE: usize = 4 + OPENING_RIFLES;

/// Whether this profile trains Riflemen in place of guns (`nomg`).
pub(super) fn no_guns(profile_id: &str) -> bool {
    is_jeff_v1_1(profile_id) && !off("nomg")
}

/// `nomg`: the gun's place in the production priorities goes to Riflemen. Returns the most guns
/// the caps allowed (`max_counts`), each of which `rifle_cap_for_guns` turns into two Riflemen;
/// `None` without `nomg`.
pub(super) fn guns_to_riflemen(
    profile_id: &str,
    priorities: &mut Vec<EntityKind>,
    max_counts: &[(EntityKind, usize)],
) -> Option<usize> {
    if !no_guns(profile_id) {
        return None;
    }
    let Some(at) = priorities
        .iter()
        .position(|unit| *unit == EntityKind::MachineGunner)
    else {
        return Some(0);
    };
    let guns = max_counts
        .iter()
        .find_map(|(kind, count)| (*kind == EntityKind::MachineGunner).then_some(*count))
        .unwrap_or(0);
    priorities.remove(at);
    match priorities
        .iter()
        .position(|unit| *unit == EntityKind::Rifleman)
    {
        Some(rifle) if rifle < at => {}
        Some(rifle) => {
            priorities.remove(rifle);
            priorities.insert(at, EntityKind::Rifleman);
        }
        None => priorities.insert(at, EntityKind::Rifleman),
    }
    Some(guns)
}

/// `nomg`: the Barracks' Rifleman cap, raised to two Riflemen for each of `guns` above `base`, the
/// Riflemen V1.1 trains anyway (its Rifleman line, the natural's party, and at least
/// `NOMG_RIFLE_BASE`). A fixed target, not the count so far plus two, or it never stops; with no
/// Rifleman cap yet the target becomes one, since Riflemen now hold the gun's place in the
/// priorities.
pub(super) fn rifle_cap_for_guns(
    guns: usize,
    base: usize,
    max_counts: &mut Vec<(EntityKind, usize)>,
) {
    let target = base.max(NOMG_RIFLE_BASE) + RIFLES_PER_GUN * guns;
    match max_counts
        .iter_mut()
        .find(|(kind, _)| *kind == EntityKind::Rifleman)
    {
        Some((_, max)) => *max = (*max).max(target),
        None => max_counts.push((EntityKind::Rifleman, target)),
    }
}

/// V1.1's Barracks trains Riflemen first from the moment its Training Centre goes down until Meth is
/// ordered, and while the natural lacks its guards. Each opening gun takes 10 Oil, which Meth holds
/// back: against Pressure AI on Classic the Barracks stood idle from 0:30, when the Training Centre
/// was placed, to 1:15, with up to 390 Steel in the bank. And with the four starters
/// out on the opening rush, V1.0's natural guards were the Riflemen trained after four guns, out at
/// 2:15-2:30, so the natural waited for them with its 450 Steel set aside from 1:53. Returns the
/// Riflemen to train up to, past the spend plan's routine cap.
pub(super) fn riflemen_first(
    profile_id: &str,
    observation: &AiObservation,
    facts: &AiFacts,
    memory: &AiDecisionMemory,
    priorities: &mut Vec<EntityKind>,
) -> Option<usize> {
    if !is_jeff_v1_1(profile_id) || off("earlyrifles") {
        return None;
    }
    let riflemen = facts.unit_count(EntityKind::Rifleman);
    let fresh = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman && unit.hp > 0)
        .filter(|unit| !memory.opening_rush.is_starter(unit.id))
        .count();
    // `opening4`: until Meth is in, no Rifleman before the Training Centre is placed, then
    // Riflemen until `OPENING_RIFLES` new ones stand, then the guns.
    // The Rifleman opening (`riflenat`) places its second Barracks first: bought after the opening
    // Riflemen, it went down at 1:57.
    let second_barracks_waits =
        rifle_opening(profile_id) && facts.building_count(EntityKind::Barracks) < 2;
    // `tcrifles`: the Rifleman opening's Barracks train Riflemen nonstop from the Training Centre's
    // placing until Methamphetamines is in, first in the priorities. On Classic against Pressure AI
    // the first Barracks stood idle from 0:30, when the Training Centre went down, to 0:52, the
    // bank reaching 243 Steel: the second Barracks waited on the one Engineer building the Training
    // Centre, not on Steel. And past four new Riflemen one Barracks idled again, 1:13-1:20, with
    // 141-214 Steel in the bank. A Barracks trains one at a time, 3.5 Steel a second against about
    // 10 mined, so the second Barracks and Meth are still paid for on time.
    if rifle_opening(profile_id)
        && !off("tcrifles")
        && facts.building_count(EntityKind::TrainingCentre) > 0
        && !observation.upgrades.contains(&UpgradeKind::Methamphetamines)
    {
        priorities.retain(|unit| *unit != EntityKind::Rifleman);
        priorities.insert(0, EntityKind::Rifleman);
        return Some(riflemen + facts.building_count(EntityKind::Barracks).max(1));
    }
    if !off("opening4") && !observation.upgrades.contains(&UpgradeKind::Methamphetamines) {
        priorities.retain(|unit| *unit != EntityKind::Rifleman);
        if facts.building_count(EntityKind::TrainingCentre) == 0
            || fresh >= OPENING_RIFLES
            || second_barracks_waits
        {
            return Some(riflemen);
        }
        priorities.insert(0, EntityKind::Rifleman);
        return Some(riflemen + OPENING_RIFLES - fresh);
    }
    let guards_short = memory.expansion_security.site.is_some()
        && facts.building_count(EntityKind::ResourceDepot) < 2
        && fresh < NATURAL_GUARDS;
    let target = if guards_short {
        riflemen + NATURAL_GUARDS - fresh
    } else if facts.building_count(EntityKind::TrainingCentre) > 0
        && !off("meth")
        && !ordered(facts, memory, UpgradeKind::Methamphetamines)
    {
        riflemen + 1
    } else {
        return None;
    };
    priorities.retain(|unit| *unit != EntityKind::Rifleman);
    priorities.insert(0, EntityKind::Rifleman);
    Some(target)
}

/// V1.1 builds its first Factory while the natural waits to be secured, once the bank holds both:
/// V1.0 held the Factory back for the natural, and against Pressure AI on Classic its bank stood at
/// 800-1,000 Steel and 180-290 Oil from 2:15 to 2:45 with the Factory paid for. The natural's cost
/// stays held back from the budget, so only Steel and Oil past it go to the Factory.
pub(super) fn factory_alongside_natural(profile_id: &str, facts: &AiFacts) -> bool {
    is_jeff_v1_1(profile_id)
        && !off("factoryfirst")
        && !rifle_opening(profile_id)
        && facts.building_count(EntityKind::ResourceDepot) < 2
}

/// `riflenat`, the user's Rifleman opening (2026-11-03): Barracks, Training Centre, a second
/// Barracks, Riflemen from both nonstop; once Methamphetamines is in and `natural_rifles` Riflemen
/// stand at home, the natural, held by them; then Factory, first Scout Car, Engineering Complex,
/// Tank Unlock and the first Tank; then the third base, taken by `THIRD_BASE_RIFLES` Riflemen and a
/// Tank. No Rifleman ceiling: Riflemen are bought from the Steel the next expansion and Tank leave.
pub(super) fn rifle_opening(profile_id: &str) -> bool {
    is_jeff_v1_1(profile_id) && !off("riflenat")
}

/// The Riflemen standing at home before the natural goes, all of whom then hold it: 10, or
/// `natrifles` in `JEFF_V11_LINE` (testing, to fit each map within five either way). Of the
/// 120s at 0.5x, 0.75x, 1x, 1.5x and 2x the user's 14 (2026-11-04), 0.75x did best against
/// Pressure AI: 0-9-31 with 8 draws ahead, against 0-21-19 with 2 at 14; it took the natural
/// earlier (The River 2:30, was 3:31), and against Rush AI went 1-7-32 (was 0-4-36).
pub(super) fn natural_rifles() -> usize {
    crate::ai_core::profiles::jeff_v1_1_param("natrifles").map_or(NATURAL_RIFLES, |value| value as usize)
}
const NATURAL_RIFLES: usize = 10;

/// The Riflemen that take and hold the third base with a Tank (`riflenat`): 12, three quarters of
/// the user's 16 with the natural's (see `natural_rifles`), or `thirdrifles` in `JEFF_V11_LINE`
/// (testing).
pub(super) fn third_base_rifles() -> usize {
    crate::ai_core::profiles::jeff_v1_1_param("thirdrifles")
        .map_or(THIRD_BASE_RIFLES, |value| value as usize)
}
const THIRD_BASE_RIFLES: usize = 12;

/// The Rifleman opening's Factory no longer waits for the natural's order (`facwait`): from
/// `factorywait` seconds in `JEFF_V11_LINE`, 180 by default. Rush AI's guns parked 8-15 tiles off
/// Jeff's Classic natural kept it contested from 2:23 on; the order never went out, so no Factory
/// and no Tank came to clear them, and 1,250 Steel sat banked at 3:20.
pub(super) fn rifle_factory_free(profile_id: &str, tick: u32) -> bool {
    let wait = crate::ai_core::profiles::jeff_v1_1_param("factorywait").map_or(180.0, |s| s.max(0.0));
    rifle_opening(profile_id) && !off("facwait") && tick >= (wait * config::TICK_HZ as f32) as u32
}

/// Jeff's AI V1.1's hand-placed naturals refit to the 4x4 Resource Depot (`depot4`). The 3x3 sites
/// (Schone Tage (19,24) and (144,24), The River (108,93)) each now touch a resource node's body,
/// which Jeff's placement check rejects, so the natural was never ordered on Schone Tage (20 of 20
/// games) or from The River's lower-right start; these are the nearest full sites that place.
pub(super) fn refit_natural(profile_id: &str, observation: &AiObservation) -> Option<(u32, u32)> {
    if !is_jeff_v1_1(profile_id) || off("depot4") {
        return None;
    }
    match (observation.map.width, observation.map.height, observation.own_start_tile) {
        (166, 166, (8, 47)) => Some((21, 24)),
        (166, 166, (157, 47)) => Some((141, 24)),
        (126, 126, (9, 9)) => Some((15, 30)),
        (126, 126, (116, 116)) => Some((107, 92)),
        _ => None,
    }
}

/// `nearnat`: V1.1's natural is ranked below every other site for lying on the enemy's way in only
/// past this walk from the main, not the shared 30 tiles. On Classic the full base at the top-left
/// main's door, 31 tiles' walk east (score 92), fell under that rule by one tile, and the natural went
/// to the base around the water, 60 tiles' walk south (score 47). Doppelganger's diagonal site, 45
/// tiles out and raided every time it went down, stays under it.
pub(super) fn far_site_tiles(profile_id: &str, shared: f32) -> f32 {
    if is_jeff_v1_1(profile_id) && !off("nearnat") {
        NEAR_NATURAL_WALK_TILES
    } else {
        shared
    }
}
const NEAR_NATURAL_WALK_TILES: f32 = 40.0;

/// Riflemen this close to the main's start count as at home for the natural (`riflenat`): far
/// enough to take in those already holding the natural, not the starters out raiding.
const HOME_RIFLE_TILES: f32 = 40.0;

/// `riflenat`: the Riflemen at home, out or in training at a Barracks.
pub(super) fn home_riflemen(observation: &AiObservation) -> usize {
    let ts = observation.map.tile_size as f32;
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let out = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman && unit.is_complete && unit.hp > 0)
        .filter(|unit| dist2(unit.x, unit.y, home.0, home.1) <= squared(HOME_RIFLE_TILES * ts))
        .count();
    let queued: usize = observation
        .owned
        .iter()
        .filter(|unit| {
            unit.kind == EntityKind::Barracks && unit.production_kind == Some(EntityKind::Rifleman)
        })
        .map(|unit| unit.production_queue_len.unwrap_or(0))
        .sum();
    out + queued
}

/// `riflenat`: two Barracks from the Training Centre's placing; past them, one more at a time while
/// Steel stays banked (`third_base::banked`, 30 s at 600 or more with a Tank out) and every Barracks
/// is busy: Steel the Tanks, the two Barracks and the expansions cannot spend.
pub(super) fn rifle_barracks_target(
    profile_id: &str,
    observation: &AiObservation,
    facts: &AiFacts,
    memory: &AiDecisionMemory,
    target: usize,
) -> usize {
    if !rifle_opening(profile_id) || facts.building_count(EntityKind::TrainingCentre) == 0 {
        return target;
    }
    let barracks = facts.building_count(EntityKind::Barracks);
    let all_busy = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Barracks && unit.hp > 0)
        .all(|unit| unit.is_complete && unit.production_queue_len.unwrap_or(0) > 0);
    let more = third_base::banked(observation, memory) && all_busy;
    target.max(OPENING_BARRACKS).max(barracks + usize::from(more))
}
const OPENING_BARRACKS: usize = 2;

/// `riflenat`: the Barracks' Rifleman cap. Before the first Tank, the Riflemen the spend plan's
/// slack pays for (its steps keep the Barracks, the natural, the Factory, the Scout Car, the
/// Engineering Complex, Tank Unlock and the Tank on time); after it, the Riflemen the Steel past
/// a Tank's Steel pays for, the next base's Depot already held back by `later_bases`.
pub(super) fn rifle_opening_cap(
    plan: Option<&spend_plan::SpendPlan>,
    intents: &[AiIntent],
    counts: &[(EntityKind, usize)],
    steel: u32,
    tank_out: bool,
) -> usize {
    let current = counts
        .iter()
        .find_map(|(kind, count)| (*kind == EntityKind::Rifleman).then_some(*count))
        .unwrap_or(0);
    if !tank_out {
        if let Some(cap) = plan.and_then(|plan| plan.rifle_cap(intents, counts)) {
            return cap;
        }
    }
    // After the first Tank Riflemen leave a Tank's Steel in the bank.
    let reserve = rts_rules::economy::cost(EntityKind::Tank).0;
    let rifle = rts_rules::economy::cost(EntityKind::Rifleman).0.max(1);
    current + (steel.saturating_sub(reserve) / rifle) as usize
}

/// The Steel V1.1 keeps before surplus Steel goes into Riflemen and Barracks: a Tank's 425 and a
/// little. V1.0 kept 600, and against Pressure AI on Classic its one Barracks stood idle from 2:45 to
/// 3:45 with 500-950 Steel in the bank while the first Tank waited on its Oil.
const SURPLUS_RESERVE: u32 = 450;

pub(super) fn surplus_reserve(profile_id: &str, reserve: u32) -> u32 {
    if is_jeff_v1_1(profile_id) && !off("surplus") {
        reserve.min(SURPLUS_RESERVE)
    } else {
        reserve
    }
}

/// Whether V1.1's Barracks trains Riflemen while it would save for the first Tank: Riflemen take
/// no Oil, and the first Tank waits on its Oil, not its Steel. V1.0's Barracks saved everything for
/// it, and against Pressure AI on Classic stood idle 3:30-3:45 with 565-885 Steel in the bank. Only
/// the Steel past the Tank's goes.
pub(super) fn rifles_past_first_tank(
    profile_id: &str,
    building_kind: EntityKind,
    steel: u32,
) -> bool {
    let tank = rts_rules::economy::cost(EntityKind::Tank).0;
    let rifle = rts_rules::economy::cost(EntityKind::Rifleman).0;
    is_jeff_v1_1(profile_id)
        && !off("surplus")
        && building_kind == EntityKind::Barracks
        && steel >= tank + rifle
}

/// The Steel V1.1's building orders still owe. An order stays pending while its builder is in its
/// build state, which lasts through construction, and is dropped only once the builder has stood
/// still for 10 s: for 10 s after each foundation went down Jeff held its cost back a second time
/// (the Training Centre's 100 Steel from 0:28 to 0:38 against Pressure AI on Classic, with the
/// Barracks idle). An order whose foundation stands owes nothing more.
pub(super) fn committed_steel(profile_id: &str, observation: &AiObservation, facts: &AiFacts) -> u32 {
    if !is_jeff_v1_1(profile_id) || off("committed") {
        return facts.committed_steel;
    }
    let ts = observation.map.tile_size;
    observation
        .pending_builds
        .iter()
        .filter(|build| build.phase == crate::ai_core::observation::AiBuildIntentPhase::ToSite)
        .filter(|build| {
            let Some(center) =
                super::geometry::building_center((build.tile_x, build.tile_y), build.kind, ts)
            else {
                return true;
            };
            !observation.owned.iter().any(|entity| {
                entity.kind == build.kind
                    && super::geometry::dist2(entity.x, entity.y, center.0, center.1)
                        <= super::geometry::squared(ts as f32)
            })
        })
        .map(|build| rts_rules::economy::cost(build.kind).0)
        .fold(0u32, u32::saturating_add)
}

/// Local ablation testing only: `JEFF_V11_OFF=scout,raid,...` turns the named V1.1 features off,
/// and `JEFF_V11_ONLY=meth,lines,...` turns off every feature it does not name.
pub(super) fn off(feature: &str) -> bool {
    crate::ai_core::profiles::jeff_v1_1_feature_off(feature)
}

fn ordered(facts: &AiFacts, memory: &AiDecisionMemory, upgrade: UpgradeKind) -> bool {
    facts.completed_upgrades().contains(&upgrade) || memory.pending_upgrades.contains(&upgrade)
}

/// Orders `upgrade` at a free building that researches it. Unlike `queue_upgrade_if_available`, it
/// never holds Oil research back for the natural: V1.1's research is in the spend plan.
fn research(
    actions: &mut AiActionContext<'_>,
    facts: &AiFacts,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    upgrade: UpgradeKind,
) -> bool {
    let definition = upgrade::definition(upgrade);
    let Some(researched) = actions::try_research_upgrade(
        actions,
        facts.production_buildings(definition.researched_at),
        upgrade,
    ) else {
        return false;
    };
    memory.pending_upgrades.insert(researched.upgrade);
    memory.note_research_producer(researched.upgrade, researched.building);
    intents.push(AiIntent::Research {
        upgrade: researched.upgrade,
    });
    true
}

/// Oil the spend plan's forecast expects within `ticks`, between its 15, 30 and 60 s marks.
fn oil_within(plan: &spend_plan::SpendPlan, ticks: u32) -> f32 {
    let marks = [
        (0, 0.0),
        (450, plan.expected[0].1),
        (900, plan.expected[1].1),
        (1800, plan.expected[2].1),
    ];
    for pair in marks.windows(2) {
        let ((t0, o0), (t1, o1)) = (pair[0], pair[1]);
        if ticks <= t1 {
            return o0 + (o1 - o0) * (ticks - t0) as f32 / (t1 - t0) as f32;
        }
    }
    plan.expected[2].1
}

/// Guns on the line in front of each finished Depot past the main. (Milder than at first: four a
/// base, four more held home and up to 30 cost V1.1 games against Pressure AI's Tank waves.)
const GUNS_PER_BASE: usize = 2;
/// Guns held at the home post on top of the home pocket's and the bases' posts.
const HOME_GUN_RESERVE: usize = 0;
const MAX_GUNS: usize = 16;
/// `mgline`: guns trained for every Depot (`gun_cap`).
const SPREAD_GUNS_PER_BASE: usize = 5;
/// `fastnat`: the third base goes up without Tanks from this time on, once this many guns stand
/// within `NATURAL_GUN_TILES` of the natural's Depot.
const FAST_THIRD_TICKS: u32 = config::TICK_HZ * 210;
const FAST_THIRD_GUNS: usize = 4;
const NATURAL_GUN_TILES: f32 = 18.0;
/// A Depot this far from the main's start is a base past it.
const PAST_MAIN_TILES: f32 = 12.0;

/// `fastnat`: whether the third base may go up without Tanks: from `FAST_THIRD_TICKS` on, with
/// `FAST_THIRD_GUNS` guns holding the natural.
pub(super) fn fast_third_ready(observation: &AiObservation, profile_id: &str) -> bool {
    if !is_jeff_v1_1(profile_id) || off("fastnat") || observation.tick < FAST_THIRD_TICKS {
        return false;
    }
    let ts = observation.map.tile_size as f32;
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let naturals: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.is_complete && unit.hp > 0)
        .filter(|unit| dist2(unit.x, unit.y, home.0, home.1) > squared(PAST_MAIN_TILES * ts))
        .map(|unit| (unit.x, unit.y))
        .collect();
    observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::MachineGunner && unit.is_complete && unit.hp > 0)
        .filter(|unit| {
            naturals
                .iter()
                .any(|depot| dist2(unit.x, unit.y, depot.0, depot.1) <= squared(NATURAL_GUN_TILES * ts))
        })
        .count()
        >= FAST_THIRD_GUNS
}
/// V1.1 keeps four Riflemen home and two for every Tank, a screen in front of the Tanks against
/// Panzerfausts.
const HOME_RIFLES: usize = 4;
pub(super) const RIFLES_PER_TANK: usize = 2;
const MAX_RIFLE_LINE: usize = 30;
/// The most Riflemen a push takes as its screen.
pub(super) const MAX_PUSH_RIFLES: usize = 20;

/// The Rifleman line V1.1 trains toward: four home and two for every Tank. Riflemen go into the
/// production priorities, after Tanks and guns, while the line is short; the training loop raises
/// each Barracks' Rifleman cap to it (`opening_rush::raise_rifle_cap`), and the spend plan still caps
/// routine Riflemen before the first Tank.
pub(super) fn rifle_line(
    profile_id: &str,
    facts: &AiFacts,
    (banked, fausts): (bool, usize),
    priorities: &mut Vec<EntityKind>,
) -> Option<usize> {
    if !is_jeff_v1_1(profile_id) || off("rifleline") {
        return None;
    }
    // Steel banked with nothing else to buy goes into Riflemen, up to the most a line takes.
    let line = if banked {
        MAX_RIFLE_LINE
    } else {
        (HOME_RIFLES + RIFLES_PER_TANK * facts.unit_count(EntityKind::Tank)).min(MAX_RIFLE_LINE)
    };
    if facts.unit_count(EntityKind::Rifleman) < line && !priorities.contains(&EntityKind::Rifleman)
    {
        priorities.push(EntityKind::Rifleman);
    }
    // Panzerfausts up to `faust_target`, ahead of the Riflemen: behind them they were never trained,
    // as surplus Steel kept raising the Rifleman cap.
    if facts.unit_count(EntityKind::Panzerfaust) < fausts
        && !priorities.contains(&EntityKind::Panzerfaust)
    {
        match priorities.iter().position(|unit| *unit == EntityKind::Rifleman) {
            Some(index) => priorities.insert(index, EntityKind::Panzerfaust),
            None => priorities.push(EntityKind::Panzerfaust),
        }
    }
    Some(line)
}

/// The Panzerfausts V1.1 keeps: none.
pub(super) fn faust_target(
    _profile_id: &str,
    _facts: &AiFacts,
    _memory: &AiDecisionMemory,
    _banked: bool,
) -> usize {
    // No Panzerfausts: the owner's call is Riflemen and guns to kill the enemy's Panzerfausts, and
    // Tanks to trade with its Tanks. With 16-32 a game, V1.1 fielded 7-15 Tanks and did worse.
    0
}

/// V1.1 trains Machine Gunners for every gun post it has: the home pocket's, four in front of each
/// finished Depot past the main, and four more held at home. Guns past the home pocket wait while
/// the spend plan has no Steel to spare for them, before the first Tank; the opening guns, the
/// expansions and the first Tank's savings still come first. Banked Steel (`third_base::banked`)
/// buys guns up to `MAX_GUNS`.
pub(super) fn gun_cap(
    profile: &AiProfile,
    facts: &AiFacts,
    (plan, banked, fausts): (Option<&spend_plan::SpendPlan>, bool, usize),
    intents: &[AiIntent],
    max_counts: &mut Vec<(EntityKind, usize)>,
) {
    if !is_jeff_v1_1(profile.id) {
        return;
    }
    // `mgline`: `SPREAD_GUNS_PER_BASE` for every Depot, building or built, trained as soon as the
    // Barracks is free. soupman trained guns from 1:06 on and had 8-10 by 3:00 and 10-15 by 4:00;
    // V1.1 held at the opening's four once its first Tank was out.
    if !off("mgline") {
        let cap = (SPREAD_GUNS_PER_BASE * facts.building_count(EntityKind::ResourceDepot))
            .clamp(expansion_security::OPENING_GUNS, MAX_GUNS);
        match max_counts
            .iter_mut()
            .find(|(kind, _)| *kind == EntityKind::MachineGunner)
        {
            Some((_, max)) => *max = cap,
            None => max_counts.push((EntityKind::MachineGunner, cap)),
        }
        return;
    }
    if off("guncap") {
        return;
    }
    let home = profile
        .defensive_machine_gunners
        .map_or(0, |policy| policy.target_count);
    let bases = facts
        .complete_building_count(EntityKind::ResourceDepot)
        .saturating_sub(1);
    let posts = if banked {
        MAX_GUNS
    } else {
        (home + GUNS_PER_BASE * bases + HOME_GUN_RESERVE).min(MAX_GUNS)
    };
    let guns = facts.unit_count(EntityKind::MachineGunner);
    let (gun_steel, _) = rts_rules::economy::cost(EntityKind::MachineGunner);
    let spare = banked || plan.is_none_or(|plan| plan.affords_off_plan(intents, gun_steel));
    let cap = if guns < home || spare { posts } else { guns };
    match max_counts
        .iter_mut()
        .find(|(kind, _)| *kind == EntityKind::MachineGunner)
    {
        Some((_, max)) => *max = cap,
        None => max_counts.push((EntityKind::MachineGunner, cap)),
    }
    match max_counts
        .iter_mut()
        .find(|(kind, _)| *kind == EntityKind::Panzerfaust)
    {
        Some((_, max)) => *max = (*max).max(fausts),
        None => max_counts.push((EntityKind::Panzerfaust, fausts)),
    }
}

/// Unit counts or caps by kind.
type UnitCounts<'a> = &'a [(EntityKind, usize)];
/// Barracks that train from the same priorities.
type BarracksBatch = (Vec<ProductionBuildingFact>, Vec<EntityKind>);

/// V1.1's Barracks, each with the priorities it trains from. With two or more, the oldest trains
/// only Machine Gunners while there are guns to train, and the others train no guns; one Barracks
/// trains everything as before, guns first.
pub(super) fn barracks_batches(
    profile_id: &str,
    building_kind: EntityKind,
    buildings: &[ProductionBuildingFact],
    priorities: &[EntityKind],
    (counts, max_counts): (UnitCounts<'_>, UnitCounts<'_>),
) -> Vec<BarracksBatch> {
    let gun = EntityKind::MachineGunner;
    let Some(gun_barracks) = buildings
        .iter()
        .map(|building| building.id)
        .min()
        .filter(|_| {
            is_jeff_v1_1(profile_id)
                && building_kind == EntityKind::Barracks
                && buildings.len() > 2
                && !off("gunrax")
        })
    else {
        return vec![(buildings.to_vec(), priorities.to_vec())];
    };
    let of = |list: &[(EntityKind, usize)]| {
        list.iter()
            .find_map(|(kind, count)| (*kind == gun).then_some(*count))
    };
    let guns_wanted =
        priorities.contains(&gun) && of(counts).unwrap_or(0) < of(max_counts).unwrap_or(0);
    let no_guns: Vec<EntityKind> = priorities
        .iter()
        .copied()
        .filter(|unit| *unit != gun)
        .collect();
    let (guns, others): (Vec<_>, Vec<_>) = buildings
        .iter()
        .copied()
        .partition(|building| building.id == gun_barracks);
    let gun_priorities = if guns_wanted {
        vec![gun]
    } else {
        no_guns.clone()
    };
    vec![(guns, gun_priorities), (others, no_guns)]
}
