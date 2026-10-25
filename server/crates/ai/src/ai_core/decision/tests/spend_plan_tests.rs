use super::spend_plan::{plan, PlanInput, SpendPlanMemory, StepKind};
use super::*;
use crate::ai_core::profiles::{JEFFS_AI, JEFFS_AI_PRE_SPEND_PLAN};

fn ts() -> f32 {
    config::TILE_SIZE as f32
}

fn extractor(id: u32, kind: EntityKind, tile: (f32, f32), complete: bool) -> AiEntitySummary {
    let mut entity = building_at(id, kind, None, tile.0 * ts(), tile.1 * ts());
    entity.is_complete = complete;
    entity
}

/// The HQ at (8, 8) with twelve Steel patches and three Oil wells around it: six patches mined, one
/// well pumped, nothing being built, and an Engineer home.
fn main_base(steel: u32, oil: u32) -> AiObservation {
    let mut units = vec![
        worker_at(1, AiEntityState::Idle, 9.0 * ts(), 9.0 * ts()),
        building_at(
            2,
            EntityKind::ResourceDepot,
            Some(0),
            8.5 * ts(),
            8.5 * ts(),
        ),
        building(3, EntityKind::Barracks, Some(0)),
        building(4, EntityKind::TrainingCentre, Some(0)),
        building(5, EntityKind::EngineeringComplex, Some(0)),
    ];
    let mut obs = observation(
        AiEconomy {
            steel,
            oil,
            supply_used: 20,
            supply_cap: 80,
        },
        Vec::new(),
    );
    obs.resources.clear();
    for i in 0..12 {
        let tile = (4.5 + (i % 6) as f32, 4.5 + (i / 6) as f32 * 9.0);
        obs.resources.push(resource(
            100 + i,
            EntityKind::Steel,
            tile.0 * ts(),
            tile.1 * ts(),
        ));
        if i < 6 {
            units.push(extractor(300 + i, EntityKind::SteelMine, tile, true));
        }
    }
    for i in 0..3 {
        let tile = (12.5, 6.5 + i as f32 * 2.0);
        obs.resources.push(resource(
            200 + i,
            EntityKind::Oil,
            tile.0 * ts(),
            tile.1 * ts(),
        ));
        if i == 0 {
            units.push(extractor(400, EntityKind::PumpJack, tile, true));
        }
    }
    obs.owned = units;
    obs.tick = 1800;
    obs
}

fn no_natural() -> PlanInput {
    PlanInput {
        natural_due: false,
        natural_site: None,
        natural_builder: None,
        natural_wait: 0,
    }
}

#[test]
fn the_forecast_counts_each_extractor_and_the_free_ones_the_depot_adds_one_at_a_time() {
    let obs = main_base(0, 0);
    let mut memory = SpendPlanMemory::default();
    memory.observe(&obs, false);
    let facts = AiFacts::from_observation(&obs);
    let forecast = plan(&obs, &facts, &memory, no_natural()).expected;
    // 60 s: six mines paying (half a harvest in) plus mines 7 (done at 24 s) and 8 (48 s), each
    // paying from a harvest after it is done; one Pump Jack plus the second (done at 36 s).
    let steel = 6.0 * 0.05 * (1800.0 - 20.0) + 0.05 * (1800.0 - 760.0) + 0.05 * (1800.0 - 1480.0);
    let oil = 0.05 * (1800.0 - 20.0) + 0.05 * (1800.0 - 1120.0);
    let (got_steel, got_oil) = forecast[2];
    assert!((got_steel - steel).abs() < 4.0, "{got_steel} vs {steel}");
    assert!((got_oil - oil).abs() < 2.0, "{got_oil} vs {oil}");
    // 15 s: only the standing extractors pay.
    let (steel15, oil15) = forecast[0];
    assert!((steel15 - 6.0 * 0.05 * 430.0).abs() < 4.0, "{steel15}");
    assert!((oil15 - 0.05 * 430.0).abs() < 2.0, "{oil15}");
}

#[test]
fn an_unfinished_mine_pays_from_its_build_time_after_it_was_first_seen() {
    let mut obs = main_base(0, 0);
    obs.owned
        .push(extractor(306, EntityKind::SteelMine, (4.5, 13.5), false));
    let mut memory = SpendPlanMemory::default();
    // First seen 600 ticks ago: done 120 ticks from now.
    obs.tick = 1200;
    memory.observe(&obs, false);
    obs.tick = 1800;
    memory.observe(&obs, false);
    let facts = AiFacts::from_observation(&obs);
    let (steel15, _) = plan(&obs, &facts, &memory, no_natural()).expected[0];
    let expected = 6.0 * 0.05 * 430.0 + 0.05 * (450.0 - 160.0);
    assert!((steel15 - expected).abs() < 4.0, "{steel15} vs {expected}");
}

#[test]
fn the_naturals_builder_goes_once_the_depot_is_paid_for_by_its_arrival() {
    let site = (40, 8);
    let input = PlanInput {
        natural_due: true,
        natural_site: Some(site),
        natural_builder: Some(1),
        natural_wait: 0,
    };
    // 300 Steel and 80 Oil banked: the Depot (450/100) is 150/20 short. The 31-tile walk takes
    // about 16 s, in which six mines and a Pump Jack bring in about 140 Steel and 23 Oil: short at
    // full income, and shorter on the cautious forecast.
    let obs = main_base(300, 80);
    let mut memory = SpendPlanMemory::default();
    memory.observe(&obs, false);
    let facts = AiFacts::from_observation(&obs);
    assert!(!plan(&obs, &facts, &memory, input).natural_funded_on_arrival);
    // With 360 banked the walk covers the rest: the builder goes now.
    let obs = main_base(360, 80);
    let laid = plan(&obs, &facts, &memory, input);
    assert!(laid.natural_funded_on_arrival);
    // The Depot is charged when the builder arrives, then the Factory once its order went out.
    let natural = laid.charged_at(StepKind::Natural).unwrap();
    assert!(
        natural > obs.tick && natural < obs.tick + 30 * 20,
        "{natural}"
    );
    let factory = laid.charged_at(StepKind::Factory).unwrap();
    assert!(factory > natural, "{factory} vs {natural}");
}

