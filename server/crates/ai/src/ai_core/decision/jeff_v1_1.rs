//! Jeff's AI V1.1: what it does differently from V1.0. Every function here acts only for V1.1
//! (`is_jeff_v1_1`); for V1.0 each returns without touching the decision.

use super::*;
use crate::ai_core::facts::ProductionBuildingFact;

pub(super) mod base_line;
pub(super) mod charge;
pub(super) mod hunt;
pub(super) mod natural;
pub(super) mod push;
pub(super) mod reveal;
pub(super) mod scout;
pub(super) mod screen;
pub(super) mod strike;
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
    pub(super) charge: charge::ChargeMemory,
    pub(super) reveal: reveal::RevealMemory,
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
/// of their posts, and the posts cover the base's ways in.
pub(super) fn keep_guns_on_posts(
    profile_id: &str,
    observation: &AiObservation,
    local_defenders: &mut Vec<u32>,
) {
    if !is_jeff_v1_1(profile_id) || off("gunposts") {
        return;
    }
    local_defenders.retain(|id| {
        observation
            .owned
            .iter()
            .all(|unit| unit.id != *id || unit.kind != EntityKind::MachineGunner)
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

/// The natural's guards V1.1 trains before its opening guns.
const NATURAL_GUARDS: usize = 2;

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
        && facts.building_count(EntityKind::ResourceDepot) < 2
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
    if !is_jeff_v1_1(profile.id) || off("guncap") {
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
