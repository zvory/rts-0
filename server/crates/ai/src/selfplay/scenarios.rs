use crate::ai_core::profiles::{
    AI_1_0_TECH_ID, RIFLE_FLOOD_FAST_ID, RIFLE_FLOOD_FULL_SATURATION_ID,
    STEEL_EXPANSION_TANKS_ID, TECH_TO_TANKS_ID,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BaselineScenario {
    pub id: &'static str,
    pub description: &'static str,
    pub profile_a: &'static str,
    pub profile_b: &'static str,
    pub max_ticks: u32,
    pub seed: u32,
}

pub const BASELINE_SCENARIOS: [BaselineScenario; 8] = [
    BaselineScenario {
        id: "opening_pressure",
        description: "Fast rifle pressure against the full-saturation baseline.",
        profile_a: RIFLE_FLOOD_FAST_ID,
        profile_b: RIFLE_FLOOD_FULL_SATURATION_ID,
        max_ticks: 9_600,
        seed: 0x1234_5678,
    },
    BaselineScenario {
        id: "midgame_expansion",
        description: "Expansion-first macro profile against tank tech.",
        profile_a: STEEL_EXPANSION_TANKS_ID,
        profile_b: TECH_TO_TANKS_ID,
        max_ticks: 14_000,
        seed: 0,
    },
    BaselineScenario {
        id: "tank_tech",
        description: "Full-saturation rifle baseline against tank tech.",
        profile_a: RIFLE_FLOOD_FULL_SATURATION_ID,
        profile_b: TECH_TO_TANKS_ID,
        max_ticks: 12_000,
        seed: 0,
    },
    BaselineScenario {
        id: "blocked_goal_pressure",
        description: "Fast proxy pressure against tank tech to expose delayed or blocked goals.",
        profile_a: RIFLE_FLOOD_FAST_ID,
        profile_b: TECH_TO_TANKS_ID,
        max_ticks: 12_000,
        seed: 0,
    },
    BaselineScenario {
        id: "ai_1_0_early_production",
        description: "AI 1.0 tech profile should open with Riflemen before vehicle production.",
        profile_a: AI_1_0_TECH_ID,
        profile_b: RIFLE_FLOOD_FULL_SATURATION_ID,
        max_ticks: 6_000,
        seed: 0x4100_0001,
    },
    BaselineScenario {
        id: "ai_1_0_tech_blocked_production",
        description: "AI 1.0 tech profile should keep useful production while saving for Tank tech.",
        profile_a: AI_1_0_TECH_ID,
        profile_b: RIFLE_FLOOD_FAST_ID,
        max_ticks: 9_600,
        seed: 0x4100_0002,
    },
    BaselineScenario {
        id: "ai_1_0_scout_car_unlock",
        description: "AI 1.0 tech profile should unlock Factory production and complete Scout Cars.",
        profile_a: AI_1_0_TECH_ID,
        profile_b: TECH_TO_TANKS_ID,
        max_ticks: 12_000,
        seed: 0x4100_0003,
    },
    BaselineScenario {
        id: "ai_1_0_tank_unlock",
        description: "AI 1.0 tech profile should complete Tank research and Tank production.",
        profile_a: AI_1_0_TECH_ID,
        profile_b: RIFLE_FLOOD_FULL_SATURATION_ID,
        max_ticks: 14_000,
        seed: 0x4100_0004,
    },
];

pub fn available_baseline_scenarios() -> &'static [BaselineScenario] {
    &BASELINE_SCENARIOS
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::available_baseline_scenarios;
    use crate::selfplay::canonical_profile_id;

    #[test]
    fn baseline_scenarios_are_selectable_and_profile_backed() {
        let scenarios = available_baseline_scenarios();
        assert_eq!(scenarios.len(), 8);

        let mut ids = BTreeSet::new();
        for scenario in scenarios {
            assert!(ids.insert(scenario.id), "duplicate scenario id");
            assert!(!scenario.description.is_empty());
            assert!(scenario.max_ticks > 0);
            assert_eq!(
                canonical_profile_id(scenario.profile_a),
                Some(scenario.profile_a)
            );
            assert_eq!(
                canonical_profile_id(scenario.profile_b),
                Some(scenario.profile_b)
            );
        }

        assert!(ids.contains("opening_pressure"));
        assert!(ids.contains("midgame_expansion"));
        assert!(ids.contains("tank_tech"));
        assert!(ids.contains("blocked_goal_pressure"));
        assert!(ids.contains("ai_1_0_early_production"));
        assert!(ids.contains("ai_1_0_tech_blocked_production"));
        assert!(ids.contains("ai_1_0_scout_car_unlock"));
        assert!(ids.contains("ai_1_0_tank_unlock"));
    }
}
