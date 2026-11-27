//! Exercise the public planner from an unrelated external strategy, through the normal host.
use std::sync::{Arc, Mutex};

use rts_ai::sdk::{
    AiActions, AiFrame, AiStrategy, DefensePlan, DefensePlanner, WorldQueries,
};
use rts_ai::{AiAlivePolicy, AiController, CanonicalAiTickDriver};
use rts_sim::game::{Game, PlayerInit};

#[derive(Default)]
struct Evidence {
    plans: Vec<(usize, DefensePlan)>,
    flows_after_first_request: u64,
    flows_after_budget_changes: u64,
}

struct IndependentDefense {
    planner: DefensePlanner,
    evidence: Arc<Mutex<Evidence>>,
}

impl AiStrategy for IndependentDefense {
    fn step(&mut self, frame: &AiFrame, actions: &mut AiActions) {
        let mut evidence = self.evidence.lock().unwrap();
        if !evidence.plans.is_empty() {
            return;
        }
        let main = WorldQueries::new(frame)
            .base(frame.player_id(), 1)
            .expect("canonical host attaches public base geography")
            .site
            .id;
        for budget in [0, 1, 12] {
            let plan = self.planner.plan(frame, &[main], budget);
            if budget == 0 {
                evidence.flows_after_first_request = self.planner.geometry_builds;
            }
            evidence.plans.push((budget, plan));
        }
        evidence.flows_after_budget_changes = self.planner.geometry_builds;
        assert!(actions.is_empty(), "planning alone issues no actions");
    }
}

#[test]
fn external_strategy_reuses_defense_without_adopting_eco_or_emitting_orders() {
    let players = [1, 2].map(|id| PlayerInit {
        id,
        team_id: id,
        faction_id: "kriegsia".into(),
        name: format!("Player {id}"),
        color: "#4cc9f0".into(),
        is_ai: true,
    });
    let map = rts_sim::game::map::Map::load_for_players("Classic", &[(1, 1), (2, 2)], 7)
        .unwrap();
    let mut game = Game::new_with_random_ai_profiles_and_map_metadata(
        &players,
        7,
        map,
        rts_sim::game::map::Map::metadata_for_name("Classic").unwrap(),
    );
    let evidence = Arc::new(Mutex::new(Evidence::default()));
    let strategy = IndependentDefense {
        planner: DefensePlanner::with_firing_range_tiles(4.8),
        evidence: evidence.clone(),
    };
    let mut controllers = [AiController::with_strategy(1, Box::new(strategy))];
    for _ in 0..18 {
        CanonicalAiTickDriver::run(
            &mut game,
            &mut controllers,
            AiAlivePolicy::StartingPrimaryBase,
        );
        game.tick();
    }
    assert_eq!(controllers[0].profile_id(), "custom_strategy");
    assert!(game.command_log().is_empty());
    let evidence = evidence.lock().unwrap();
    assert_eq!(evidence.plans.len(), 3);
    assert!(evidence.flows_after_first_request > 0);
    assert_eq!(
        evidence.flows_after_first_request,
        evidence.flows_after_budget_changes,
        "a changing force budget must reuse the static cut work"
    );
    for (budget, plan) in &evidence.plans {
        assert_eq!(plan.slots.len() + plan.unallocated_budget, *budget);
        assert_eq!(
            plan.sectors.iter().map(|s| s.slots.len()).sum::<usize>(),
            plan.slots.len()
        );
        assert!(!plan.sectors.is_empty());
        assert!((plan.sectors.iter().map(|s| s.priority).sum::<f32>() - 1.0).abs() < 0.0001);
        if *budget > 0 {
            assert!(!plan.slots.is_empty());
        }
        for slot in &plan.slots {
            assert!(slot.position.0.is_finite() && slot.position.1.is_finite());
            assert!(plan.sectors.iter().any(|sector| sector.id == slot.sector_id));
        }
    }
}
