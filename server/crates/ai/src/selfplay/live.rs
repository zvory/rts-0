use crate::ai_core::profiles::{AI_2_1_ID, AI_TURTLE_ID};
use crate::live::{AiAlivePolicy, AiController, CanonicalAiTickDriver};
use rts_sim::game::{Game, PlayerInit};

pub struct LiveSelfPlay {
    players: Vec<PlayerInit>,
    controllers: Vec<AiController>,
}

impl LiveSelfPlay {
    pub fn default_match() -> Self {
        let players = vec![
            PlayerInit {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".to_string(),
                name: "Alpha Script".to_string(),
                color: "#6f8fa8".to_string(),
                is_ai: true,
            },
            PlayerInit {
                id: 2,
                team_id: 2,
                faction_id: "kriegsia".to_string(),
                name: "Bravo Script".to_string(),
                color: "#b2775f".to_string(),
                is_ai: true,
            },
        ];
        let controllers = vec![
            AiController::with_profile_id(players[0].id, AI_2_1_ID),
            AiController::with_profile_id(players[1].id, AI_TURTLE_ID),
        ];
        Self {
            players,
            controllers,
        }
    }

    pub fn players(&self) -> &[PlayerInit] {
        &self.players
    }

    pub fn enqueue_for_tick(&mut self, game: &mut Game) {
        CanonicalAiTickDriver::run(
            game,
            &mut self.controllers,
            AiAlivePolicy::StartingPrimaryBase,
        );
    }
}
