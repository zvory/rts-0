use super::*;

const TURTLE_SECOND_BARRACKS_STEEL_THRESHOLD: u32 = 450;
const TURTLE_SECOND_GUN_WORKS_FLOAT_THRESHOLD: ResourceFloatThreshold = ResourceFloatThreshold {
    steel: 600,
    oil: 250,
};
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
    workers: WorkerPolicy {
        steel_saturation_fraction: Ratio::new(1, 1),
        steel_worker_cap: None,
        extra_oil_workers: 6,
        pressure_worker_cap: None,
        pressure_until_complete: None,
    },
    supply: SupplyPolicy {
        free_supply_buffer: 10,
        emergency_depot_threshold: 4,
    },
    buildings: BuildingPolicy {
        barracks_curve: BarracksCurve {
            before_steel_saturation: 1,
            after_steel_saturation: 1,
            banked_steel_threshold: TURTLE_SECOND_BARRACKS_STEEL_THRESHOLD,
            banked_steel_step: TURTLE_SECOND_BARRACKS_STEEL_THRESHOLD,
            max: 2,
        },
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
    resources: ResourcePolicy {
        oil_after_steel_workers: 6,
        oil_after_full_steel_saturation: false,
        tank_adaptive: None,
    },
    expansion: Some(ExpansionPolicy {
        target_city_centres: 2,
        required_complete_building: EntityKind::TrainingCentre,
        defensive_unit: EntityKind::Rifleman,
        defensive_unit_count: 0,
        pre_expansion_steel_worker_cap: 18,
        post_expansion_steel_worker_cap: Some(36),
        search_radius_tiles: 6,
        trigger_steel: 350,
        trigger_supply_used: 30,
        blocks_tech_path: false,
        oil_before_steel_in_expansion: true,
        remote_worker_assignment_fallback: true,
    }),
    defensive_machine_gunners: None,
    turtle_defense: Some(TurtleDefensePolicy {
        max_chokes: 3,
        anti_tank_back_tiles: 10.0,
        opening_riflemen: 3,
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
    fn turtle_adds_support_producers_only_after_their_float_thresholds() {
        let barracks = AI_TURTLE_CHOKES.buildings.barracks_curve;
        let turtle = AI_TURTLE_CHOKES.turtle_defense.unwrap();

        assert_eq!(
            barracks.target(TURTLE_SECOND_BARRACKS_STEEL_THRESHOLD, 0, 18),
            1
        );
        assert_eq!(
            barracks.target(TURTLE_SECOND_BARRACKS_STEEL_THRESHOLD + 1, 0, 18),
            2
        );
        assert_eq!(barracks.max, 2);
        assert_eq!(turtle.gun_works_target, 2);
        assert_eq!(
            turtle.gun_works_resource_float,
            TURTLE_SECOND_GUN_WORKS_FLOAT_THRESHOLD
        );
    }
}
