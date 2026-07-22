use super::{
    AiProfile, AttackPolicy, BarracksCurve, BuildingPolicy, DefensiveMachineGunnerPolicy,
    ExpansionPolicy, ExtraFactoryPolicy, FrontalWavePolicy, ProductionPolicy, Ratio,
    ResourceFloatThreshold, ResourcePolicy, TankResourcePolicy, TechTransitionPolicy, WorkerPolicy,
};
use rts_sim::game::entity::EntityKind;
use rts_sim::game::upgrade::UpgradeKind;

pub(crate) const JEFFS_AI_ID: &str = "jeffs_ai";

const OPENING_UNITS: [EntityKind; 2] = [EntityKind::Rifleman, EntityKind::MachineGunner];
const RIFLE_ONLY: [EntityKind; 1] = [EntityKind::Rifleman];
const ARMORED_UNITS: [EntityKind; 4] = [
    EntityKind::Tank,
    EntityKind::ScoutCar,
    EntityKind::CommandCar,
    EntityKind::Rifleman,
];
const BASE_TECH_PATH: [EntityKind; 2] = [EntityKind::Barracks, EntityKind::TrainingCentre];
const ARMORED_TECH_PATH: [EntityKind; 4] = [
    EntityKind::Barracks,
    EntityKind::TrainingCentre,
    EntityKind::ResearchComplex,
    EntityKind::Factory,
];
const UPGRADES: [UpgradeKind; 2] = [UpgradeKind::TankUnlock, UpgradeKind::Entrenchment];

/// Server-authoritative port of the champion V3 policy developed in the standalone
/// `Jeff's AI` workspace. The live controller still emits ordinary fog-constrained
/// commands through the shared AI action layer.
pub(crate) static JEFFS_AI: AiProfile = AiProfile {
    id: JEFFS_AI_ID,
    workers: WorkerPolicy {
        steel_saturation_fraction: Ratio::new(1, 1),
        steel_worker_cap: Some(40),
        extra_oil_workers: 10,
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
        resource_float: ResourceFloatThreshold {
            steel: 350,
            oil: 225,
        },
    }),
    production: ProductionPolicy {
        queue_depth: 2,
        unit_priorities: &OPENING_UNITS,
        save_for_first_tech_unit: Some(EntityKind::MachineGunner),
        balance_unit_priorities: true,
    },
    upgrade_priorities: &UPGRADES,
    attack: AttackPolicy {
        first_attack_size: 4,
        wave_growth: 2,
        regroup_reset_ticks: 450,
        reissue_cadence_ticks: 450,
        stage_distance_tiles: 3.25,
        unit_kinds: &RIFLE_ONLY,
        required_unit: None,
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
        target_city_centres: 2,
        required_complete_building: EntityKind::TrainingCentre,
        defensive_unit: EntityKind::MachineGunner,
        defensive_unit_count: 3,
        pre_expansion_steel_worker_cap: 18,
        post_expansion_steel_worker_cap: Some(40),
        search_radius_tiles: 6,
        trigger_steel: 450,
        trigger_supply_used: 30,
        blocks_tech_path: false,
        oil_before_steel_in_expansion: true,
        remote_worker_assignment_fallback: true,
    }),
    defensive_machine_gunners: Some(DefensiveMachineGunnerPolicy { target_count: 7 }),
    turtle_defense: None,
    frontal_wave: FrontalWavePolicy {
        exclude_launched_ticks: Some(450),
        line_staging: true,
    },
    tech_transition: Some(TechTransitionPolicy {
        resource_float: ResourceFloatThreshold {
            steel: 275,
            oil: 100,
        },
        required_tech_path: &ARMORED_TECH_PATH,
        production: ProductionPolicy {
            queue_depth: 3,
            unit_priorities: &ARMORED_UNITS,
            save_for_first_tech_unit: Some(EntityKind::Tank),
            balance_unit_priorities: false,
        },
        attack: AttackPolicy {
            first_attack_size: 5,
            wave_growth: 1,
            regroup_reset_ticks: 450,
            reissue_cadence_ticks: 450,
            stage_distance_tiles: 3.25,
            unit_kinds: &ARMORED_UNITS,
            required_unit: Some(EntityKind::ScoutCar),
        },
    }),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_profile_preserves_the_local_v3_champion_targets() {
        let transition = JEFFS_AI.tech_transition.expect("armored transition");
        assert_eq!(JEFFS_AI.workers.steel_worker_cap, Some(40));
        assert_eq!(JEFFS_AI.workers.extra_oil_workers, 10);
        assert_eq!(JEFFS_AI.attack.first_attack_size, 4);
        assert_eq!(JEFFS_AI.attack.unit_kinds, &[EntityKind::Rifleman]);
        assert_eq!(transition.attack.first_attack_size, 5);
        assert_eq!(transition.attack.required_unit, Some(EntityKind::ScoutCar));
        assert_eq!(JEFFS_AI.defensive_machine_gunners.unwrap().target_count, 7);
    }
}
