use crate::game::{checkpoint, systems, Game, Map, MapMetadata};

struct CheckpointStartComposition {
    map: Map,
    map_metadata: MapMetadata,
    checkpoint_payload_text: String,
}

impl CheckpointStartComposition {
    fn from_game(game: &Game) -> Result<Self, checkpoint::CheckpointPayloadError> {
        Ok(Self {
            map: game.state.map.clone(),
            map_metadata: game.map_metadata().clone(),
            checkpoint_payload_text: game.checkpoint_payload_text()?,
        })
    }

    fn restore(self) -> Result<Game, checkpoint::CheckpointPayloadError> {
        Game::restore_checkpoint_payload_text(
            &self.checkpoint_payload_text,
            self.map,
            self.map_metadata,
        )
    }
}

impl Game {
    pub(in crate::game) fn checkpoint_backed_start_from_direct_for_setup(
        mut direct: Game,
        _label: &str,
    ) -> Result<Game, checkpoint::CheckpointPayloadError> {
        direct.repair_start_state_for_checkpoint();
        CheckpointStartComposition::from_game(&direct)?.restore()
    }

    pub(in crate::game) fn checkpoint_backed_start_from_direct(direct: Game, label: &str) -> Game {
        Self::checkpoint_backed_start_from_direct_for_setup(direct, label)
            .unwrap_or_else(|err| panic!("failed to build checkpoint-backed {label} start: {err}"))
    }

    fn repair_start_state_for_checkpoint(&mut self) {
        self.state.entities.clear_stale_miner_slots();
        systems::recompute_supply(&mut self.state.players, &self.state.entities);
        self.reset_derived_state();
        let ids = self.state.player_ids();
        self.recompute_live_fog(&ids);
        self.refresh_fog_memories(&ids);
        #[cfg(debug_assertions)]
        self.assert_invariants();
    }
}