#[test]
fn the_live_jeff_orders_its_natural_ahead_of_the_money_and_the_freeze_waits_for_it() {
    for profile in [&JEFFS_AI, &JEFFS_AI_PRE_SPEND_PLAN] {
        let live = profile.id == JEFFS_AI_ID;
        let mut obs = main_base(400, 95);
        obs.owned.extend(
            (20..24).map(|id| combat_at(id, EntityKind::MachineGunner, 10.0 * ts(), 10.0 * ts())),
        );
        obs.owned.extend(
            (10..16).map(|id| combat_at(id, EntityKind::Rifleman, 14.0 * ts(), 14.0 * ts())),
        );
        obs.upgrades.push(UpgradeKind::Entrenchment);
        for i in 0..6 {
            obs.resources.push(resource(
                500 + i,
                EntityKind::Steel,
                (44.5 + i as f32) * ts(),
                6.5 * ts(),
            ));
        }
        obs.resources
            .push(resource(600, EntityKind::Oil, 47.5 * ts(), 10.5 * ts()));
        let mut memory = AiDecisionMemory::for_profile(profile);
        expansion_security::prepare(
            &obs,
            &AiFacts::from_observation(&obs),
            profile,
            &mut memory,
            None,
            &mut |_, _, _| true,
        );
        // The guards stand on their posts through the secure dwell.
        let posts = expansion_security::positions(&obs, None, &memory.expansion_security);
        for (id, post) in memory
            .expansion_security
            .riflemen
            .clone()
            .iter()
            .zip(&posts)
        {
            let unit = obs.owned.iter_mut().find(|unit| unit.id == *id).unwrap();
            unit.x = post.0;
            unit.y = post.1;
        }
        let ordered = |decision: &AiDecision| {
            decision.intents.contains(&AiIntent::Build {
                kind: EntityKind::ResourceDepot,
            })
        };
        let mut seen = false;
        for _ in 0..4 {
            obs.tick += config::TICK_HZ * 2;
            seen |= ordered(&decide(&obs, profile, &mut memory));
        }
        // 400/95 banked, the Depot 450/100: the live Jeff sends the builder now, the freeze waits
        // until it is banked.
        assert_eq!(seen, live, "{}", profile.id);
    }
}

#[test]
fn slack_keeps_steel_from_a_steel_bound_unlock_but_not_from_an_oil_bound_one() {
    // A Factory up and no Tank Unlock: Steel 160, Oil 400. Unlock (150/100) is due now; a Rifleman
    // (35) would hold it back.
    let mut obs = main_base(160, 400);
    obs.owned.push(building(6, EntityKind::Factory, Some(0)));
    let mut memory = SpendPlanMemory::default();
    memory.observe(&obs, false);
    let facts = AiFacts::from_observation(&obs);
    let laid = plan(&obs, &facts, &memory, no_natural());
    assert!(laid.gates);
    assert_eq!(laid.charged_at(StepKind::TankUnlock), Some(obs.tick));
    assert!(laid.slack_steel < 35, "{}", laid.slack_steel);
    // Oil short instead (Steel 600, Oil 40): Unlock waits on Oil, and Steel to spare is slack.
    let obs = main_base(600, 40);
    let mut obs = obs;
    obs.owned.push(building(6, EntityKind::Factory, Some(0)));
    let facts = AiFacts::from_observation(&obs);
    let laid = plan(&obs, &facts, &memory, no_natural());
    assert!(laid.slack_steel >= 140, "{}", laid.slack_steel);
}

#[test]
fn a_mine_counts_once_where_patches_are_packed_tighter_than_a_tile() {
    // A second patch a third of a tile from each mined one (Fastest Map Possible packs 36 around a
    // Depot): still six mines paying.
    let mut obs = main_base(0, 0);
    for i in 0..6u32 {
        let tile = (4.5 + i as f32 + 0.3, 4.5);
        obs.resources.push(resource(
            700 + i,
            EntityKind::Steel,
            tile.0 * ts(),
            tile.1 * ts(),
        ));
    }
    let mut memory = SpendPlanMemory::default();
    memory.observe(&obs, false);
    let facts = AiFacts::from_observation(&obs);
    let (steel15, _) = plan(&obs, &facts, &memory, no_natural()).expected[0];
    assert!((steel15 - 6.0 * 0.05 * 430.0).abs() < 4.0, "{steel15}");
}

#[test]
fn with_a_factory_up_the_natural_waits_for_its_cost_banked() {
    let input = PlanInput {
        natural_due: true,
        natural_site: Some((40, 8)),
        natural_builder: Some(1),
        natural_wait: 0,
    };
    let mut memory = SpendPlanMemory::default();
    for (steel, oil, funded) in [(420, 120, false), (450, 100, true)] {
        let mut obs = main_base(steel, oil);
        obs.owned.push(building(6, EntityKind::Factory, Some(0)));
        memory.observe(&obs, false);
        let facts = AiFacts::from_observation(&obs);
        let laid = plan(&obs, &facts, &memory, input);
        assert_eq!(laid.natural_funded_on_arrival, funded, "{steel}/{oil}");
        // Planned: set aside once banked, down a walk later.
        let natural = laid.charged_at(StepKind::Natural).unwrap();
        assert!(natural >= obs.tick + 400, "{natural}");
    }
}
