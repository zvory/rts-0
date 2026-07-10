use super::*;

const TURTLE_SECOND_GUN_WORKS_FLOAT_THRESHOLD: ResourceFloatThreshold = ResourceFloatThreshold {
    steel: 600,
    oil: 250,
};
const TURTLE_EXPANSION_STEEL_TRIGGER: u32 = 0;
const TURTLE_UNITS: [EntityKind; 3] = [
    EntityKind::AntiTankGun,
    EntityKind::MachineGunner,
    EntityKind::Rifleman,
];
const TURTLE_ANTI_TANK: [EntityKind; 1] = [EntityKind::AntiTankGun];
const TURTLE_UPGRADES: [UpgradeKind; 2] =
    [UpgradeKind::Entrenchment, UpgradeKind::AntiTankGunUnlock];
const TURTLE_TECH_PATH: [EntityKind; 4] = [
    EntityKind::Barracks,
    EntityKind::TrainingCentre,
    EntityKind::ResearchComplex,
    EntityKind::Steelworks,
];

pub(crate) static AI_TURTLE_CHOKES: AiProfile = AiProfile {
    id: AI_TURTLE_CHOKES_ID,
    economy: EconomyPolicy::ProposalManager,
    // Keep Turtle on the AI 2.0/2.1 economy cadence.  Its opening and defensive
    // production differ, but worker, oil, supply, and first-Barracks spending must
    // not put it behind the pressure profile before support weapons come online.
    workers: AI_2_1_ECONOMY_MANAGER.workers,
    supply: AI_2_1_ECONOMY_MANAGER.supply,
    buildings: BuildingPolicy {
        barracks_curve: AI_2_1_ECONOMY_MANAGER.buildings.barracks_curve,
        factory_target: 0,
        proxy_barracks: None,
        required_tech_path: &TURTLE_TECH_PATH,
        max_pending_per_kind: 1,
    },
    extra_factories: None,
    production: ProductionPolicy {
        queue_depth: 3,
        unit_priorities: &TURTLE_UNITS,
        save_for_first_tech_unit: None,
        balance_unit_priorities: false,
    },
    upgrade_priorities: &TURTLE_UPGRADES,
    attack: AttackPolicy {
        first_attack_size: usize::MAX,
        wave_growth: 0,
        regroup_reset_ticks: 540,
        reissue_cadence_ticks: 120,
        stage_distance_tiles: 0.0,
        unit_kinds: &TURTLE_UNITS,
        required_unit: None,
    },
    resources: AI_2_1_ECONOMY_MANAGER.resources,
    expansion: Some(ExpansionPolicy {
        target_city_centres: 2,
        required_complete_building: EntityKind::TrainingCentre,
        defensive_unit: EntityKind::Rifleman,
        defensive_unit_count: 0,
        pre_expansion_steel_worker_cap: 18,
        post_expansion_steel_worker_cap: Some(36),
        search_radius_tiles: 6,
        // Turtle cannot rely on the Tank profile's high army spending to bank
        // the ordinary 350-steel trigger. Once its Training Centre is complete,
        // reserve for the second City Centre before adding more support units.
        trigger_steel: TURTLE_EXPANSION_STEEL_TRIGGER,
        trigger_supply_used: 30,
        blocks_tech_path: false,
        oil_before_steel_in_expansion: true,
        remote_worker_assignment_fallback: true,
    }),
    defensive_machine_gunners: None,
    turtle_defense: Some(TurtleDefensePolicy {
        max_chokes: 3,
        anti_tank_back_tiles: 10.0,
        opening_riflemen: 2,
        support_barracks_target: 1,
        gun_works_target: 2,
        gun_works_resource_float: TURTLE_SECOND_GUN_WORKS_FLOAT_THRESHOLD,
        main_machine_gunner_target: 2,
        machine_gunner_target_chokes: 2,
        machine_gunners_per_choke: 4,
        machine_gunner_slot_gap_tiles: 3.0,
        slot_gap_tiles: 2.0,
        anti_tank_kinds: &TURTLE_ANTI_TANK,
    }),
    frontal_wave: FrontalWavePolicy::DEFAULT,
    recovery_transition: None,
    tech_transition: None,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turtle_keeps_ai_2_economy_cadence_and_defers_only_second_gun_works() {
        let ai2 = AI_2_1_ECONOMY_MANAGER;
        let turtle = AI_TURTLE_CHOKES.turtle_defense.unwrap();

        assert_eq!(AI_TURTLE_CHOKES.workers, ai2.workers);
        assert_eq!(AI_TURTLE_CHOKES.supply, ai2.supply);
        assert_eq!(AI_TURTLE_CHOKES.resources, ai2.resources);
        assert_eq!(
            AI_TURTLE_CHOKES.buildings.barracks_curve,
            ai2.buildings.barracks_curve
        );
        assert_eq!(turtle.opening_riflemen, 2);
        assert_eq!(
            AI_TURTLE_CHOKES.expansion.unwrap().trigger_steel,
            TURTLE_EXPANSION_STEEL_TRIGGER
        );
        assert_eq!(turtle.gun_works_target, 2);
        assert_eq!(
            turtle.gun_works_resource_float,
            TURTLE_SECOND_GUN_WORKS_FLOAT_THRESHOLD
        );
    }
}
