use rts_ai::sdk::{AiActions, AiFrame, AiStrategy, EntityKind, UnitGroup, WorldQueries};

/// A deliberately small opening that demonstrates the public SDK lifecycle.
///
/// The strategy enables Depot-built extractors and sends its Engineer to scout the first enemy
/// start. It is a usage specimen, not a competitive profile.
#[derive(Default)]
pub struct ReferenceStrategy {
    enemy_start: Option<(f32, f32)>,
    scout_dispatched: bool,
    steps: u32,
}

impl AiStrategy for ReferenceStrategy {
    fn initialize(&mut self, frame: &AiFrame) {
        let tile_size = frame.map().tile_size as f32;
        self.enemy_start = frame
            .players()
            .iter()
            .find(|player| player.team_id != frame.team_id())
            .map(|player| {
                (
                    (player.start_tile.0 as f32 + 0.5) * tile_size,
                    (player.start_tile.1 as f32 + 0.5) * tile_size,
                )
            });
    }

    fn step(&mut self, frame: &AiFrame, actions: &mut AiActions) {
        self.steps = self.steps.saturating_add(1);
        let queries = WorldQueries::new(frame);

        let workers = frame
            .owned()
            .iter()
            .filter(|entity| entity.kind == EntityKind::Worker)
            .map(|entity| entity.id)
            .collect::<Vec<_>>();

        let depots = frame
            .owned()
            .iter()
            .filter(|entity| entity.kind == EntityKind::ResourceDepot)
            .map(|entity| entity.id)
            .collect::<Vec<_>>();
        if self.steps == 1 && queries.known_resources(EntityKind::Oil).next().is_some() {
            let _ = actions.set_production_repeat(&depots, EntityKind::PumpJack, true);
        }

        // Wait for a second decision so the example visibly uses cross-tick state.
        if !self.scout_dispatched && self.steps >= 2 {
            if let (Some(&scout), Some((x, y))) = (workers.first(), self.enemy_start) {
                if let Ok(group) = UnitGroup::new([scout]) {
                    if actions.attack_move(&group, x, y, false).is_ok() {
                        self.scout_dispatched = true;
                    }
                }
            }
        }
    }
}
