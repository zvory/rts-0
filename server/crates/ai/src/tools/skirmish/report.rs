//! Aggregation and the human/agent-readable briefs for skirmish runs.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::Serialize;

use super::fight::{FightOutcome, FightResult};

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Aggregate {
    pub(crate) fights: u32,
    pub(crate) wins: u32,
    pub(crate) losses: u32,
    pub(crate) draws: u32,
    pub(crate) no_contact: u32,
    pub(crate) win_rate: f32,
    /// Mean of per-fight `hpMargin`; the score a sweep maximizes.
    pub(crate) mean_hp_margin: f32,
    pub(crate) mean_candidate_survivors: f32,
    pub(crate) mean_opponent_survivors: f32,
    pub(crate) mean_ticks: f32,
    /// Damage dealt per shot fired as a fraction of one full shot (below 1.0 means wasted shots).
    pub(crate) shot_efficiency: Option<f32>,
    pub(crate) reinforced_fights: u32,
}

impl Aggregate {
    pub(crate) fn from_results<'a>(
        results: impl IntoIterator<Item = &'a FightResult>,
        shot_damage: u32,
    ) -> Self {
        let mut aggregate = Self::default();
        let (mut margin, mut ours, mut theirs, mut ticks) = (0.0_f64, 0_u64, 0_u64, 0_u64);
        let (mut dealt, mut shots) = (0_u64, 0_u64);
        for result in results {
            aggregate.fights += 1;
            match result.outcome {
                FightOutcome::Win => aggregate.wins += 1,
                FightOutcome::Loss => aggregate.losses += 1,
                FightOutcome::Draw => aggregate.draws += 1,
                FightOutcome::NoContact => aggregate.no_contact += 1,
            }
            if result.reinforced {
                aggregate.reinforced_fights += 1;
            }
            margin += f64::from(result.hp_margin);
            ours += u64::from(result.candidate_survivors);
            theirs += u64::from(result.opponent_survivors);
            ticks += u64::from(result.ticks);
            dealt += u64::from(result.damage_dealt);
            shots += u64::from(result.candidate_shots);
        }
        if aggregate.fights == 0 {
            return aggregate;
        }
        let n = f64::from(aggregate.fights);
        aggregate.win_rate = (f64::from(aggregate.wins) / n) as f32;
        aggregate.mean_hp_margin = (margin / n) as f32;
        aggregate.mean_candidate_survivors = (ours as f64 / n) as f32;
        aggregate.mean_opponent_survivors = (theirs as f64 / n) as f32;
        aggregate.mean_ticks = (ticks as f64 / n) as f32;
        aggregate.shot_efficiency = (shots > 0 && shot_damage > 0)
            .then(|| (dealt as f64 / (shots as f64 * f64::from(shot_damage))) as f32);
        aggregate
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioAggregate {
    pub(crate) scenario: String,
    #[serde(flatten)]
    pub(crate) aggregate: Aggregate,
}

pub(crate) fn by_side(results: &[FightResult], shot_damage: u32) -> BTreeMap<String, Aggregate> {
    [1, 2]
        .into_iter()
        .map(|player| {
            (
                format!("player{player}"),
                Aggregate::from_results(
                    results.iter().filter(|r| r.candidate_player == player),
                    shot_damage,
                ),
            )
        })
        .collect()
}

/// Per-scenario aggregates, worst mean margin first.
pub(crate) fn by_scenario(results: &[FightResult], shot_damage: u32) -> Vec<ScenarioAggregate> {
    let mut grouped: BTreeMap<&str, Vec<&FightResult>> = BTreeMap::new();
    for result in results {
        grouped.entry(&result.scenario).or_default().push(result);
    }
    let mut scenarios: Vec<ScenarioAggregate> = grouped
        .into_iter()
        .map(|(scenario, results)| ScenarioAggregate {
            scenario: scenario.to_string(),
            aggregate: Aggregate::from_results(results, shot_damage),
        })
        .collect();
    scenarios.sort_by(|a, b| {
        a.aggregate
            .mean_hp_margin
            .total_cmp(&b.aggregate.mean_hp_margin)
            .then_with(|| a.scenario.cmp(&b.scenario))
    });
    scenarios
}

pub(crate) struct BriefContext<'a> {
    pub(crate) candidate: &'a str,
    pub(crate) opponent: &'a str,
    pub(crate) suite: &'a str,
    pub(crate) scenarios: usize,
    pub(crate) seeds: u32,
    pub(crate) map_name: &'a str,
    pub(crate) squad_size: usize,
    pub(crate) max_ticks: u32,
    pub(crate) build: &'a str,
}

