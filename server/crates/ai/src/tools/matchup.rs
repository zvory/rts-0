//! Command-line profile matchup runner.
//!
//! This is a developer/test tool. It runs one directed self-play profile matchup to either
//! elimination or a fixed tick cap and prints the result.
#![allow(dead_code)]

use std::path::PathBuf;
use std::process;

use crate::selfplay::{
    available_profile_ids, canonical_profile_id, run_profile_matchup_result, ProfileMatchupOptions,
    ProfileMatchupResult,
};

const DEFAULT_TICKS: u32 = 20_000;
const DEFAULT_SEED: u32 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutputFormat {
    Table,
    Json,
}

struct CliConfig {
    profile_a: String,
    profile_b: String,
    seed: u32,
    ticks: u32,
    verify_replay: bool,
    save_replay_name: Option<String>,
    replay_dir: Option<PathBuf>,
    output_format: OutputFormat,
}

pub fn run_from_env() {
    let Some(config) = parse_args_or_exit() else {
        return;
    };

    let result = run_profile_matchup_result(ProfileMatchupOptions {
        profile_a: config.profile_a,
        profile_b: config.profile_b,
        seed: config.seed,
        max_ticks: config.ticks,
        verify_replay: config.verify_replay,
        save_replay_name: config.save_replay_name,
        replay_dir: config.replay_dir,
    });

    match result {
        Ok(result) => match config.output_format {
            OutputFormat::Table => print_table(&result),
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(&result).unwrap_or_else(|err| {
                    eprintln!("failed to serialize result: {err}");
                    process::exit(1);
                });
                println!("{json}");
            }
        },
        Err(err) => {
            eprintln!("ai-matchup failed: {err}");
            process::exit(1);
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
    let mut profile_a = None;
    let mut profile_b = None;
    let mut seed = DEFAULT_SEED;
    let mut ticks = DEFAULT_TICKS;
    let mut verify_replay = true;
    let mut save_replay_name = None;
    let mut replay_dir = None;
    let mut output_format = OutputFormat::Table;
    let mut positionals = Vec::new();

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
            "--profile-a" | "--a" => {
                profile_a = Some(required_value(&arg, &mut args)?);
            }
            "--profile-b" | "--b" => {
                profile_b = Some(required_value(&arg, &mut args)?);
            }
            "--seed" => {
                seed = parse_u32_flag(&arg, &mut args)?;
            }
            "--ticks" => {
                ticks = parse_u32_flag(&arg, &mut args)?;
            }
            "--format" => {
                let value = required_value(&arg, &mut args)?;
                output_format = match value.as_str() {
                    "table" => OutputFormat::Table,
                    "json" => OutputFormat::Json,
                    _ => return Err(format!("--format must be table or json, got {value:?}")),
                };
            }
            "--json" => {
                output_format = OutputFormat::Json;
            }
            "--no-verify-replay" => {
                verify_replay = false;
            }
            "--save-replay" => {
                save_replay_name = Some(required_value(&arg, &mut args)?);
            }
            "--replay-dir" => {
                replay_dir = Some(PathBuf::from(required_value(&arg, &mut args)?));
            }
            _ if arg.starts_with('-') => {
                return Err(format!("unknown flag: {arg}"));
            }
            _ => {
                positionals.push(arg);
            }
        }
    }

    if profile_a.is_none() {
        profile_a = positionals.first().cloned();
    }
    if profile_b.is_none() {
        profile_b = positionals.get(1).cloned();
    }
    if positionals.len() > 2 {
        return Err(format!(
            "unexpected positional argument: {}",
            positionals[2]
        ));
    }

    let profile_a = profile_a.ok_or_else(|| "missing profile A".to_string())?;
    let profile_b = profile_b.ok_or_else(|| "missing profile B".to_string())?;
    let profile_a = resolve_profile_arg(&profile_a)?;
    let profile_b = resolve_profile_arg(&profile_b)?;
    if ticks == 0 {
        return Err("--ticks must be greater than zero".to_string());
    }

    Ok(Some(CliConfig {
        profile_a,
        profile_b,
        seed,
        ticks,
        verify_replay,
        save_replay_name,
        replay_dir,
        output_format,
    }))
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

fn resolve_profile_arg(value: &str) -> Result<String, String> {
    canonical_profile_id(value)
        .map(str::to_string)
        .ok_or_else(|| {
            format!(
                "unknown profile {value:?}; known profiles: {}",
                available_profile_ids().join(", ")
            )
        })
}

