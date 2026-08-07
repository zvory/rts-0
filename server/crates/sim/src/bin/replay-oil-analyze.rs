use std::env;
use std::fs;

use rts_sim::game::replay::{analyze_vehicle_movement_oil, ReplayArtifactV1};
use rts_sim::game::PlayerInit;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplayIdentity<'a> {
    server_build_sha: &'a str,
    map_name: &'a str,
    duration_ticks: u32,
    players: &'a [PlayerInit],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalysisOutput<'a> {
    match_id: i64,
    analysis_build_sha: &'a str,
    replay: ReplayIdentity<'a>,
    vehicles: Vec<rts_sim::game::replay::VehicleOilRecord>,
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let artifact_path = args.next().ok_or_else(|| {
        "usage: replay-oil-analyze ARTIFACT_JSON MATCH_ID ANALYSIS_BUILD_SHA".to_string()
    })?;
    let match_id = args
        .next()
        .ok_or_else(|| "missing MATCH_ID".to_string())?
        .parse::<i64>()
        .map_err(|err| format!("invalid MATCH_ID: {err}"))?;
    let analysis_build_sha = args
        .next()
        .ok_or_else(|| "missing ANALYSIS_BUILD_SHA".to_string())?;
    if args.next().is_some() {
        return Err("unexpected extra arguments".to_string());
    }

    let text = fs::read_to_string(&artifact_path)
        .map_err(|err| format!("cannot read {artifact_path}: {err}"))?;
    let replay: ReplayArtifactV1 = serde_json::from_str(&text)
        .map_err(|err| format!("cannot decode {artifact_path}: {err}"))?;
    if analysis_build_sha != replay.server_build_sha {
        return Err(format!(
            "analysis build SHA {analysis_build_sha} does not match replay build SHA {}",
            replay.server_build_sha
        ));
    }
    let vehicles = analyze_vehicle_movement_oil(&replay)?;
    let output = AnalysisOutput {
        match_id,
        analysis_build_sha: &analysis_build_sha,
        replay: ReplayIdentity {
            server_build_sha: &replay.server_build_sha,
            map_name: &replay.map_name,
            duration_ticks: replay.duration_ticks,
            players: &replay.players,
        },
        vehicles,
    };
    println!(
        "{}",
        serde_json::to_string(&output).map_err(|err| err.to_string())?
    );
    Ok(())
}
