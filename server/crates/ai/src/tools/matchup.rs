//! Command-line profile matchup runner.
//!
//! This is a developer/test tool. It runs one directed self-play profile matchup until one
//! starting City Centre dies or a fixed tick cap is reached, then prints the result.
#![allow(dead_code)]

use std::path::PathBuf;
use std::process;

use crate::selfplay::{
    available_profile_request_ids, canonical_profile_request_id_for_match,
    resolve_profile_request_id_for_match, run_profile_matchup_result, ProfileMatchupEndReason,
    ProfileMatchupOptions, ProfileMatchupResult,
};

const DEFAULT_TICKS: u32 = 25_000;
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
    let profile_a = resolve_profile_request_id_for_match(&config.profile_a, config.seed, 0)
        .unwrap_or(&config.profile_a)
        .to_string();
    let profile_b = resolve_profile_request_id_for_match(&config.profile_b, config.seed, 1)
        .unwrap_or(&config.profile_b)
        .to_string();

    let result = run_profile_matchup_result(ProfileMatchupOptions {
        profile_a,
        profile_b,
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
    canonical_profile_request_id_for_match(value)
        .map(str::to_string)
        .ok_or_else(|| {
            format!(
                "unknown profile or suite {value:?}; known requests: {}",
                available_profile_request_ids().join(", ")
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
    println!(
        "starting City Centres: {}",
        starting_city_centre_text(&result.starting_city_centres)
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
        return format!(
            "{} won by killing the enemy starting City Centre as player {}",
            winner.profile, winner.player_id
        );
    }
    match result.end_reason {
        ProfileMatchupEndReason::TickCap => "draw at tick cap".to_string(),
        ProfileMatchupEndReason::StartingCityCentresDestroyed => {
            "draw: both starting City Centres were destroyed".to_string()
        }
        ProfileMatchupEndReason::StartingCityCentreKilled => {
            "draw: starting City Centre destroyed without a surviving objective winner".to_string()
        }
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

fn starting_city_centre_text(
    centres: &[crate::selfplay::ProfileMatchupStartingCityCentreResult],
) -> String {
    if centres.is_empty() {
        return "-".to_string();
    }
    centres
        .iter()
        .map(|centre| {
            format!(
                "player {} `{}` id={} death={}",
                centre.player_id,
                centre.profile,
                centre.entity_id,
                tick_text(centre.death_tick)
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
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
    println!("profile and suite requests:");
    for profile in available_profile_request_ids() {
        println!("  {profile}");
    }
    println!();
    println!("aliases:");
    println!("  ai -> ai_1_2");
    println!("  ai1 -> ai_1_0");
    println!("  ai_1_0 -> ai_1_0");
    println!("  default -> ai_1_2");
    println!("  ai_1_1 -> ai_1_1");
    println!("  ai11 -> ai_1_1");
    println!("  ai_1_2 -> ai_1_2");
    println!("  ai12 -> ai_1_2");
    println!("  ai_2_0 -> ai_2_0");
    println!("  ai20 -> ai_2_0");
    println!("  ai_turtle -> ai_turtle");
    println!("  turtle -> ai_turtle");
}

fn print_usage() {
    println!(
        "Usage:
  cargo run --bin ai-matchup -- <profile-or-suite-a> <profile-or-suite-b> [options]
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
  cargo run --bin ai-matchup -- ai_1_2 ai_1_1 --seed 7 --ticks 3000 --json
  cargo run --bin ai-matchup -- ai_2_0 ai --seed 7 --ticks 25000 --json
  cargo run --bin ai-matchup -- default ai_1_0_tech --seed 7 --ticks 25000 --json
"
    );
}

#[cfg(test)]
mod tests {
    use super::{parse_args, winner_text, DEFAULT_TICKS};
    use crate::selfplay::{
        ProfileMatchupEndReason, ProfileMatchupPlayerResult, ProfileMatchupResult,
        ProfileMatchupStartingCityCentreResult, ProfileMatchupWinner,
    };
    use std::collections::BTreeMap;

    #[test]
    fn default_tick_cap_is_twenty_five_thousand() {
        let config = parse_args(vec!["ai_1_0_tech".to_string(), "ai_1_1".to_string()])
            .expect("default args should parse")
            .expect("default args should return config");

        assert_eq!(DEFAULT_TICKS, 25_000);
        assert_eq!(config.ticks, 25_000);
    }

    #[test]
    fn tick_cap_result_is_reported_as_draw() {
        let result = profile_result(ProfileMatchupEndReason::TickCap, None);

        assert_eq!(winner_text(&result), "draw at tick cap");
    }

    #[test]
    fn starting_city_centre_result_is_reported_as_objective_win() {
        let result = profile_result(ProfileMatchupEndReason::StartingCityCentreKilled, Some(2));

        assert_eq!(
            winner_text(&result),
            "right won by killing the enemy starting City Centre as player 2"
        );
    }

    fn profile_result(
        end_reason: ProfileMatchupEndReason,
        winner_player_id: Option<u32>,
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
            players: vec![player_result(1), player_result(2)],
            first_damage_tick: None,
            attack_events: 0,
            death_events: 0,
            event_count: 0,
            replay_verified: false,
            replay_artifact: None,
            ai_trace_tail: Vec::new(),
        }
    }

    fn player_result(player_id: u32) -> ProfileMatchupPlayerResult {
        ProfileMatchupPlayerResult {
            player_id,
            profile: profile_for_player(player_id).to_string(),
            alive: true,
            army_value: 0,
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
