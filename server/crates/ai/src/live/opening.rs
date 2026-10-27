//! Match-local production milestones, retained across the Rush-to-Pressure handoff.
use rts_sim::protocol::Snapshot;
use std::collections::BTreeSet;

#[derive(Default)]
pub(super) struct OpeningProgress {
    initialized: bool,
    seen_riflemen: BTreeSet<u32>,
    trained_riflemen: u32,
}

impl OpeningProgress {
    pub(super) fn observe(&mut self, snapshot: &Snapshot, player: u32) {
        // The controller observes its own snapshot every tick. Initial units are a baseline,
        // not production; keep ids after deaths so casualties never undo the milestone.
        for entity in &snapshot.entities {
            if entity.owner == player
                && entity.kind == "rifleman"
                && entity.hp > 0
                && self.seen_riflemen.insert(entity.id)
                && self.initialized
            {
                self.trained_riflemen = self.trained_riflemen.saturating_add(1);
            }
        }
        self.initialized = true;
    }

    pub(super) fn trained_riflemen(&self) -> u32 {
        self.trained_riflemen
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_sim::{
        game::{Game, PlayerInit},
        protocol::EntityView,
    };

    #[test]
    fn production_excludes_starting_and_enemy_rifles_and_survives_losses() {
        let players = [1, 2].map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: "kriegsia".into(),
            name: "Test".into(),
            color: "#ffffff".into(),
            is_ai: true,
        });
        let game = Game::new_without_ai_controllers(&players, 7);
        let mut snapshot = game.snapshot_for(1);
        let mut progress = OpeningProgress::default();
        progress.observe(&snapshot, 1);
        assert_eq!(progress.trained_riflemen(), 0);
        snapshot.entities.push(EntityView::new(
            900, 1, "rifleman", 500.0, 500.0, 45, 45, "idle",
        ));
        snapshot.entities.push(EntityView::new(
            901, 2, "rifleman", 500.0, 500.0, 45, 45, "idle",
        ));
        progress.observe(&snapshot, 1);
        progress.observe(&snapshot, 1);
        assert_eq!(progress.trained_riflemen(), 1);
        snapshot.entities.retain(|e| e.id != 900);
        progress.observe(&snapshot, 1);
        assert_eq!(progress.trained_riflemen(), 1);
        snapshot.entities.push(EntityView::new(
            902, 1, "rifleman", 500.0, 500.0, 45, 45, "idle",
        ));
        progress.observe(&snapshot, 1);
        assert_eq!(progress.trained_riflemen(), 2);
    }
}
