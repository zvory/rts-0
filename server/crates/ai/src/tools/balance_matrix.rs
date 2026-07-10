//! Command-line AI profile balance matrix runner.
//!
//! This developer tool runs unordered profile pairs across a configurable number of seeds,
//! writes every replay artifact, and aggregates starting-City-Centre wins plus draws.
#![allow(dead_code)]

use std::path::PathBuf;
use std::process;

use crate::selfplay::{
    available_profile_ids, canonical_profile_id, run_profile_matchup_result, ProfileMatchupOptions,
    ProfileMatchupResult,
};
use rayon::prelude::*;

const DEFAULT_SEEDS: u32 = 5;
const DEFAULT_TICKS: u32 = 25_000;

#[derive(Debug)]
struct CliConfig {
    seeds: u32,
    seed_start: u32,
    ticks: u32,
    profiles: Vec<String>,
    out_dir: PathBuf,
    verify_replay: bool,
}

#[derive(Default)]
struct MatchupAggregate {
    profile_a: String,
    profile_b: String,
    runs: u32,
    wins_a: u32,
    wins_b: u32,
    unresolved_draws: u32,
    army_a_total: u64,
    army_b_total: u64,
    buildings_a_total: u64,
    buildings_b_total: u64,
    workers_a_total: u64,
    workers_b_total: u64,
    damage_a_total: u64,
    damage_b_total: u64,
    deaths_a_total: u64,
    deaths_b_total: u64,
    first_tank_a_total: u64,
    first_tank_a_count: u32,
    first_tank_b_total: u64,
    first_tank_b_count: u32,
    expansion_a_total: u64,
    expansion_a_count: u32,
    expansion_b_total: u64,
    expansion_b_count: u32,
}

pub fn run_from_env() {
    let Some(config) = parse_args_or_exit() else {
        return;
    };

    if let Err(err) = std::fs::create_dir_all(&config.out_dir) {
        eprintln!(
            "failed to create replay output directory {}: {err}",
            config.out_dir.display()
        );
        process::exit(1);
    }

    println!("AI balance matrix");
    println!("profiles: {}", config.profiles.join(", "));
    println!(
        "seeds: {}..{}  ticks: {}  replay dir: {}",
        config.seed_start,
        config
            .seed_start
            .saturating_add(config.seeds)
            .saturating_sub(1),
        config.ticks,
        config.out_dir.display()
    );
    println!(
        "replay verification: {}",
        if config.verify_replay {
            "enabled"
        } else {
            "skipped"
        }
    );
    println!();

    // Build all (pair, seed) jobs upfront so rayon can distribute them freely.
    struct Job {
        profile_a: String,
        profile_b: String,
        seed: u32,
    }

    let mut jobs: Vec<Job> = Vec::new();
    for i in 0..config.profiles.len() {
        for j in (i + 1)..config.profiles.len() {
            for offset in 0..config.seeds {
                jobs.push(Job {
                    profile_a: config.profiles[i].clone(),
                    profile_b: config.profiles[j].clone(),
                    seed: config.seed_start.saturating_add(offset),
                });
            }
        }
    }

    let total = jobs.len();
    println!("running {total} matches in parallel…");

    let results: Vec<(Job, ProfileMatchupResult)> = jobs
        .into_par_iter()
        .map(|job| {
            let name = replay_name(&job.profile_a, &job.profile_b, job.seed);
            let result = run_profile_matchup_result(ProfileMatchupOptions {
                profile_a: job.profile_a.clone(),
                profile_b: job.profile_b.clone(),
                seed: job.seed,
                max_ticks: config.ticks,
                verify_replay: config.verify_replay,
                save_replay_name: Some(name),
                replay_dir: Some(config.out_dir.clone()),
            })
            .unwrap_or_else(|err| {
                eprintln!(
                    "matchup failed for {} vs {} seed {}: {err}",
                    job.profile_a, job.profile_b, job.seed
                );
                process::exit(1);
            });
            (job, result)
        })
        .collect();

    // Re-group by ordered pair to preserve table order.
    let mut aggregates: Vec<MatchupAggregate> = Vec::new();
    for i in 0..config.profiles.len() {
        for j in (i + 1)..config.profiles.len() {
            aggregates.push(MatchupAggregate::new(
                config.profiles[i].clone(),
                config.profiles[j].clone(),
            ));
        }
    }
    for (job, result) in &results {
        if let Some(agg) = aggregates
            .iter_mut()
            .find(|a| a.profile_a == job.profile_a && a.profile_b == job.profile_b)
        {
            agg.record(result);
        }
    }

    print_table(&aggregates);
}

impl MatchupAggregate {
    fn new(profile_a: String, profile_b: String) -> Self {
        Self {
            profile_a,
            profile_b,
            ..Self::default()
        }
    }