pub(crate) fn evaluation_brief(
    context: &BriefContext<'_>,
    overall: &Aggregate,
    sides: &BTreeMap<String, Aggregate>,
    scenarios: &[ScenarioAggregate],
    results: &[FightResult],
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Squad micro skirmish\n");
    let _ = writeln!(out, "- Candidate: `{}`", context.candidate);
    let _ = writeln!(out, "- Opponent: `{}`", context.opponent);
    let _ = writeln!(
        out,
        "- Suite `{}`: {} scenarios x 2 sides x {} seed(s) = {} fights, {} v {} riflemen on `{}`, cap {} ticks",
        context.suite,
        context.scenarios,
        context.seeds,
        overall.fights,
        context.squad_size,
        context.squad_size,
        context.map_name,
        context.max_ticks,
    );
    let _ = writeln!(out, "- Build: `{}`\n", context.build);
    let _ = writeln!(
        out,
        "| Slice | Fights | W | L | D | No contact | Win rate | Mean HP margin | Survivors us/them | Shot efficiency |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|---|---|---|---|---|");
    aggregate_row(&mut out, "All", overall);
    for (side, aggregate) in sides {
        aggregate_row(&mut out, &format!("As {side}"), aggregate);
    }
    if overall.reinforced_fights > 0 {
        let _ = writeln!(
            out,
            "\n**Warning:** {} fight(s) saw a new combat unit join mid-fight; their margins are not pure squad micro.",
            overall.reinforced_fights
        );
    }
    let _ = writeln!(out, "\n## Worst scenarios\n");
    let _ = writeln!(out, "| Scenario | Fights | W-L-D | Mean HP margin |");
    let _ = writeln!(out, "|---|---|---|---|");
    for scenario in scenarios.iter().take(8) {
        let a = &scenario.aggregate;
        let _ = writeln!(
            out,
            "| `{}` | {} | {}-{}-{} | {:+.3} |",
            scenario.scenario,
            a.fights,
            a.wins,
            a.losses,
            a.draws + a.no_contact,
            a.mean_hp_margin
        );
    }
    let mut replays: Vec<&FightResult> = results
        .iter()
        .filter(|result| result.replay_artifact.is_some())
        .collect();
    replays.sort_by(|a, b| a.hp_margin.total_cmp(&b.hp_margin));
    if !replays.is_empty() {
        let _ = writeln!(out, "\n## Replays (worst first)\n");
        let _ = writeln!(
            out,
            "Run a local server (`cd server && cargo run`) and open `http://localhost:<port>/?replayArtifact=<name>`.\n"
        );
        for result in replays.iter().take(12) {
            let _ = writeln!(
                out,
                "- `{}`: {} as player {}, {:?}, margin {:+.3}",
                result.replay_artifact.as_deref().unwrap_or_default(),
                result.scenario,
                result.candidate_player,
                result.outcome,
                result.hp_margin
            );
        }
    }
    out
}

fn aggregate_row(out: &mut String, label: &str, a: &Aggregate) {
    let _ = writeln!(
        out,
        "| {} | {} | {} | {} | {} | {} | {:.1}% | {:+.3} | {:.2}/{:.2} | {} |",
        label,
        a.fights,
        a.wins,
        a.losses,
        a.draws,
        a.no_contact,
        a.win_rate * 100.0,
        a.mean_hp_margin,
        a.mean_candidate_survivors,
        a.mean_opponent_survivors,
        a.shot_efficiency
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| "-".to_string()),
    );
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SweepEntry {
    pub(crate) rank: usize,
    pub(crate) label: String,
    pub(crate) params: String,
    pub(crate) train: Aggregate,
    pub(crate) holdout: Option<Aggregate>,
}

pub(crate) fn sweep_brief(opponent: &str, trials: u32, entries: &[SweepEntry]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Squad micro sweep vs `{opponent}`\n");
    let _ = writeln!(
        out,
        "{trials} random parameter sets plus the reference presets, ranked by mean HP margin on the \
         `train` suite. The top entries are re-run on the `holdout` suite, whose distances, \
         formations, angles, and fight location differ from `train`; trust a parameter set only \
         when its holdout result agrees.\n"
    );
    let _ = writeln!(
        out,
        "| Rank | Label | Train margin | Train win | Holdout margin | Holdout win | Params |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|---|---|");
    for entry in entries {
        let (holdout_margin, holdout_win) = entry
            .holdout
            .as_ref()
            .map(|h| {
                (
                    format!("{:+.3}", h.mean_hp_margin),
                    format!("{:.1}%", h.win_rate * 100.0),
                )
            })
            .unwrap_or_else(|| ("-".to_string(), "-".to_string()));
        let _ = writeln!(
            out,
            "| {} | {} | {:+.3} | {:.1}% | {} | {} | `{}` |",
            entry.rank,
            entry.label,
            entry.train.mean_hp_margin,
            entry.train.win_rate * 100.0,
            holdout_margin,
            holdout_win,
            entry.params
        );
    }
    out
}
