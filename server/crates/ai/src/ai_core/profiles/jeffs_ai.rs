use super::{
    AiProfile, AttackPolicy, BarracksCurve, BuildingPolicy, DefensiveMachineGunnerPolicy,
    ExpansionContainmentPolicy, ExpansionPolicy, ExtraFactoryPolicy, FastTankTimingPolicy,
    FrontalWavePolicy, HomeAntiTankPolicy, ProductionPolicy, Ratio, ResourceFloatThreshold,
    ResourcePolicy, SurplusSteelProductionPolicy, TankResourcePolicy, TechTransitionPolicy,
    WorkerPolicy,
};
use rts_sim::game::entity::EntityKind;
use rts_sim::game::upgrade::UpgradeKind;

pub(crate) const JEFFS_AI_ID: &str = "jeffs_ai";
pub(crate) const JEFFS_AI_PRE_EARLY_METH_ID: &str = "jeffs_ai_pre_early_meth";
pub(crate) const JEFFS_AI_PRE_DEFENSE_ENVELOPE_ID: &str = "jeffs_ai_pre_defense_envelope";
pub(crate) const JEFFS_AI_PRE_RIFLE_COVERAGE_ID: &str = "jeffs_ai_pre_rifle_coverage";

const OPENING_UNITS: [EntityKind; 1] = [EntityKind::MachineGunner];
const ARMORED_UNITS: [EntityKind; 2] = [EntityKind::Tank, EntityKind::ScoutCar];
const ARMORED_TECH_PATH: [EntityKind; 4] = [
    EntityKind::Barracks,
    EntityKind::TrainingCentre,
    EntityKind::EngineeringComplex,
    EntityKind::Factory,
];
const UPGRADES: [UpgradeKind; 2] = [UpgradeKind::TankUnlock, UpgradeKind::Entrenchment];
const EARLY_METH_UPGRADES: [UpgradeKind; 3] = [
    UpgradeKind::TankUnlock,
    UpgradeKind::Entrenchment,
    UpgradeKind::Methamphetamines,
];
const NO_OPTIONAL_UPGRADES: [UpgradeKind; 0] = [];
const OPTIONAL_UPGRADES: [UpgradeKind; 1] = [UpgradeKind::Methamphetamines];

const EARLY_METH_FAST_TANK_TIMING: FastTankTimingPolicy = FastTankTimingPolicy {
    workers_before_barracks: 1,
    pump_jacks_before_barracks: 2,
    tanks_before_scout_car: 2,
    scout_car_target: 1,
    tanks_before_optional_upgrades: 3,
    optional_upgrades: &NO_OPTIONAL_UPGRADES,
    preserve_during_defensive_panic: true,
};

