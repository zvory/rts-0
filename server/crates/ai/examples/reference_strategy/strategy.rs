use rts_ai::sdk::{AiActionRequest, AiActions, AiFrame, AiResourceAmount, AiStrategy, EntityKind};

/// A deliberately small opening that demonstrates the public SDK lifecycle.
///
/// The strategy keeps one worker gathering steel and sends a different worker to scout the first
/// enemy start. It is a usage specimen, not a competitive profile.
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

        let workers = frame
            .owned()
            .iter()
            .filter(|entity| entity.kind == EntityKind::Worker)
            .map(|entity| entity.id)
            .collect::<Vec<_>>();

        if frame.economy().steel < 500 {
            if let (Some(&worker), Some(node)) = (
                workers.first(),
                frame.resources().iter().find(|resource| {
                    resource.kind == EntityKind::Steel
                        && resource.remaining != AiResourceAmount::Known(0)
                }),
            ) {
                let _ = actions.submit(AiActionRequest::Gather {
                    units: vec![worker],
                    node: node.id,
                    queued: false,
                });
            }
        }

        // Wait for a second decision so the example visibly uses cross-tick state. The stable
        // frame ordering makes the second worker deterministic and keeps it separate from mining.
        if !self.scout_dispatched && self.steps >= 2 {
            if let (Some(&scout), Some((x, y))) = (workers.get(1), self.enemy_start) {
                if actions.submit(AiActionRequest::AttackMove {
                    units: vec![scout],
                    x,
                    y,
                    queued: false,
                }) {
                    self.scout_dispatched = true;
                }
            }
        }
    }
}
