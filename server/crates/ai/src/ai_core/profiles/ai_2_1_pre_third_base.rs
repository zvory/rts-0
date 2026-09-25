// Frozen AI 2.1 policy from origin/main 69d1e29a6 for internal arena comparisons.
use super::*;

pub(crate) const AI_2_1_PRE_THIRD_BASE_ID: &str = "ai_2_1_pre_third_base";

pub(crate) static AI_2_1_PRE_THIRD_BASE: AiProfile = AiProfile {
    id: AI_2_1_PRE_THIRD_BASE_ID,
    workers: WorkerPolicy {
        steel_saturation_fraction: Ratio::new(1, 1),
        steel_worker_cap: None,
        extra_oil_workers: 12,
        extra_builder_workers: 0,
        train_workers_for_oil: true,
        reuse_idle_before_training: false,
    },
    buildings: BuildingPolicy {
        barracks_curve: BarracksCurve {
            before_steel_saturation: 1,
            after_steel_saturation: 1,
            banked_steel_threshold: 0,
            banked_steel_step: 0,
            max: 1,
        },
        factory_target: 1,
        required_tech_path: &BASE_TECH_PATH,
        max_pending_per_kind: 1,
    },
    extra_factories: Some(ExtraFactoryPolicy {
        target_count: 2,
        minimum_units: 0,
        prerequisite_unit: EntityKind::Tank,
        resource_float: AI_2_1_SECOND_FACTORY_FLOAT_THRESHOLD,
    }),
    surplus_steel_production: None,
    production: ProductionPolicy {
        queue_depth: 2,
        unit_priorities: &RIFLE_ONLY,
        save_for_first_tech_unit: None,
        balance_unit_priorities: false,
    },
    upgrade_priorities: &NO_UPGRADES,
    attack: AttackPolicy {
        first_attack_size: 4,
        wave_growth: 2,
        regroup_reset_ticks: 480,
        reissue_cadence_ticks: 120,
        stage_distance_tiles: 8.0,
        unit_kinds: &RIFLE_ONLY,
        required_unit: None,
    },
    resources: ResourcePolicy {
        oil_after_steel_workers: 5,
        tank_adaptive: Some(TankResourcePolicy {
            max_oil_workers: 12,
            oil_workers_per_factory: 6,
            deficit_response_workers: 2,
        }),
    },
    production_expansion: None,
    expansion: Some(ExpansionPolicy {
        target_resource_depots: 2,
        required_complete_building: EntityKind::TrainingCentre,
        defensive_unit: EntityKind::Rifleman,
        defensive_unit_count: 4,
        pre_expansion_steel_worker_cap: 18,
        post_expansion_steel_worker_cap: Some(36),
        search_radius_tiles: 6,
        trigger_steel: 350,
        trigger_supply_used: 30,
        blocks_tech_path: false,
        oil_before_steel_in_expansion: true,
        remote_worker_assignment_fallback: true,
    }),
    defensive_machine_gunners: Some(DefensiveMachineGunnerPolicy {
        target_count: 4,
        perimeter_distance_tiles: 20.0,
        lateral_spacing_tiles: 1.5,
        replacement_health_percent: None,
    }),
    turtle_defense: None,
    frontal_wave: FrontalWavePolicy {
        exclude_launched_ticks: Some(FRONTAL_COHORT_TICKS),
        line_staging: true,
    },
    expansion_containment: None,
    home_anti_tank: None,
    tech_transition: Some(TechTransitionPolicy {
        resource_float: AI_2_1_TANK_PRESSURE_FLOAT_THRESHOLD,
        required_tech_path: &TANK_TECH_PATH,
        production: ProductionPolicy {
            queue_depth: 3,
            unit_priorities: &TANK_AND_RIFLE,
            save_for_first_tech_unit: None,
            balance_unit_priorities: false,
        },
        attack: AttackPolicy {
            first_attack_size: 2,
            wave_growth: 1,
            regroup_reset_ticks: 480,
            reissue_cadence_ticks: 120,
            stage_distance_tiles: 8.0,
            unit_kinds: &TANK_AND_RIFLE,
            required_unit: None,
        },
    }),
    fast_tank_timing: None,
};