/// Server-authoritative port of the champion V3 policy developed in the standalone
/// `Jeff's AI` workspace. The live controller still emits ordinary fog-constrained
/// commands through the shared AI action layer.
const JEFFS_AI_TEMPLATE: AiProfile = AiProfile {
    id: JEFFS_AI_ID,
    workers: WorkerPolicy {
        steel_saturation_fraction: Ratio::new(1, 1),
        steel_worker_cap: Some(40),
        extra_oil_workers: 10,
        extra_builder_workers: 0,
        train_workers_for_oil: false,
        reuse_idle_before_training: true,
    },
    buildings: BuildingPolicy {
        barracks_curve: BarracksCurve {
            before_steel_saturation: 1,
            after_steel_saturation: 1,
            banked_steel_threshold: 600,
            banked_steel_step: u32::MAX,
            max: 2,
        },
        factory_target: 1,
        required_tech_path: &ARMORED_TECH_PATH,
        max_pending_per_kind: 1,
    },
    extra_factories: Some(ExtraFactoryPolicy {
        target_count: 2,
        minimum_units: 3,
        prerequisite_unit: EntityKind::Tank,
        resource_float: ResourceFloatThreshold {
            steel: 250,
            oil: 125,
        },
    }),
    surplus_steel_production: Some(SurplusSteelProductionPolicy {
        reserve: 600,
        unit: EntityKind::Rifleman,
    }),
    production: ProductionPolicy {
        queue_depth: 1,
        unit_priorities: &OPENING_UNITS,
        save_for_first_tech_unit: Some(EntityKind::Tank),
        balance_unit_priorities: false,
    },
    upgrade_priorities: &UPGRADES,
    attack: AttackPolicy {
        first_attack_size: 4,
        wave_growth: 2,
        regroup_reset_ticks: 450,
        reissue_cadence_ticks: 450,
        stage_distance_tiles: 3.25,
        unit_kinds: &OPENING_UNITS,
        required_unit: Some(EntityKind::MachineGunner),
    },
    resources: ResourcePolicy {
        oil_after_steel_workers: 5,
        tank_adaptive: Some(TankResourcePolicy {
            max_oil_workers: 10,
            oil_workers_per_factory: 5,
            deficit_response_workers: 2,
        }),
    },
    expansion: Some(ExpansionPolicy {
        target_resource_depots: 2,
        required_complete_building: EntityKind::Factory,
        defensive_unit: EntityKind::Tank,
        defensive_unit_count: 1,
        pre_expansion_steel_worker_cap: 18,
        post_expansion_steel_worker_cap: Some(40),
        search_radius_tiles: 6,
        trigger_steel: 300,
        trigger_supply_used: 24,
        blocks_tech_path: false,
        oil_before_steel_in_expansion: true,
        remote_worker_assignment_fallback: true,
    }),
    defensive_machine_gunners: Some(DefensiveMachineGunnerPolicy {
        target_count: 2,
        perimeter_distance_tiles: 6.0,
        lateral_spacing_tiles: 4.5,
        replacement_health_percent: Some(50),
    }),
    turtle_defense: None,
    frontal_wave: FrontalWavePolicy {
        exclude_launched_ticks: Some(120),
        line_staging: true,
    },
    expansion_containment: Some(ExpansionContainmentPolicy {
        // A stationary Tank ramps from 5 to 14 tiles over three seconds.
        // Anchor just inside the fully charged range so it does not have to
        // translate and lose that bonus when the expansion becomes visible.
        tank_standoff_tiles: 13.5,
        scout_trailing_tiles: 1.5,
        scout_forward_tiles: 2.0,
        flank_tiles: 5.0,
        contact_stop_tiles: 18.0,
        minimum_tanks_to_continue: 2,
        recovery_tanks_to_continue: 3,
        additional_tanks_per_repush: 1,
        repush_regroup_radius_tiles: 3.0,
    }),
    home_anti_tank: Some(HomeAntiTankPolicy {
        defensive_tanks: 1,
        target_guns: 2,
        // Keep the guns three tiles behind the six-tile home Tank line while
        // remaining forward of the production-building belt.
        anti_tank_position_tiles: 3.0,
        // Keep the valuable MGs behind the Rifle screen, but one tile ahead
        // of the home Tank so all three layers acquire the same raid.
        machine_gunner_screen_tiles: 1.0,
        lateral_spacing_tiles: 4.5,
    }),
    tech_transition: Some(TechTransitionPolicy {
        resource_float: ResourceFloatThreshold { steel: 0, oil: 0 },
        required_tech_path: &ARMORED_TECH_PATH,
        production: ProductionPolicy {
            queue_depth: 1,
            unit_priorities: &ARMORED_UNITS,
            save_for_first_tech_unit: Some(EntityKind::Tank),
            balance_unit_priorities: false,
        },
        attack: AttackPolicy {
            first_attack_size: 3,
            wave_growth: 1,
            regroup_reset_ticks: 120,
            reissue_cadence_ticks: 120,
            stage_distance_tiles: 8.0,
            unit_kinds: &ARMORED_UNITS,
            required_unit: Some(EntityKind::ScoutCar),
        },
    }),
    fast_tank_timing: Some(FastTankTimingPolicy {
        workers_before_barracks: 1,
        pump_jacks_before_barracks: 2,
        tanks_before_scout_car: 2,
        scout_car_target: 1,
        tanks_before_optional_upgrades: 3,
        optional_upgrades: &OPTIONAL_UPGRADES,
        preserve_during_defensive_panic: true,
    }),
};

pub(crate) static JEFFS_AI: AiProfile = AiProfile {
    upgrade_priorities: &EARLY_METH_UPGRADES,
    fast_tank_timing: Some(EARLY_METH_FAST_TANK_TIMING),
    ..JEFFS_AI_TEMPLATE
};

/// Frozen immediately before Methamphetamines moved directly behind Entrenchment. This profile
/// retains the defended-building envelope and bounded incident response for isolated arena tests.
pub(crate) static JEFFS_AI_PRE_EARLY_METH: AiProfile = AiProfile {
    id: JEFFS_AI_PRE_EARLY_METH_ID,
    ..JEFFS_AI_TEMPLATE
};