    fn record(&mut self, result: &ProfileMatchupResult) {
        self.runs = self.runs.saturating_add(1);
        let Some(player_a) = result.players.iter().find(|player| player.player_id == 1) else {
            return;
        };
        let Some(player_b) = result.players.iter().find(|player| player.player_id == 2) else {
            return;
        };

        self.army_a_total = self.army_a_total.saturating_add(player_a.army_value as u64);
        self.army_b_total = self.army_b_total.saturating_add(player_b.army_value as u64);
        self.buildings_a_total = self
            .buildings_a_total
            .saturating_add(player_a.building_value as u64);
        self.buildings_b_total = self
            .buildings_b_total
            .saturating_add(player_b.building_value as u64);
        self.workers_a_total = self
            .workers_a_total
            .saturating_add(player_a.worker_count as u64);
        self.workers_b_total = self
            .workers_b_total
            .saturating_add(player_b.worker_count as u64);
        self.damage_a_total = self
            .damage_a_total
            .saturating_add(player_a.damage_dealt_events as u64);
        self.damage_b_total = self
            .damage_b_total
            .saturating_add(player_b.damage_dealt_events as u64);
        self.deaths_a_total = self
            .deaths_a_total
            .saturating_add(player_a.death_count as u64);
        self.deaths_b_total = self
            .deaths_b_total
            .saturating_add(player_b.death_count as u64);
        if let Some(tick) = player_a.first_tank_tick {
            self.first_tank_a_total = self.first_tank_a_total.saturating_add(tick as u64);
            self.first_tank_a_count = self.first_tank_a_count.saturating_add(1);
        }
        if let Some(tick) = player_b.first_tank_tick {
            self.first_tank_b_total = self.first_tank_b_total.saturating_add(tick as u64);
            self.first_tank_b_count = self.first_tank_b_count.saturating_add(1);
        }
        if let Some(tick) = player_a.first_expansion_city_centre_planned_tick {
            self.expansion_a_total = self.expansion_a_total.saturating_add(tick as u64);
            self.expansion_a_count = self.expansion_a_count.saturating_add(1);
        }
        if let Some(tick) = player_b.first_expansion_city_centre_planned_tick {
            self.expansion_b_total = self.expansion_b_total.saturating_add(tick as u64);
            self.expansion_b_count = self.expansion_b_count.saturating_add(1);
        }

        if let Some(winner) = &result.winner {
            if winner.player_id == 1 {
                self.wins_a = self.wins_a.saturating_add(1);
            } else if winner.player_id == 2 {
                self.wins_b = self.wins_b.saturating_add(1);
            }
        } else {
            self.unresolved_draws = self.unresolved_draws.saturating_add(1);
        }
    }
}

fn parse_args_or_exit() -> Option<CliConfig> {
    match parse_args(std::env::args().skip(1)) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("{err}");
            eprintln!();
            print_usage();
            process::exit(2);
        }
    }
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<CliConfig>, String> {
    let mut seeds = DEFAULT_SEEDS;
    let mut seed_start = 0;
    let mut ticks = DEFAULT_TICKS;
    let mut profiles = default_profiles();
    let mut out_dir = default_out_dir();
    let mut verify_replay = true;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                return Ok(None);
            }
            "--list-profiles" => {
                print_profiles();
                return Ok(None);
            }
            "--seeds" => {
                seeds = parse_u32_flag(&arg, &mut args)?;
            }
            "--seed-start" => {
                seed_start = parse_u32_flag(&arg, &mut args)?;
            }
            "--ticks" => {
                ticks = parse_u32_flag(&arg, &mut args)?;
            }
            "--profiles" => {
                let value = required_value(&arg, &mut args)?;
                profiles = parse_profiles(&value)?;
            }
            "--out-dir" => {
                out_dir = PathBuf::from(required_value(&arg, &mut args)?);
            }
            "--no-verify-replay" => {
                verify_replay = false;
            }
            _ => return Err(format!("unknown flag: {arg}")),
        }
    }

    if seeds == 0 {
        return Err("--seeds must be greater than zero".to_string());
    }
    if ticks == 0 {
        return Err("--ticks must be greater than zero".to_string());
    }
    if profiles.is_empty() {
        return Err("--profiles must include at least one profile".to_string());
    }
    Ok(Some(CliConfig {
        seeds,
        seed_start,
        ticks,
        profiles,
        out_dir,
        verify_replay,
    }))
}

