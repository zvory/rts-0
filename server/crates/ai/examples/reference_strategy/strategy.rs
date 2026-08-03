use rts_ai::sdk::{
    AiActionRequest, AiActions, AiFrame, AiRulebook, AiStrategy, EntityKind, WorldQueries,
};

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
        let Some(rules) = AiRulebook::for_frame(frame) else {
            return;
        };
        let queries = WorldQueries::new(frame);

        let workers = frame
            .owned()
            .iter()
            .filter(|entity| entity.kind == EntityKind::Worker)
            .map(|entity| entity.id)
            .collect::<Vec<_>>();

        let expansion_steel = rules
            .cost(EntityKind::CityCentre)
            .map(|cost| cost.steel)
            .unwrap_or(u32::MAX);
        if rules.can_gather(EntityKind::Worker) && frame.economy().steel < expansion_steel {
            if let (Some(&worker), Some(node)) = (
                workers.first(),
                queries.known_resources(EntityKind::Steel).next(),
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