/// Frozen immediately before defended-building envelope placement and bounded incident response.
/// This remains internal and exists so arena runs can compare the active profile against the exact
/// prior behavior in one authoritative simulation.
pub(crate) static JEFFS_AI_PRE_DEFENSE_ENVELOPE: AiProfile = AiProfile {
    id: JEFFS_AI_PRE_DEFENSE_ENVELOPE_ID,
    ..JEFFS_AI_TEMPLATE
};

/// Frozen immediately before the home-Rifleman coverage change. This remains internal and
/// addressable only for deterministic balance comparisons.
pub(crate) static JEFFS_AI_PRE_RIFLE_COVERAGE: AiProfile = AiProfile {
    id: JEFFS_AI_PRE_RIFLE_COVERAGE_ID,
    ..JEFFS_AI_TEMPLATE
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_profile_preserves_the_local_v3_champion_targets() {
        let transition = JEFFS_AI.tech_transition.expect("armored transition");
        assert_eq!(JEFFS_AI.workers.steel_worker_cap, Some(40));
        assert_eq!(JEFFS_AI.workers.extra_oil_workers, 10);
        assert_eq!(JEFFS_AI.workers.extra_builder_workers, 0);
        assert!(!JEFFS_AI.workers.train_workers_for_oil);
        assert!(JEFFS_AI.workers.reuse_idle_before_training);
        assert_eq!(JEFFS_AI.defensive_machine_gunners.unwrap().target_count, 2);
        assert_eq!(
            JEFFS_AI
                .defensive_machine_gunners
                .unwrap()
                .lateral_spacing_tiles,
            4.5
        );
        assert_eq!(transition.production.unit_priorities, &ARMORED_UNITS);
        assert_eq!(JEFFS_AI.production.queue_depth, 1);
        assert_eq!(transition.production.queue_depth, 1);
        assert_eq!(transition.attack.first_attack_size, 3);
        assert_eq!(transition.attack.required_unit, Some(EntityKind::ScoutCar));
        assert_eq!(transition.attack.regroup_reset_ticks, 120);
        assert_eq!(JEFFS_AI.frontal_wave.exclude_launched_ticks, Some(120));
        assert_eq!(JEFFS_AI.expansion.unwrap().defensive_unit_count, 1);
        let containment = JEFFS_AI.expansion_containment.unwrap();
        assert_eq!(containment.minimum_tanks_to_continue, 2);
        assert_eq!(containment.recovery_tanks_to_continue, 3);
        assert_eq!(containment.additional_tanks_per_repush, 1);
        assert_eq!(containment.repush_regroup_radius_tiles, 3.0);
        assert_eq!(containment.contact_stop_tiles, 18.0);
        let home_anti_tank = JEFFS_AI.home_anti_tank.unwrap();
        assert_eq!(home_anti_tank.defensive_tanks, 1);
        assert_eq!(home_anti_tank.target_guns, 2);
        assert_eq!(home_anti_tank.anti_tank_position_tiles, 3.0);
        assert_eq!(home_anti_tank.machine_gunner_screen_tiles, 1.0);
        assert_eq!(
            transition.resource_float,
            ResourceFloatThreshold { steel: 0, oil: 0 }
        );
        let timing = JEFFS_AI.fast_tank_timing.expect("fast tank timing");
        assert_eq!(timing.workers_before_barracks, 1);
        assert_eq!(timing.pump_jacks_before_barracks, 2);
        assert_eq!(timing.tanks_before_scout_car, 2);
        assert_eq!(timing.scout_car_target, 1);
        assert_eq!(JEFFS_AI.upgrade_priorities, &EARLY_METH_UPGRADES);
        assert_eq!(timing.optional_upgrades, &NO_OPTIONAL_UPGRADES);
        assert_eq!(JEFFS_AI_PRE_EARLY_METH.upgrade_priorities, &UPGRADES);
        assert_eq!(
            JEFFS_AI_PRE_EARLY_METH
                .fast_tank_timing
                .expect("frozen fast tank timing")
                .optional_upgrades,
            &OPTIONAL_UPGRADES
        );
        assert_eq!(JEFFS_AI.surplus_steel_production.unwrap().reserve, 600);
        assert_eq!(JEFFS_AI.extra_factories.unwrap().minimum_units, 3);
    }
}