fn print_table(result: &ProfileMatchupResult) {
    println!(
        "matchup: {} (player 1) vs {} (player 2)",
        result.profile_a, result.profile_b
    );
    println!(
        "seed: {}  ticks: {}/{}  result: {}",
        result.seed,
        result.ticks,
        result.max_ticks,
        winner_text(result)
    );
    println!(
        "first damage: {}  attack events: {}  death events: {}  replay: {}",
        tick_text(result.first_damage_tick),
        result.attack_events,
        result.death_events,
        if result.replay_verified {
            "verified"
        } else {
            "skipped"
        }
    );
    if let Some(path) = &result.replay_artifact {
        println!("replay artifact: {path}");
    }
    println!();
    println!(
        "{:<8} {:<32} {:<6} {:>6} {:>6} {:>4} {:>6} {:>6} {:>6} {:>5} {:>9} {:>9} {:>9} {:>9} {:>9}  final counts",
        "player",
        "profile",
        "alive",
        "army",
        "bldg",
        "wrk",
        "cmds",
        "atk",
        "dmg",
        "lost",
        "firstAtk",
        "rifleAtk",
        "scout",
        "expand",
        "firstTank"
    );
    for player in &result.players {
        println!(
            "{:<8} {:<32} {:<6} {:>6} {:>6} {:>4} {:>6} {:>6} {:>6} {:>5} {:>9} {:>9} {:>9} {:>9} {:>9}  {}",
            player.player_id,
            player.profile,
            player.alive,
            player.army_value,
            player.building_value,
            player.worker_count,
            player.command_count,
            player.attack_command_count,
            player.damage_dealt_events,
            player.death_count,
            tick_text(player.first_attack_command_tick),
            tick_text(player.first_rifleman_attack_command_tick),
            tick_text(player.first_scout_car_tick),
            planned_completed_tick_text(
                player.first_expansion_city_centre_planned_tick,
                player.first_expansion_city_centre_completed_tick
            ),
            tick_text(player.first_tank_tick),
            format_counts(&player.final_counts)
        );
    }
}

fn winner_text(result: &ProfileMatchupResult) -> String {
    if let Some(winner) = &result.winner {
        return format!("{} won as player {}", winner.profile, winner.player_id);
    }
    if result.completed_by_elimination {
        "no players alive".to_string()
    } else {
        "no winner at tick cap".to_string()
    }
}

fn tick_text(tick: Option<u32>) -> String {
    tick.map(|tick| tick.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn planned_completed_tick_text(planned: Option<u32>, completed: Option<u32>) -> String {
    match (planned, completed) {
        (Some(planned), Some(completed)) => format!("{planned}/{completed}"),
        (Some(planned), None) => format!("{planned}/-"),
        (None, Some(completed)) => format!("-/{completed}"),
        (None, None) => "-".to_string(),
    }
}

fn format_counts(counts: &std::collections::BTreeMap<String, u32>) -> String {
    if counts.is_empty() {
        return "-".to_string();
    }
    counts
        .iter()
        .map(|(kind, count)| format!("{kind}={count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn print_profiles() {
    println!("profiles:");
    for profile in available_profile_ids() {
        println!("  {profile}");
    }
    println!();
    println!("aliases:");
    println!("  ai -> ai_1_1_tank_mg");
    println!("  ai1 -> ai_1_0_tech");
    println!("  ai_1_0 -> ai_1_0_tech");
    println!("  default -> ai_1_1_tank_mg");
    println!("  ai_1_1 -> ai_1_1_tank_mg");
    println!("  ai11 -> ai_1_1_tank_mg");
}

fn print_usage() {
    println!(
        "Usage:
  cargo run --bin ai-matchup -- <profile-a> <profile-b> [options]
  cargo run --bin ai-matchup -- --profile-a <id> --profile-b <id> [options]

Options:
  --seed <u32>           Match seed (default: {DEFAULT_SEED})
  --ticks <u32>          Tick cap (default: {DEFAULT_TICKS})
  --format table|json    Output format (default: table)
  --json                 Shortcut for --format json
  --save-replay <name>   Write target/selfplay-artifacts/<name>/replay.json
  --replay-dir <path>    Parent directory for --save-replay artifacts
  --no-verify-replay     Skip deterministic command-log replay verification
  --list-profiles        Print available profiles and aliases
  -h, --help             Print this help

Examples:
  cargo run --bin ai-matchup -- ai ai
  cargo run --bin ai-matchup -- ai_1_1 ai_1_0_tech --seed 7 --ticks 3000 --json
  cargo run --bin ai-matchup -- default ai_1_0_tech --seed 7 --ticks 20000 --json
"
    );
}
