use std::sync::{Arc, Mutex};

use rts_ai::sdk::{AiActions, AiFrame, AiRulebook, AiStrategy, EntityKind, WorldQueries};
use rts_ai::{AiAlivePolicy, AiController, CanonicalAiTickDriver};
use rts_sim::game::replay::{replay_commands, CommandLogEntry};
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::{self, Snapshot};

#[path = "../examples/reference_strategy/strategy.rs"]
mod reference_strategy;

use reference_strategy::ReferenceStrategy;

const SEED: u32 = 0xA15D_0004;
const TICKS: u32 = 36;

#[derive(Default)]
struct LifecycleLog {
    initialized_at: Vec<u32>,
    stepped_at: Vec<u32>,
}

struct LifecycleProbe<S> {
    inner: S,
    log: Arc<Mutex<LifecycleLog>>,
}

impl<S: AiStrategy> AiStrategy for LifecycleProbe<S> {
    fn initialize(&mut self, frame: &AiFrame) {
        let rules = AiRulebook::for_frame(frame).expect("reference faction should have a rulebook");
        assert!(!rules.can_gather(EntityKind::Worker));
        assert_eq!(
            rules.entity(EntityKind::ResourceDepot).unwrap().builders,
            vec![EntityKind::Worker]
        );
        let queries = WorldQueries::new(frame);
        assert!(queries.known_resources(EntityKind::Steel).next().is_some());
        self.log.lock().unwrap().initialized_at.push(frame.tick());
        self.inner.initialize(frame);
    }

    fn step(&mut self, frame: &AiFrame, actions: &mut AiActions) {
        self.log.lock().unwrap().stepped_at.push(frame.tick());
        self.inner.step(frame, actions);
    }
}

struct ReferenceRun {
    commands: Vec<CommandLogEntry>,
    final_snapshot: Snapshot,
    starting_loadouts: Vec<rts_sim::game::PlayerStartingLoadout>,
    lifecycle: Arc<Mutex<LifecycleLog>>,
}

#[test]
fn reference_strategy_is_deterministic_and_replayable_through_canonical_runtime() {
    let first = run_reference_match();
    let second = run_reference_match();

    assert_eq!(first.commands, second.commands);
    assert_eq!(first.final_snapshot, second.final_snapshot);

    let lifecycle = first.lifecycle.lock().unwrap();
    assert_eq!(lifecycle.initialized_at, vec![8]);
    assert_eq!(lifecycle.stepped_at, vec![8, 17, 26, 35]);
    drop(lifecycle);

    let extractor_repeat = first
        .commands
        .iter()
        .find_map(|entry| match &entry.command {
            protocol::Command::AdjustProductionRepeat {
                buildings,
                unit,
                delta,
            } if unit == "pump_jack" && *delta == 1 => Some(buildings[0]),
            _ => None,
        })
        .expect("reference strategy should enable Depot extractor production");
    let scout = first
        .commands
        .iter()
        .find_map(|entry| match &entry.command {
            protocol::Command::AttackMove { units, .. } => Some(units[0]),
            _ => None,
        })
        .expect("reference strategy should issue a tactical attack-move action");
    let resource_depot = first
        .final_snapshot
        .entities
        .iter()
        .find(|entity| entity.id == extractor_repeat)
        .expect("Resource Depot should remain owner-visible");
    assert_eq!(resource_depot.kind, "resource_depot");
    let scouting_worker = first
        .final_snapshot
        .entities
        .iter()
        .find(|entity| entity.id == scout)
        .expect("scout should remain owner-visible");
    assert_ne!(scouting_worker.state, protocol::states::IDLE);

    let replay = replay_commands(
        &players(),
        &first.commands,
        TICKS,
        SEED,
        &first.starting_loadouts,
    )
    .expect("ordinary replay should accept the canonical command log");
    let replay_snapshot = replay
        .final_snapshots
        .iter()
        .find(|snapshot| snapshot.player_id == 1)
        .expect("replay should include the strategy player's view");
    assert_eq!(replay_snapshot.snapshot, first.final_snapshot);
}

fn run_reference_match() -> ReferenceRun {
    let lifecycle = Arc::new(Mutex::new(LifecycleLog::default()));
    let strategy: Box<dyn AiStrategy> = Box::new(LifecycleProbe {
        inner: ReferenceStrategy::default(),
        log: lifecycle.clone(),
    });
    let mut controllers = vec![AiController::with_strategy(1, strategy)];
    let mut game = Game::new_without_ai_controllers(&players(), SEED);

    for _ in 0..TICKS {
        CanonicalAiTickDriver::run(
            &mut game,
            &mut controllers,
            AiAlivePolicy::StartingPrimaryBase,
        );
        game.tick();
    }

    assert_eq!(controllers[0].profile_id(), "custom_strategy");
    ReferenceRun {
        commands: game.command_log().to_vec(),
        final_snapshot: game.snapshot_for(1),
        starting_loadouts: game.starting_loadouts().to_vec(),
        lifecycle,
    }
}

fn players() -> Vec<PlayerInit> {
    vec![
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "SDK reference".to_string(),
            color: "#4cc9f0".to_string(),
            is_ai: true,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Passive opponent".to_string(),
            color: "#f72585".to_string(),
            is_ai: true,
        },
    ]
}
