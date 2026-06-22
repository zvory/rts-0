use super::super::milestones::{CombatGoal, Milestones, PlayerMilestoneGoal};
use super::super::scripts::{ProfileBackedScript, ScriptedPlayer};
use super::harness::{finalize_self_play_success, replay_artifact_url, SelfPlayRunner};
use crate::ai_core::profiles::AI_1_0_TECH_ID;
use crate::config;
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::kinds;

struct MatchupPlayerSpec {
    id: u32,
    name: &'static str,
    color: &'static str,
    profile_id: &'static str,
    goal: PlayerMilestoneGoal,
}

struct MatchupConfig {
    artifact_name: &'static str,
    seed: u32,
    max_ticks: u32,
    players: [MatchupPlayerSpec; 2],
    combat_goal: CombatGoal,
    assert_outcome: fn(&Milestones),
}

fn run_profile_matchup(config: MatchupConfig) {
    let players: Vec<PlayerInit> = config
        .players
        .iter()
        .map(|player| PlayerInit {
            id: player.id,
            team_id: player.id,
            faction_id: "kriegsia".to_string(),
            name: player.name.to_string(),
            color: player.color.to_string(),
            is_ai: true,
        })
        .collect();
    let game = Game::new_without_ai_controllers(&players, config.seed);
    let start = game.start_payload();
    let specs = players.clone();
    let scripts: Vec<Box<dyn ScriptedPlayer>> = config
        .players
        .iter()
        .map(|player| {
            Box::new(ProfileBackedScript::new(player.id, player.profile_id))
                as Box<dyn ScriptedPlayer>
        })
        .collect();
    let milestones = Milestones::with_goals(
        config
            .players
            .iter()
            .map(|player| (player.id, player.goal.clone())),
        config.combat_goal,
    );
    let mut runner = SelfPlayRunner::with_options(
        config.artifact_name,
        config.max_ticks,
        game,
        start,
        specs,
        scripts,
        milestones,
    );

    match runner.run() {
        Ok(report) => {
            (config.assert_outcome)(&runner.milestones);
            finalize_self_play_success(&runner, &players, &report);
        }
        Err(failure) => {
            let artifact = runner
                .write_failure_artifact(&failure)
                .map(|p| {
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.display().to_string());
                    replay_artifact_url(&name)
                })
                .unwrap_or_else(|e| format!("artifact write failed: {e}"));
            panic!("matchup failed: {}; REPLAY={artifact}", failure.reason);
        }
    }
}

fn ai_1_0_tech_goal() -> PlayerMilestoneGoal {
    PlayerMilestoneGoal {
        require_gathering: true,
        require_oil: true,
        require_oil_worker_assignment: true,
        require_depot_supply: true,
        require_barracks_complete: true,
        require_rifleman: true,
        require_tank: true,
        ..PlayerMilestoneGoal::default()
    }
    .with_min_workers(12)
    .with_min_supply_cap(config::CITY_CENTRE_SUPPLY + config::DEPOT_SUPPLY)
    .with_min_buildings(kinds::TRAINING_CENTRE, 1)
    .with_min_buildings(kinds::RESEARCH_COMPLEX, 1)
    .with_min_buildings(kinds::FACTORY, 1)
    .with_min_buildings(kinds::CITY_CENTRE, 2)
    .with_min_units(kinds::RIFLEMAN, 4)
    .with_min_units(kinds::SCOUT_CAR, 1)
    .with_min_units(kinds::TANK, 1)
}

#[test]
fn profile_backed_self_play_exercises_ai_1_0_tech_arc() {
    if crate::skip_unless_full_ai("profile_backed_self_play_exercises_ai_1_0_tech_arc") {
        return;
    }

    run_profile_matchup(MatchupConfig {
        artifact_name: "profile_backed_self_play_exercises_ai_1_0_tech_arc",
        seed: 0x4100_0004,
        max_ticks: 14_000,
        players: [
            MatchupPlayerSpec {
                id: 1,
                name: "AI 1.0 Tech",
                color: "#4cc9f0",
                profile_id: AI_1_0_TECH_ID,
                goal: ai_1_0_tech_goal(),
            },
            MatchupPlayerSpec {
                id: 2,
                name: "AI 1.0 Mirror",
                color: "#f72585",
                profile_id: AI_1_0_TECH_ID,
                goal: ai_1_0_tech_goal().allowing_elimination_before_milestones(),
            },
        ],
        combat_goal: CombatGoal::damage(),
        assert_outcome: |_| {},
    });
}
