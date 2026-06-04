use super::player_view::PlayerView;
use super::scripts::{ProfileBackedScript, ScriptedPlayer};
use crate::game::ai_core::profiles::TECH_TO_TANKS_ID;
use crate::game::{Game, PlayerInit};

pub(crate) struct LiveSelfPlay {
    players: Vec<PlayerInit>,
    scripts: Vec<Box<dyn ScriptedPlayer>>,
}

impl LiveSelfPlay {
    pub(crate) fn default_match() -> Self {
        let players = vec![
            PlayerInit {
                id: 1,
                name: "Alpha Script".to_string(),
                color: "#6f8fa8".to_string(),
                is_ai: true,
            },
            PlayerInit {
                id: 2,
                name: "Bravo Script".to_string(),
                color: "#b2775f".to_string(),
                is_ai: true,
            },
        ];
        let scripts: Vec<Box<dyn ScriptedPlayer>> = vec![
            Box::new(ProfileBackedScript::new(players[0].id, TECH_TO_TANKS_ID)),
            Box::new(ProfileBackedScript::new(players[1].id, TECH_TO_TANKS_ID)),
        ];
        Self { players, scripts }
    }

    pub(crate) fn players(&self) -> &[PlayerInit] {
        &self.players
    }

    pub(crate) fn enqueue_for_tick(&mut self, game: &mut Game) {
        let tick = game.tick_count();
        let start = game.start_payload();
        let mut commands = Vec::new();
        for script in &mut self.scripts {
            let player_id = script.player_id();
            let snapshot = game.snapshot_for(player_id);
            let view = PlayerView {
                player_id,
                tick,
                start: &start,
                snapshot: &snapshot,
            };
            for command in script.commands(view) {
                commands.push((player_id, command));
            }
        }
        for (player_id, command) in commands {
            game.enqueue(player_id, command);
        }
    }
}
