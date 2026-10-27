//! Jeff's AI V1.1: what it does differently from V1.0. Every function here acts only for V1.1
//! (`is_jeff_v1_1`); for V1.0 each returns without touching the decision.

use super::*;
use crate::ai_core::facts::ProductionBuildingFact;

pub(super) mod base_line;
pub(super) mod push;
pub(super) mod third_base;

/// What V1.1 remembers between decisions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct V1_1Memory {
    /// Entrenchment waits for Methamphetamines this decision: Meth's Oil comes in before
    /// Entrenchment's research would end, so the Training Centre stays free for Meth.
    pub(super) entrenchment_waits: bool,
    /// Each base-line unit's post (`base_line`).
    pub(super) base_lines: BTreeMap<u32, base_line::LinePost>,
    /// Since when Steel has stood at `third_base::BANK_STEEL` or more.
    pub(super) banked_since: Option<u32>,
    pub(super) push: push::PushMemory,
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
    if !is_jeff_v1_1(profile_id) || facts.complete_building_count(EntityKind::TrainingCentre) == 0 {
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
    if !is_jeff_v1_1(profile_id) {
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
    if !is_jeff_v1_1(profile_id) {
        return;
    }
    local_defenders.retain(|id| {
        observation
            .owned
            .iter()
            .all(|unit| unit.id != *id || unit.kind != EntityKind::MachineGunner)
    });
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

/// Guns on the line in front of each finished Depot past the main.
const GUNS_PER_BASE: usize = 4;
/// Guns held at the home post on top of the home pocket's and the bases' posts.
const HOME_GUN_RESERVE: usize = 4;
const MAX_GUNS: usize = 30;
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
    banked: bool,
    priorities: &mut Vec<EntityKind>,
) -> Option<usize> {
    if !is_jeff_v1_1(profile_id) {
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
    Some(line)
}

/// V1.1 trains Machine Gunners for every gun post it has: the home pocket's, four in front of each
/// finished Depot past the main, and four more held at home. Guns past the home pocket wait while
/// the spend plan has no Steel to spare for them, before the first Tank; the opening guns, the
/// expansions and the first Tank's savings still come first. Banked Steel (`third_base::banked`)
/// buys guns up to `MAX_GUNS`.
pub(super) fn gun_cap(
    profile: &AiProfile,
    facts: &AiFacts,
    (plan, banked): (Option<&spend_plan::SpendPlan>, bool),
    intents: &[AiIntent],
    max_counts: &mut Vec<(EntityKind, usize)>,
) {
    if !is_jeff_v1_1(profile.id) {
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
            is_jeff_v1_1(profile_id) && building_kind == EntityKind::Barracks && buildings.len() > 1
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