fn parse_profiles(value: &str) -> Result<Vec<String>, String> {
    let profiles = value
        .split(',')
        .map(str::trim)
        .filter(|profile| !profile.is_empty())
        .map(|profile| {
            canonical_profile_id(profile)
                .map(str::to_string)
                .ok_or_else(|| {
                    format!(
                        "unknown profile {profile:?}; known profiles: {}",
                        available_profile_ids().join(", ")
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    ensure_distinct_profiles(profiles)
}

fn required_value(flag: &str, args: &mut impl Iterator<Item = String>) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn parse_u32_flag(flag: &str, args: &mut impl Iterator<Item = String>) -> Result<u32, String> {
    let value = required_value(flag, args)?;
    value
        .parse()
        .map_err(|_| format!("{flag} requires a u32 value, got {value:?}"))
}

fn default_out_dir() -> PathBuf {
    std::env::temp_dir().join(format!("rts-ai-balance-matrix-{}", process::id()))
}

fn default_profiles() -> Vec<String> {
    available_profile_ids()
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn ensure_distinct_profiles(profiles: Vec<String>) -> Result<Vec<String>, String> {
    let mut distinct = Vec::with_capacity(profiles.len());
    for profile in profiles {
        if distinct.contains(&profile) {
            return Err(format!("duplicate profile {profile:?} in selection"));
        }
        distinct.push(profile);
    }
    Ok(distinct)
}

fn replay_name(profile_a: &str, profile_b: &str, seed: u32) -> String {
    format!("{profile_a}__vs__{profile_b}__seed_{seed}")
}

fn print_table(aggregates: &[MatchupAggregate]) {
    println!(
        "{:<72} {:>4} {:>3} {:>3} {:>3} {:>7} {:>7} {:>7} {:>7} {:>5} {:>5} {:>5} {:>5} {:>5} {:>5} {:>7} {:>7} {:>7} {:>7}",
        "matchup",
        "runs",
        "W",
        "L",
        "D",
        "armyA",
        "bldgA",
        "armyB",
        "bldgB",
        "wrkA",
        "wrkB",
        "dmgA",
        "dmgB",
        "lostA",
        "lostB",
        "tankA",
        "tankB",
        "expA",
        "expB",
    );
    for aggregate in aggregates {
        let runs = aggregate.runs.max(1) as u64;
        println!(
            "{:<72} {:>4} {:>3} {:>3} {:>3} {:>7} {:>7} {:>7} {:>7} {:>5} {:>5} {:>5} {:>5} {:>5} {:>5} {:>7} {:>7} {:>7} {:>7}",
            format!("{} vs {}", aggregate.profile_a, aggregate.profile_b),
            aggregate.runs,
            aggregate.wins_a,
            aggregate.wins_b,
            aggregate.unresolved_draws,
            aggregate.army_a_total / runs,
            aggregate.buildings_a_total / runs,
            aggregate.army_b_total / runs,
            aggregate.buildings_b_total / runs,
            aggregate.workers_a_total / runs,
            aggregate.workers_b_total / runs,
            aggregate.damage_a_total / runs,
            aggregate.damage_b_total / runs,
            aggregate.deaths_a_total / runs,
            aggregate.deaths_b_total / runs,
            average_tick_text(aggregate.first_tank_a_total, aggregate.first_tank_a_count),
            average_tick_text(aggregate.first_tank_b_total, aggregate.first_tank_b_count),
            average_tick_text(aggregate.expansion_a_total, aggregate.expansion_a_count),
            average_tick_text(aggregate.expansion_b_total, aggregate.expansion_b_count),
        );
    }
    println!();
    println!(
        "W/L/D are from the left profile's perspective and use only starting City Centre kills."
    );
    println!("Runs with no starting City Centre winner by the tick cap are draws.");
    println!("army/bldg/wrk/dmg are diagnostic per-run averages, not tiebreakers; tank/exp are average first ticks, or '-' if never seen.");
}

fn average_tick_text(total: u64, count: u32) -> String {
    if count == 0 {
        "-".to_string()
    } else {
        (total / count as u64).to_string()
    }
}

fn print_profiles() {
    println!("profiles:");
    for profile in available_profile_ids() {
        println!("  {profile}");
    }
}

fn print_usage() {
    println!(
        "Usage:
  ai-balance-matrix [options]

Options:
  --seeds <u32>          Number of seeds per unordered matchup (default: {DEFAULT_SEEDS})
  --seed-start <u32>     First seed to run (default: 0)
  --ticks <u32>          Tick cap per run (default: {DEFAULT_TICKS})
  --profiles <csv>       Comma-separated explicit profile list (default: all available profiles)
  --out-dir <path>       Replay parent directory (default: /tmp/rts-ai-balance-matrix-<pid>)
  --no-verify-replay     Skip deterministic command-log replay verification
  --list-profiles        Print available profiles
  -h, --help             Print this help
"
    );
}

#[cfg(test)]
mod tests {
    use super::{default_profiles, ensure_distinct_profiles, parse_args};
    use crate::selfplay::{
        ProfileMatchupEndReason, ProfileMatchupPlayerResult, ProfileMatchupResult,
        ProfileMatchupStartingCityCentreResult, ProfileMatchupWinner,
    };
    use std::collections::BTreeMap;

    #[test]
    fn default_profile_selection_uses_all_available_profiles() {
        let config = parse_args(Vec::<String>::new())
            .expect("default args should parse")
            .expect("default args should return config");

        assert_eq!(config.profiles, default_profiles());
        assert_eq!(config.ticks, super::DEFAULT_TICKS);
        assert_eq!(config.ticks, 25_000);
    }

    #[test]
    fn duplicate_profile_selection_is_rejected() {
        let err = ensure_distinct_profiles(vec!["ai_2_1".to_string(), "ai_2_1".to_string()])
            .expect_err("duplicate profiles should fail");

        assert!(err.contains("duplicate profile"));
    }

    #[test]
    fn single_profile_selection_is_allowed() {
        let config = parse_args(vec!["--profiles".to_string(), "ai_2_1".to_string()])
            .expect("single profile should parse")
            .expect("single profile should return config");

        assert_eq!(config.profiles, vec!["ai_2_1".to_string()]);
    }

    #[test]
    fn tick_cap_draw_is_not_resolved_by_army_value() {
        let mut aggregate = super::MatchupAggregate::new("left".to_string(), "right".to_string());
        aggregate.record(&profile_result(
            ProfileMatchupEndReason::TickCap,
            None,
            1_000,
            10,
        ));

        assert_eq!(aggregate.runs, 1);
        assert_eq!(aggregate.wins_a, 0);
        assert_eq!(aggregate.wins_b, 0);
        assert_eq!(aggregate.unresolved_draws, 1);
    }

    #[test]
    fn starting_city_centre_winner_counts_even_with_lower_army_value() {
        let mut aggregate = super::MatchupAggregate::new("left".to_string(), "right".to_string());
        aggregate.record(&profile_result(
            ProfileMatchupEndReason::StartingCityCentreKilled,
            Some(2),
            1_000,
            10,
        ));

        assert_eq!(aggregate.runs, 1);
        assert_eq!(aggregate.wins_a, 0);
        assert_eq!(aggregate.wins_b, 1);
        assert_eq!(aggregate.unresolved_draws, 0);
    }

    fn profile_result(
        end_reason: ProfileMatchupEndReason,
        winner_player_id: Option<u32>,
        player_one_army: u32,
        player_two_army: u32,
    ) -> ProfileMatchupResult {
        ProfileMatchupResult {
            profile_a: "left".to_string(),
            profile_b: "right".to_string(),
            seed: 0,
            max_ticks: 120,
            ticks: 120,
            end_reason,
            winner: winner_player_id.map(|player_id| ProfileMatchupWinner {
                player_id,
                profile: profile_for_player(player_id).to_string(),
            }),
            starting_city_centres: vec![
                starting_city_centre(1, (winner_player_id == Some(2)).then_some(120)),
                starting_city_centre(2, (winner_player_id == Some(1)).then_some(120)),
            ],
            players: vec![
                player_result(1, player_one_army),
                player_result(2, player_two_army),
            ],
            first_damage_tick: None,
            attack_events: 0,
            death_events: 0,
            event_count: 0,
            replay_verified: false,
            replay_artifact: None,
            ai_trace_tail: Vec::new(),
        }
    }

    fn player_result(player_id: u32, army_value: u32) -> ProfileMatchupPlayerResult {
        ProfileMatchupPlayerResult {
            player_id,
            profile: profile_for_player(player_id).to_string(),
            alive: true,
            army_value,
            building_value: 0,
            worker_count: 0,
            command_count: 0,
            attack_command_count: 0,
            damage_dealt_events: 0,
            death_count: 0,
            first_attack_command_tick: None,
            first_rifleman_attack_command_tick: None,
            first_scout_car_tick: None,
            first_scout_car_harass_command_tick: None,
            first_expansion_city_centre_planned_tick: None,
            first_expansion_city_centre_completed_tick: None,
            first_tank_tick: None,
            final_counts: BTreeMap::new(),
        }
    }

    fn starting_city_centre(
        player_id: u32,
        death_tick: Option<u32>,
    ) -> ProfileMatchupStartingCityCentreResult {
        ProfileMatchupStartingCityCentreResult {
            player_id,
            profile: profile_for_player(player_id).to_string(),
            entity_id: player_id * 100,
            alive: death_tick.is_none(),
            death_tick,
        }
    }

    fn profile_for_player(player_id: u32) -> &'static str {
        if player_id == 1 {
            "left"
        } else {
            "right"
        }
    }
}
