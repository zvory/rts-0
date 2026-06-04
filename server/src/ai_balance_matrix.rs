//! Command-line AI profile balance matrix runner.
//!
//! This developer tool runs unordered profile pairs across a configurable number of seeds,
//! writes every replay artifact, and aggregates elimination wins plus army-value-resolved
//! tick-cap draws.
#![allow(dead_code)]

mod config;
mod game;
mod protocol;
mod rules;

use std::path::PathBuf;
use std::process;

use game::selfplay::{
    available_profile_ids, canonical_profile_id, run_profile_matchup_result, ProfileMatchupOptions,
    ProfileMatchupResult,
};

const DEFAULT_SEEDS: u32 = 5;
const DEFAULT_TICKS: u32 = 20_000;
const DEFAULT_PROFILES: &[&str] = &[
    "rifle_flood_fast",
    "rifle_flood_full_saturation",
    "tech_to_tanks",
    "steel_expansion_tanks",
];

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
    raw_draws: u32,
    army_tiebreaks_a: u32,
    army_tiebreaks_b: u32,
    eliminations_a: u32,
    eliminations_b: u32,
    army_a_total: u64,
    army_b_total: u64,
    buildings_a_total: u64,
    buildings_b_total: u64,
}

fn main() {
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

    let mut aggregates = Vec::new();
    for i in 0..config.profiles.len() {
        for j in i..config.profiles.len() {
            let mut aggregate =
                MatchupAggregate::new(config.profiles[i].clone(), config.profiles[j].clone());
            for offset in 0..config.seeds {
                let seed = config.seed_start.saturating_add(offset);
                let replay_name = replay_name(&aggregate.profile_a, &aggregate.profile_b, seed);
                let result = run_profile_matchup_result(ProfileMatchupOptions {
                    profile_a: aggregate.profile_a.clone(),
                    profile_b: aggregate.profile_b.clone(),
                    seed,
                    max_ticks: config.ticks,
                    verify_replay: config.verify_replay,
                    save_replay_name: Some(replay_name),
                    replay_dir: Some(config.out_dir.clone()),
                })
                .unwrap_or_else(|err| {
                    eprintln!(
                        "matchup failed for {} vs {} seed {}: {err}",
                        aggregate.profile_a, aggregate.profile_b, seed
                    );
                    process::exit(1);
                });
                aggregate.record(&result);
            }
            aggregates.push(aggregate);
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

        if let Some(winner) = &result.winner {
            if winner.player_id == 1 {
                self.wins_a = self.wins_a.saturating_add(1);
                self.eliminations_a = self.eliminations_a.saturating_add(1);
            } else if winner.player_id == 2 {
                self.wins_b = self.wins_b.saturating_add(1);
                self.eliminations_b = self.eliminations_b.saturating_add(1);
            }
            return;
        }

        self.raw_draws = self.raw_draws.saturating_add(1);
        match player_a.army_value.cmp(&player_b.army_value) {
            std::cmp::Ordering::Greater => {
                self.wins_a = self.wins_a.saturating_add(1);
                self.army_tiebreaks_a = self.army_tiebreaks_a.saturating_add(1);
            }
            std::cmp::Ordering::Less => {
                self.wins_b = self.wins_b.saturating_add(1);
                self.army_tiebreaks_b = self.army_tiebreaks_b.saturating_add(1);
            }
            std::cmp::Ordering::Equal => {
                self.unresolved_draws = self.unresolved_draws.saturating_add(1);
            }
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
    let mut profiles = DEFAULT_PROFILES
        .iter()
        .map(|profile| (*profile).to_string())
        .collect::<Vec<_>>();
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
    value
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
        .collect()
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

fn replay_name(profile_a: &str, profile_b: &str, seed: u32) -> String {
    format!("{profile_a}__vs__{profile_b}__seed_{seed}")
}

fn print_table(aggregates: &[MatchupAggregate]) {
    println!(
        "{:<72} {:>4} {:>3} {:>3} {:>3} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7}",
        "matchup", "runs", "W", "L", "D", "rawD", "tbW", "tbL", "armyA", "bldgA", "armyB", "bldgB",
    );
    for aggregate in aggregates {
        let runs = aggregate.runs.max(1) as u64;
        println!(
            "{:<72} {:>4} {:>3} {:>3} {:>3} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7}",
            format!("{} vs {}", aggregate.profile_a, aggregate.profile_b),
            aggregate.runs,
            aggregate.wins_a,
            aggregate.wins_b,
            aggregate.unresolved_draws,
            aggregate.raw_draws,
            aggregate.army_tiebreaks_a,
            aggregate.army_tiebreaks_b,
            aggregate.army_a_total / runs,
            aggregate.buildings_a_total / runs,
            aggregate.army_b_total / runs,
            aggregate.buildings_b_total / runs,
        );
    }
    println!();
    println!("W/L/D are from the left profile's perspective.");
    println!("Tick-cap draws are counted in rawD; non-tied army value resolves them into W or L.");
    println!("armyA/bldgA/armyB/bldgB are per-run averages.");
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
  --profiles <csv>       Comma-separated explicit profile list
  --out-dir <path>       Replay parent directory (default: /tmp/rts-ai-balance-matrix-<pid>)
  --no-verify-replay     Skip deterministic command-log replay verification
  --list-profiles        Print available profiles
  -h, --help             Print this help
"
    );
}
