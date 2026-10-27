//! Jeff's AI V1.1: what it does differently from V1.0. Every function here acts only for V1.1
//! (`is_jeff_v1_1`); for V1.0 each returns without touching the decision.

use super::*;

/// What V1.1 remembers between decisions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct V1_1Memory {
    /// Entrenchment waits for Methamphetamines this decision: Meth's Oil comes in before
    /// Entrenchment's research would end, so the Training Centre stays free for Meth.
    pub(super) entrenchment_waits: bool,
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
    if !is_jeff_v1_1(profile_id) || facts.complete_building_count(EntityKind::TrainingCentre) == 0
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
pub(super) fn research_ahead(profile_id: &str, facts: &AiFacts, memory: &AiDecisionMemory) -> (u32, u32) {
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
    let marks = [(0, 0.0), (450, plan.expected[0].1), (900, plan.expected[1].1), (1800, plan.expected[2].1)];
    for pair in marks.windows(2) {
        let ((t0, o0), (t1, o1)) = (pair[0], pair[1]);
        if ticks <= t1 {
            return o0 + (o1 - o0) * (ticks - t0) as f32 / (t1 - t0) as f32;
        }
    }
    plan.expected[2].1
}
