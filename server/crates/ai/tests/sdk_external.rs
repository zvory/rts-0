use std::sync::{Arc, Mutex};

use rts_ai::sdk::{AiActionRequest, AiActions, AiFrame, AiStrategy};
use rts_ai::{AiAlivePolicy, AiController, CanonicalAiTickDriver};
use rts_sim::game::{Game, PlayerInit};

#[derive(Default)]
struct LifecycleLog {
    initialized_at: Vec<u32>,
    stepped_at: Vec<u32>,
}

struct RecordingStrategy {
    log: Arc<Mutex<LifecycleLog>>,
}

impl AiStrategy for RecordingStrategy {
    fn initialize(&mut self, frame: &AiFrame) {
        self.log.lock().unwrap().initialized_at.push(frame.tick());
    }

    fn step(&mut self, frame: &AiFrame, actions: &mut AiActions) {
        self.log.lock().unwrap().stepped_at.push(frame.tick());
        let units = frame
            .owned()
            .iter()
            .take(1)
            .map(|entity| entity.id)
            .collect();
        actions.submit(AiActionRequest::Move {
            units,
            x: 32.0,
            y: 32.0,
            queued: false,
        });
    }
}

fn players() -> Vec<PlayerInit> {
    vec![
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Custom".to_string(),
            color: "#111111".to_string(),
            is_ai: true,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Opponent".to_string(),
            color: "#222222".to_string(),
            is_ai: true,
        },
    ]
}

#[test]
fn public_strategy_runs_once_initialized_on_canonical_cadence() {
    let log = Arc::new(Mutex::new(LifecycleLog::default()));
    let strategy: Box<dyn AiStrategy> = Box::new(RecordingStrategy { log: log.clone() });
    let mut controllers = vec![AiController::with_strategy(1, strategy)];
    let mut game = Game::new_without_ai_controllers(&players(), 31);

    for _ in 0..20 {
        CanonicalAiTickDriver::run(
            &mut game,
            &mut controllers,
            AiAlivePolicy::StartingPrimaryBase,
        );
        game.tick();
    }

    let log = log.lock().unwrap();
    assert_eq!(log.initialized_at, vec![8]);
    assert_eq!(log.stepped_at, vec![8, 17]);
    assert_eq!(controllers[0].profile_id(), "custom_strategy");
    assert_eq!(game.command_log().len(), 2);
}
