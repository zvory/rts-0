use super::{
    AiProfile, AttackPolicy, BarracksCurve, BuildingPolicy, DefensiveMachineGunnerPolicy,
    ExpansionContainmentPolicy, ExpansionPolicy, ExtraFactoryPolicy, FastTankTimingPolicy,
    FrontalWavePolicy, HomeAntiTankPolicy, ProductionPolicy, Ratio, ResourceFloatThreshold,
    ResourcePolicy, SurplusSteelProductionPolicy, TankResourcePolicy, TechTransitionPolicy,
    WorkerPolicy,
};
use rts_sim::game::entity::EntityKind;
use rts_sim::game::upgrade::UpgradeKind;

pub(crate) const JEFFS_AI_CHAT_START_ID: &str = "jeffs_ai_chat_start";

const OPENING_UNITS: [EntityKind; 1] = [EntityKind::MachineGunner];
const ARMORED_UNITS: [EntityKind; 2] = [EntityKind::Tank, EntityKind::ScoutCar];
const ARMORED_TECH_PATH: [EntityKind; 4] = [
    EntityKind::Barracks,
    EntityKind::TrainingCentre,
    EntityKind::Factory,
    EntityKind::EngineeringComplex,
];
const UPGRADES: [UpgradeKind; 2] = [UpgradeKind::TankUnlock, UpgradeKind::Entrenchment];
const OPTIONAL_UPGRADES: [UpgradeKind; 1] = [UpgradeKind::Methamphetamines];

/// Server-authoritative port of the champion V3 policy developed in the standalone
/// `Jeff's AI` workspace. The live controller still emits ordinary fog-constrained
/// commands through the shared AI action layer.
pub(crate) static JEFFS_AI_CHAT_START: AiProfile = AiProfile {
    id: JEFFS_AI_CHAT_START_ID,
    workers: WorkerPolicy {
        steel_saturation_fraction: Ratio::new(1, 1),
        steel_worker_cap: Some(40),
        extra_oil_workers: 10,
        extra_builder_workers: 1,
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
        repush_regroup_radius_tiles: 5.0,
    }),
    home_anti_tank: Some(HomeAntiTankPolicy {
        defensive_tanks: 1,
        target_guns: 2,
        // Keep the guns three tiles behind the six-tile home Tank line while
        // remaining forward of the production-building belt.
        anti_tank_position_tiles: 3.0,
        // Screen 7.5 tiles ahead of the defensive Tanks: deep enough to meet
        // infantry first without detaching the Machine Gunners from support.
        machine_gunner_screen_tiles: 7.5,
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
        builder_engineers_before_barracks: 2,
        automatic_extractors_before_barracks: 2,
        tanks_before_scout_car: 0,
        scout_car_target: 1,
        tanks_before_scout_car_replacement: 2,
        tanks_before_optional_upgrades: 3,
        optional_upgrades: &OPTIONAL_UPGRADES,
        preserve_during_defensive_panic: true,
    }),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_profile_preserves_the_local_v3_champion_targets() {
        let transition = JEFFS_AI_CHAT_START
            .tech_transition
            .expect("armored transition");
        assert_eq!(JEFFS_AI_CHAT_START.workers.steel_worker_cap, Some(40));
        assert_eq!(JEFFS_AI_CHAT_START.workers.extra_oil_workers, 10);
        assert_eq!(JEFFS_AI_CHAT_START.workers.extra_builder_workers, 1);
        assert!(!JEFFS_AI_CHAT_START.workers.train_workers_for_oil);
        assert!(JEFFS_AI_CHAT_START.workers.reuse_idle_before_training);
        assert_eq!(
            JEFFS_AI_CHAT_START
                .defensive_machine_gunners
                .unwrap()
                .target_count,
            2
        );
        assert_eq!(transition.production.unit_priorities, &ARMORED_UNITS);
        assert_eq!(
            transition.required_tech_path,
            &[
                EntityKind::Barracks,
                EntityKind::TrainingCentre,
                EntityKind::Factory,
                EntityKind::EngineeringComplex,
            ]
        );
        assert_eq!(JEFFS_AI_CHAT_START.production.queue_depth, 1);
        assert_eq!(transition.production.queue_depth, 1);
        assert_eq!(transition.attack.first_attack_size, 3);
        assert_eq!(transition.attack.required_unit, Some(EntityKind::ScoutCar));
        assert_eq!(transition.attack.regroup_reset_ticks, 120);
        assert_eq!(
            JEFFS_AI_CHAT_START.frontal_wave.exclude_launched_ticks,
            Some(120)
        );
        assert_eq!(
            JEFFS_AI_CHAT_START.expansion.unwrap().defensive_unit_count,
            1
        );
        let containment = JEFFS_AI_CHAT_START.expansion_containment.unwrap();
        assert_eq!(containment.minimum_tanks_to_continue, 2);
        assert_eq!(containment.recovery_tanks_to_continue, 3);
        assert_eq!(containment.additional_tanks_per_repush, 1);
        assert_eq!(containment.repush_regroup_radius_tiles, 5.0);
        assert_eq!(containment.contact_stop_tiles, 18.0);
        let home_anti_tank = JEFFS_AI_CHAT_START.home_anti_tank.unwrap();
        assert_eq!(home_anti_tank.defensive_tanks, 1);
        assert_eq!(home_anti_tank.target_guns, 2);
        assert_eq!(home_anti_tank.anti_tank_position_tiles, 3.0);
        assert_eq!(home_anti_tank.machine_gunner_screen_tiles, 7.5);
        assert_eq!(
            transition.resource_float,
            ResourceFloatThreshold { steel: 0, oil: 0 }
        );
        let timing = JEFFS_AI_CHAT_START
            .fast_tank_timing
            .expect("fast tank timing");
        assert_eq!(timing.builder_engineers_before_barracks, 2);
        assert_eq!(timing.automatic_extractors_before_barracks, 2);
        assert_eq!(timing.tanks_before_scout_car, 0);
        assert_eq!(timing.scout_car_target, 1);
        assert_eq!(timing.tanks_before_scout_car_replacement, 2);
        assert_eq!(timing.optional_upgrades, &OPTIONAL_UPGRADES);
        assert_eq!(
            JEFFS_AI_CHAT_START
                .surplus_steel_production
                .unwrap()
                .reserve,
            600
        );
        assert_eq!(
            JEFFS_AI_CHAT_START.extra_factories.unwrap().minimum_units,
            3
        );
    }
}
