#![allow(dead_code)]

use rts_sim::game::entity::EntityKind;
use rts_sim::game::upgrade::UpgradeKind;

pub(crate) const RIFLE_FLOOD_FAST_ID: &str = "rifle_flood_fast";
pub(crate) const RIFLE_FLOOD_FULL_SATURATION_ID: &str = "rifle_flood_full_saturation";
pub(crate) const TECH_TO_TANKS_ID: &str = "tech_to_tanks";
pub(crate) const STEEL_EXPANSION_TANKS_ID: &str = "steel_expansion_tanks";
pub(crate) const AI_1_0_TECH_ID: &str = "ai_1_0_tech";
pub(crate) const AI_1_1_TANK_MG_ID: &str = "ai_1_1_tank_mg";
pub(crate) const AI_1_2_WAVE_COHORTS_ID: &str = "ai_1_2_wave_cohorts";
pub(crate) const AI_2_0_TANK_PRESSURE_ID: &str = "ai_2_0_tank_pressure";
pub(crate) const AI_2_1_ECONOMY_MANAGER_ID: &str = "ai_2_1_economy_manager";
pub(crate) const AI_TURTLE_CHOKES_ID: &str = "ai_turtle_chokes";

const AI_1_2_FRONTAL_COHORT_TICKS: u32 = 3_600;
const TANK_TECH_FLOAT_THRESHOLD: ResourceFloatThreshold = ResourceFloatThreshold {
    steel: 400,
    oil: 150,
};
const SUPPORT_TO_TANK_FLOAT_THRESHOLD: ResourceFloatThreshold = ResourceFloatThreshold {
    steel: 500,
    oil: 300,
};
const LATE_TANK_TECH_FLOAT_THRESHOLD: ResourceFloatThreshold = ResourceFloatThreshold {
    steel: 700,
    oil: 300,
};
const AI_1_2_SECOND_FACTORY_FLOAT_THRESHOLD: ResourceFloatThreshold = ResourceFloatThreshold {
    steel: 600,
    oil: 400,
};
const AI_2_0_TANK_PRESSURE_FLOAT_THRESHOLD: ResourceFloatThreshold = ResourceFloatThreshold {
    steel: 275,
    oil: 100,
};
const AI_2_0_SECOND_FACTORY_FLOAT_THRESHOLD: ResourceFloatThreshold = ResourceFloatThreshold {
    steel: 500,
    oil: 325,
};
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AiProfile {
    pub(crate) id: &'static str,
    pub(crate) workers: WorkerPolicy,
    pub(crate) supply: SupplyPolicy,
    pub(crate) buildings: BuildingPolicy,
    pub(crate) extra_factories: Option<ExtraFactoryPolicy>,
    pub(crate) production: ProductionPolicy,
    pub(crate) upgrade_priorities: &'static [UpgradeKind],
    pub(crate) attack: AttackPolicy,
    pub(crate) resources: ResourcePolicy,
    pub(crate) expansion: Option<ExpansionPolicy>,
    pub(crate) defensive_machine_gunners: Option<DefensiveMachineGunnerPolicy>,
    pub(crate) turtle_defense: Option<TurtleDefensePolicy>,
    pub(crate) frontal_wave: FrontalWavePolicy,
    pub(crate) recovery_transition: Option<RecoveryTransitionPolicy>,
    pub(crate) tech_transition: Option<TechTransitionPolicy>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WorkerPolicy {
    pub(crate) steel_saturation_fraction: Ratio,
    pub(crate) steel_worker_cap: Option<usize>,
    pub(crate) extra_oil_workers: usize,
    pub(crate) pressure_worker_cap: Option<usize>,
    pub(crate) pressure_until_complete: Option<EntityKind>,
}

impl WorkerPolicy {
    pub(crate) fn target_steel_workers(
        self,
        main_base_steel_saturation: usize,
        complete_gate_count: usize,
    ) -> usize {
        let mut target = self
            .steel_saturation_fraction
            .apply_ceil(main_base_steel_saturation);
        if let Some(cap) = self.steel_worker_cap {
            target = target.min(cap);
        }
        if self.pressure_until_complete.is_some() && complete_gate_count == 0 {
            if let Some(cap) = self.pressure_worker_cap {
                target = target.min(cap);
            }
        }
        target
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Ratio {
    numerator: usize,
    denominator: usize,
}

impl Ratio {
    pub(crate) const fn new(numerator: usize, denominator: usize) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    pub(crate) fn apply_ceil(self, value: usize) -> usize {
        if self.denominator == 0 {
            return value;
        }
        value
            .saturating_mul(self.numerator)
            .saturating_add(self.denominator - 1)
            / self.denominator
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SupplyPolicy {
    pub(crate) free_supply_buffer: u32,
    pub(crate) emergency_depot_threshold: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BuildingPolicy {
    pub(crate) barracks_curve: BarracksCurve,
    pub(crate) factory_target: usize,
    pub(crate) proxy_barracks: Option<ProxyBarracksPolicy>,
    pub(crate) required_tech_path: &'static [EntityKind],
    pub(crate) max_pending_per_kind: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExtraFactoryPolicy {
    pub(crate) target_count: usize,
    pub(crate) resource_float: ResourceFloatThreshold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExpansionPolicy {
    pub(crate) target_city_centres: usize,
    pub(crate) required_complete_building: EntityKind,
    pub(crate) defensive_unit: EntityKind,
    pub(crate) defensive_unit_count: usize,
    pub(crate) pre_expansion_steel_worker_cap: usize,
    pub(crate) post_expansion_steel_worker_cap: Option<usize>,
    pub(crate) search_radius_tiles: i32,
    pub(crate) trigger_steel: u32,
    pub(crate) trigger_supply_used: u32,
    pub(crate) blocks_tech_path: bool,
    pub(crate) oil_before_steel_in_expansion: bool,
    pub(crate) remote_worker_assignment_fallback: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProxyBarracksPolicy {
    pub(crate) search_radius_tiles: i32,
    pub(crate) min_enemy_base_distance_tiles: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BarracksCurve {
    pub(crate) before_steel_saturation: usize,
    pub(crate) after_steel_saturation: usize,
    pub(crate) banked_steel_threshold: u32,
    pub(crate) banked_steel_step: u32,
    pub(crate) max: usize,
}

impl BarracksCurve {
    pub(crate) fn target(
        self,
        steel: u32,
        worker_count: usize,
        target_steel_workers: usize,
    ) -> usize {
        let base = if worker_count >= target_steel_workers {
            self.after_steel_saturation
        } else {
            self.before_steel_saturation
        };
        let extra = if self.banked_steel_step == 0 {
            0
        } else {
            steel
                .checked_sub(self.banked_steel_threshold + 1)
                .map(|over| 1 + (over / self.banked_steel_step) as usize)
                .unwrap_or(0)
        };
        base.saturating_add(extra).min(self.max)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProductionPolicy {
    pub(crate) queue_depth: usize,
    pub(crate) unit_priorities: &'static [EntityKind],
    pub(crate) save_for_first_tech_unit: Option<EntityKind>,
    pub(crate) balance_unit_priorities: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AttackPolicy {
    pub(crate) first_attack_size: usize,
    pub(crate) wave_growth: usize,
    pub(crate) regroup_reset_ticks: u32,
    pub(crate) reissue_cadence_ticks: u32,
    pub(crate) stage_distance_tiles: f32,
    pub(crate) unit_kinds: &'static [EntityKind],
    pub(crate) required_unit: Option<EntityKind>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DefensiveMachineGunnerPolicy {
    pub(crate) target_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TurtleDefensePolicy {
    pub(crate) max_chokes: usize,
    pub(crate) anti_tank_back_tiles: f32,
    pub(crate) opening_riflemen: usize,
    pub(crate) support_barracks_target: usize,
    pub(crate) main_machine_gunner_target: usize,
    pub(crate) machine_gunner_target_chokes: usize,
    pub(crate) machine_gunners_per_choke: usize,
    pub(crate) machine_gunner_slot_gap_tiles: f32,
    pub(crate) slot_gap_tiles: f32,
    pub(crate) anti_tank_kinds: &'static [EntityKind],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct FrontalWavePolicy {
    pub(crate) exclude_launched_ticks: Option<u32>,
    pub(crate) line_staging: bool,
}

impl FrontalWavePolicy {
    pub(crate) const DEFAULT: Self = Self {
        exclude_launched_ticks: None,
        line_staging: false,
    };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TechTransitionPolicy {
    pub(crate) resource_float: ResourceFloatThreshold,
    pub(crate) required_tech_path: &'static [EntityKind],
    pub(crate) production: ProductionPolicy,
    pub(crate) attack: AttackPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResourceFloatThreshold {
    pub(crate) steel: u32,
    pub(crate) oil: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RecoveryTransitionPolicy {
    pub(crate) completed_building: EntityKind,
    pub(crate) delay_unit: EntityKind,
    pub(crate) delay_unit_build_count: u32,
    pub(crate) workers: WorkerPolicy,
    pub(crate) barracks_curve: BarracksCurve,
    pub(crate) required_tech_path: &'static [EntityKind],
    pub(crate) production: ProductionPolicy,
    pub(crate) attack: AttackPolicy,
    pub(crate) resources: ResourcePolicy,
    pub(crate) expansion: Option<ExpansionPolicy>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResourcePolicy {
    pub(crate) oil_after_steel_workers: usize,
    pub(crate) oil_after_full_steel_saturation: bool,
    pub(crate) tank_adaptive: Option<TankResourcePolicy>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TankResourcePolicy {
    pub(crate) max_oil_workers: usize,
    pub(crate) oil_workers_per_factory: usize,
    pub(crate) deficit_response_workers: usize,
}

const RIFLE_ONLY: [EntityKind; 1] = [EntityKind::Rifleman];
const TANK_AND_RIFLE: [EntityKind; 2] = [EntityKind::Tank, EntityKind::Rifleman];
const TANK_SCOUT_RIFLE: [EntityKind; 3] =
    [EntityKind::Tank, EntityKind::ScoutCar, EntityKind::Rifleman];
const TANK_ONLY: [EntityKind; 1] = [EntityKind::Tank];
const SUPPORT_WEAPONS: [EntityKind; 2] = [EntityKind::MachineGunner, EntityKind::AntiTankGun];
const SUPPORT_WEAPONS_AND_RIFLE: [EntityKind; 3] = [
    EntityKind::MachineGunner,
    EntityKind::AntiTankGun,
    EntityKind::Rifleman,
];
const TURTLE_UNITS: [EntityKind; 3] = [
    EntityKind::AntiTankGun,
    EntityKind::MachineGunner,
    EntityKind::Rifleman,
];
const TURTLE_ANTI_TANK: [EntityKind; 1] = [EntityKind::AntiTankGun];
const NO_UPGRADES: [UpgradeKind; 0] = [];
const TURTLE_UPGRADES: [UpgradeKind; 2] =
    [UpgradeKind::Entrenchment, UpgradeKind::AntiTankGunUnlock];

const FAST_TECH_PATH: [EntityKind; 1] = [EntityKind::Barracks];
const FULL_TECH_PATH: [EntityKind; 1] = [EntityKind::Barracks];
const TANK_TECH_PATH: [EntityKind; 5] = [
    EntityKind::Barracks,
    EntityKind::TrainingCentre,
    EntityKind::ResearchComplex,
    EntityKind::Factory,
    EntityKind::Steelworks,
];
const AI_1_0_TANK_TECH_PATH: [EntityKind; 4] = [
    EntityKind::Barracks,
    EntityKind::TrainingCentre,
    EntityKind::ResearchComplex,
    EntityKind::Factory,
];
const SUPPORT_TECH_PATH: [EntityKind; 4] = [
    EntityKind::Barracks,
    EntityKind::TrainingCentre,
    EntityKind::ResearchComplex,
    EntityKind::Steelworks,
];
const TURTLE_TECH_PATH: [EntityKind; 4] = [
    EntityKind::Barracks,
    EntityKind::TrainingCentre,
    EntityKind::ResearchComplex,
    EntityKind::Steelworks,
];

pub(crate) static RIFLE_FLOOD_FAST: AiProfile = AiProfile {
    id: RIFLE_FLOOD_FAST_ID,
    workers: WorkerPolicy {
        steel_saturation_fraction: Ratio::new(1, 2),
        steel_worker_cap: Some(5),
        extra_oil_workers: 0,
        pressure_worker_cap: Some(5),
        pressure_until_complete: Some(EntityKind::Barracks),
    },
    supply: SupplyPolicy {
        free_supply_buffer: 6,
        emergency_depot_threshold: 2,
    },
    buildings: BuildingPolicy {
        barracks_curve: BarracksCurve {
            before_steel_saturation: 2,
            after_steel_saturation: 3,
            banked_steel_threshold: 350,
            banked_steel_step: 250,
            max: 4,
        },
        factory_target: 1,
        proxy_barracks: Some(ProxyBarracksPolicy {
            search_radius_tiles: 28,
            min_enemy_base_distance_tiles: 18,
        }),
        required_tech_path: &FAST_TECH_PATH,
        max_pending_per_kind: 1,
    },
    extra_factories: None,
    production: ProductionPolicy {
        queue_depth: 2,
        unit_priorities: &RIFLE_ONLY,
        save_for_first_tech_unit: None,
        balance_unit_priorities: false,
    },
    upgrade_priorities: &NO_UPGRADES,
    attack: AttackPolicy {
        first_attack_size: 1,
        wave_growth: 0,
        regroup_reset_ticks: 120,
        reissue_cadence_ticks: 30,
        stage_distance_tiles: 8.0,
        unit_kinds: &RIFLE_ONLY,
        required_unit: None,
    },
    resources: ResourcePolicy {
        oil_after_steel_workers: 10,
        oil_after_full_steel_saturation: false,
        tank_adaptive: None,
    },
    expansion: Some(ExpansionPolicy {
        target_city_centres: 2,
        required_complete_building: EntityKind::Factory,
        defensive_unit: EntityKind::Rifleman,
        defensive_unit_count: 0,
        pre_expansion_steel_worker_cap: 12,
        post_expansion_steel_worker_cap: Some(24),
        search_radius_tiles: 6,
        trigger_steel: 500,
        trigger_supply_used: 70,
        blocks_tech_path: false,
        oil_before_steel_in_expansion: false,
        remote_worker_assignment_fallback: false,
    }),
    defensive_machine_gunners: None,
    turtle_defense: None,
    frontal_wave: FrontalWavePolicy::DEFAULT,
    recovery_transition: Some(RecoveryTransitionPolicy {
        completed_building: EntityKind::Barracks,
        delay_unit: EntityKind::Rifleman,
        delay_unit_build_count: 7,
        workers: WorkerPolicy {
            steel_saturation_fraction: Ratio::new(1, 1),
            steel_worker_cap: None,
            extra_oil_workers: 3,
            pressure_worker_cap: None,
            pressure_until_complete: None,
        },
        barracks_curve: BarracksCurve {
            before_steel_saturation: 2,
            after_steel_saturation: 3,
            banked_steel_threshold: 450,
            banked_steel_step: 300,
            max: 4,
        },
        required_tech_path: &SUPPORT_TECH_PATH,
        production: ProductionPolicy {
            queue_depth: 3,
            unit_priorities: &SUPPORT_WEAPONS_AND_RIFLE,
            save_for_first_tech_unit: None,
            balance_unit_priorities: true,
        },
        attack: AttackPolicy {
            first_attack_size: usize::MAX,
            wave_growth: 0,
            regroup_reset_ticks: 540,
            reissue_cadence_ticks: 120,
            stage_distance_tiles: 3.0,
            unit_kinds: &SUPPORT_WEAPONS_AND_RIFLE,
            required_unit: None,
        },
        resources: ResourcePolicy {
            oil_after_steel_workers: 8,
            oil_after_full_steel_saturation: false,
            tank_adaptive: None,
        },
        expansion: Some(ExpansionPolicy {
            target_city_centres: 2,
            required_complete_building: EntityKind::TrainingCentre,
            defensive_unit: EntityKind::MachineGunner,
            defensive_unit_count: 0,
            pre_expansion_steel_worker_cap: usize::MAX,
            post_expansion_steel_worker_cap: Some(28),
            search_radius_tiles: 6,
            trigger_steel: 500,
            trigger_supply_used: 50,
            blocks_tech_path: false,
            oil_before_steel_in_expansion: false,
            remote_worker_assignment_fallback: false,
        }),
    }),
    tech_transition: Some(TechTransitionPolicy {
        // If the proxy rush stalls and we float resources, pivot to tanks so we can break a
        // contained game instead of bleeding riflemen into entrenched defenses.
        resource_float: LATE_TANK_TECH_FLOAT_THRESHOLD,
        required_tech_path: &TANK_TECH_PATH,
        production: ProductionPolicy {
            queue_depth: 2,
            unit_priorities: &TANK_AND_RIFLE,
            save_for_first_tech_unit: Some(EntityKind::Tank),
            balance_unit_priorities: false,
        },
        attack: AttackPolicy {
            first_attack_size: 3,
            wave_growth: 1,
            regroup_reset_ticks: 480,
            reissue_cadence_ticks: 120,
            stage_distance_tiles: 8.0,
            unit_kinds: &TANK_AND_RIFLE,
            required_unit: Some(EntityKind::Tank),
        },
    }),
};

pub(crate) static RIFLE_FLOOD_FULL_SATURATION: AiProfile = AiProfile {
    id: RIFLE_FLOOD_FULL_SATURATION_ID,
    workers: WorkerPolicy {
        steel_saturation_fraction: Ratio::new(1, 1),
        steel_worker_cap: None,
        extra_oil_workers: 6,
        pressure_worker_cap: None,
        pressure_until_complete: None,
    },
    supply: SupplyPolicy {
        free_supply_buffer: 5,
        emergency_depot_threshold: 2,
    },
    buildings: BuildingPolicy {
        barracks_curve: BarracksCurve {
            before_steel_saturation: 1,
            after_steel_saturation: 3,
            banked_steel_threshold: 450,
            banked_steel_step: 250,
            max: 5,
        },
        factory_target: 1,
        proxy_barracks: None,
        required_tech_path: &FULL_TECH_PATH,
        max_pending_per_kind: 1,
    },
    extra_factories: None,
    production: ProductionPolicy {
        queue_depth: 3,
        unit_priorities: &RIFLE_ONLY,
        save_for_first_tech_unit: None,
        balance_unit_priorities: false,
    },
    upgrade_priorities: &NO_UPGRADES,
    attack: AttackPolicy {
        first_attack_size: 3,
        wave_growth: 2,
        regroup_reset_ticks: 480,
        reissue_cadence_ticks: 120,
        stage_distance_tiles: 8.0,
        unit_kinds: &RIFLE_ONLY,
        required_unit: None,
    },
    resources: ResourcePolicy {
        oil_after_steel_workers: 10,
        oil_after_full_steel_saturation: true,
        tank_adaptive: None,
    },
    expansion: Some(ExpansionPolicy {
        target_city_centres: 2,
        required_complete_building: EntityKind::TrainingCentre,
        defensive_unit: EntityKind::Rifleman,
        defensive_unit_count: 0,
        pre_expansion_steel_worker_cap: usize::MAX,
        post_expansion_steel_worker_cap: Some(36),
        search_radius_tiles: 6,
        trigger_steel: 300,
        trigger_supply_used: 30,
        blocks_tech_path: false,
        oil_before_steel_in_expansion: true,
        remote_worker_assignment_fallback: true,
    }),
    defensive_machine_gunners: None,
    turtle_defense: None,
    frontal_wave: FrontalWavePolicy::DEFAULT,
    recovery_transition: None,
    tech_transition: Some(TechTransitionPolicy {
        // Once the rifle flood is floating enough resources, pivot to tanks so a stalemated
        // saturation push doesn't bleed out against superior tech.
        resource_float: TANK_TECH_FLOAT_THRESHOLD,
        required_tech_path: &TANK_TECH_PATH,
        production: ProductionPolicy {
            queue_depth: 2,
            unit_priorities: &TANK_AND_RIFLE,
            save_for_first_tech_unit: Some(EntityKind::Tank),
            balance_unit_priorities: false,
        },
        attack: AttackPolicy {
            first_attack_size: 4,
            wave_growth: 1,
            regroup_reset_ticks: 480,
            reissue_cadence_ticks: 120,
            stage_distance_tiles: 8.0,
            unit_kinds: &TANK_AND_RIFLE,
            required_unit: Some(EntityKind::Tank),
        },
    }),
};

pub(crate) static TECH_TO_TANKS: AiProfile = AiProfile {
    id: TECH_TO_TANKS_ID,
    workers: WorkerPolicy {
        steel_saturation_fraction: Ratio::new(2, 3),
        steel_worker_cap: Some(12),
        extra_oil_workers: 3,
        pressure_worker_cap: None,
        pressure_until_complete: None,
    },
    supply: SupplyPolicy {
        free_supply_buffer: 6,
        emergency_depot_threshold: 2,
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
        proxy_barracks: None,
        required_tech_path: &TANK_TECH_PATH,
        max_pending_per_kind: 1,
    },
    extra_factories: None,
    production: ProductionPolicy {
        queue_depth: 1,
        unit_priorities: &TANK_AND_RIFLE,
        save_for_first_tech_unit: Some(EntityKind::Tank),
        balance_unit_priorities: false,
    },
    upgrade_priorities: &NO_UPGRADES,
    attack: AttackPolicy {
        first_attack_size: 1,
        wave_growth: 1,
        regroup_reset_ticks: 540,
        reissue_cadence_ticks: 120,
        stage_distance_tiles: 8.0,
        unit_kinds: &TANK_AND_RIFLE,
        required_unit: Some(EntityKind::Tank),
    },
    resources: ResourcePolicy {
        oil_after_steel_workers: 8,
        oil_after_full_steel_saturation: false,
        tank_adaptive: None,
    },
    expansion: Some(ExpansionPolicy {
        target_city_centres: 2,
        required_complete_building: EntityKind::Factory,
        defensive_unit: EntityKind::Tank,
        defensive_unit_count: 0,
        pre_expansion_steel_worker_cap: 12,
        post_expansion_steel_worker_cap: Some(24),
        search_radius_tiles: 6,
        trigger_steel: 500,
        trigger_supply_used: 70,
        blocks_tech_path: false,
        oil_before_steel_in_expansion: false,
        remote_worker_assignment_fallback: false,
    }),
    defensive_machine_gunners: None,
    turtle_defense: None,
    frontal_wave: FrontalWavePolicy::DEFAULT,
    recovery_transition: None,
    tech_transition: None,
};

pub(crate) static STEEL_EXPANSION_TANKS: AiProfile = AiProfile {
    id: STEEL_EXPANSION_TANKS_ID,
    workers: WorkerPolicy {
        steel_saturation_fraction: Ratio::new(1, 1),
        steel_worker_cap: Some(24),
        extra_oil_workers: 6,
        pressure_worker_cap: None,
        pressure_until_complete: None,
    },
    supply: SupplyPolicy {
        free_supply_buffer: 8,
        emergency_depot_threshold: 2,
    },
    buildings: BuildingPolicy {
        barracks_curve: BarracksCurve {
            before_steel_saturation: 2,
            after_steel_saturation: 4,
            banked_steel_threshold: 0,
            banked_steel_step: 0,
            max: 4,
        },
        factory_target: 1,
        proxy_barracks: None,
        required_tech_path: &SUPPORT_TECH_PATH,
        max_pending_per_kind: 1,
    },
    extra_factories: None,
    production: ProductionPolicy {
        queue_depth: 3,
        unit_priorities: &SUPPORT_WEAPONS,
        save_for_first_tech_unit: Some(EntityKind::MachineGunner),
        balance_unit_priorities: true,
    },
    upgrade_priorities: &NO_UPGRADES,
    attack: AttackPolicy {
        first_attack_size: usize::MAX,
        wave_growth: 0,
        regroup_reset_ticks: 540,
        reissue_cadence_ticks: 120,
        stage_distance_tiles: 3.0,
        unit_kinds: &SUPPORT_WEAPONS,
        required_unit: None,
    },
    resources: ResourcePolicy {
        oil_after_steel_workers: 8,
        oil_after_full_steel_saturation: false,
        tank_adaptive: None,
    },
    expansion: Some(ExpansionPolicy {
        target_city_centres: 2,
        required_complete_building: EntityKind::CityCentre,
        defensive_unit: EntityKind::MachineGunner,
        defensive_unit_count: 0,
        pre_expansion_steel_worker_cap: 8,
        post_expansion_steel_worker_cap: Some(24),
        search_radius_tiles: 6,
        trigger_steel: 0,
        trigger_supply_used: 0,
        blocks_tech_path: true,
        oil_before_steel_in_expansion: false,
        remote_worker_assignment_fallback: false,
    }),
    defensive_machine_gunners: None,
    turtle_defense: None,
    frontal_wave: FrontalWavePolicy::DEFAULT,
    recovery_transition: None,
    tech_transition: Some(TechTransitionPolicy {
        resource_float: SUPPORT_TO_TANK_FLOAT_THRESHOLD,
        required_tech_path: &TANK_TECH_PATH,
        production: ProductionPolicy {
            queue_depth: 2,
            unit_priorities: &TANK_ONLY,
            save_for_first_tech_unit: Some(EntityKind::Tank),
            balance_unit_priorities: false,
        },
        attack: AttackPolicy {
            first_attack_size: 3,
            wave_growth: 0,
            regroup_reset_ticks: 540,
            reissue_cadence_ticks: 120,
            stage_distance_tiles: 8.0,
            unit_kinds: &TANK_ONLY,
            required_unit: Some(EntityKind::Tank),
        },
    }),
};

pub(crate) static AI_1_0_TECH: AiProfile = AiProfile {
    id: AI_1_0_TECH_ID,
    workers: WorkerPolicy {
        steel_saturation_fraction: Ratio::new(1, 1),
        steel_worker_cap: Some(18),
        extra_oil_workers: 6,
        pressure_worker_cap: None,
        pressure_until_complete: None,
    },
    supply: SupplyPolicy {
        free_supply_buffer: 7,
        emergency_depot_threshold: 2,
    },
    buildings: BuildingPolicy {
        barracks_curve: BarracksCurve {
            before_steel_saturation: 1,
            after_steel_saturation: 2,
            banked_steel_threshold: 550,
            banked_steel_step: 350,
            max: 3,
        },
        factory_target: 1,
        proxy_barracks: None,
        required_tech_path: &FULL_TECH_PATH,
        max_pending_per_kind: 1,
    },
    extra_factories: None,
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
        oil_after_steel_workers: 8,
        oil_after_full_steel_saturation: false,
        tank_adaptive: None,
    },
    expansion: Some(ExpansionPolicy {
        target_city_centres: 2,
        required_complete_building: EntityKind::TrainingCentre,
        defensive_unit: EntityKind::Rifleman,
        defensive_unit_count: 4,
        pre_expansion_steel_worker_cap: 14,
        post_expansion_steel_worker_cap: Some(30),
        search_radius_tiles: 6,
        trigger_steel: 350,
        trigger_supply_used: 30,
        blocks_tech_path: false,
        oil_before_steel_in_expansion: true,
        remote_worker_assignment_fallback: true,
    }),
    defensive_machine_gunners: None,
    turtle_defense: None,
    frontal_wave: FrontalWavePolicy::DEFAULT,
    recovery_transition: None,
    tech_transition: Some(TechTransitionPolicy {
        resource_float: TANK_TECH_FLOAT_THRESHOLD,
        required_tech_path: &AI_1_0_TANK_TECH_PATH,
        production: ProductionPolicy {
            queue_depth: 2,
            unit_priorities: &TANK_SCOUT_RIFLE,
            save_for_first_tech_unit: Some(EntityKind::Tank),
            balance_unit_priorities: false,
        },
        attack: AttackPolicy {
            first_attack_size: 6,
            wave_growth: 2,
            regroup_reset_ticks: 480,
            reissue_cadence_ticks: 120,
            stage_distance_tiles: 8.0,
            unit_kinds: &TANK_SCOUT_RIFLE,
            required_unit: Some(EntityKind::Tank),
        },
    }),
};

pub(crate) static AI_1_1_TANK_MG: AiProfile = AiProfile {
    id: AI_1_1_TANK_MG_ID,
    workers: WorkerPolicy {
        steel_saturation_fraction: Ratio::new(1, 1),
        steel_worker_cap: None,
        extra_oil_workers: 6,
        pressure_worker_cap: None,
        pressure_until_complete: None,
    },
    supply: AI_1_0_TECH.supply,
    buildings: BuildingPolicy {
        barracks_curve: BarracksCurve {
            before_steel_saturation: 1,
            after_steel_saturation: 1,
            banked_steel_threshold: 0,
            banked_steel_step: 0,
            max: 1,
        },
        factory_target: 1,
        proxy_barracks: None,
        required_tech_path: &FULL_TECH_PATH,
        max_pending_per_kind: 1,
    },
    extra_factories: None,
    production: AI_1_0_TECH.production,
    upgrade_priorities: &NO_UPGRADES,
    attack: AI_1_0_TECH.attack,
    resources: AI_1_0_TECH.resources,
    expansion: Some(ExpansionPolicy {
        target_city_centres: 2,
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
    defensive_machine_gunners: Some(DefensiveMachineGunnerPolicy { target_count: 4 }),
    turtle_defense: None,
    frontal_wave: FrontalWavePolicy::DEFAULT,
    recovery_transition: None,
    tech_transition: Some(TechTransitionPolicy {
        resource_float: TANK_TECH_FLOAT_THRESHOLD,
        required_tech_path: &AI_1_0_TANK_TECH_PATH,
        production: ProductionPolicy {
            queue_depth: 2,
            unit_priorities: &TANK_ONLY,
            save_for_first_tech_unit: Some(EntityKind::Tank),
            balance_unit_priorities: false,
        },
        attack: AttackPolicy {
            first_attack_size: 1,
            wave_growth: 2,
            regroup_reset_ticks: 480,
            reissue_cadence_ticks: 120,
            stage_distance_tiles: 8.0,
            unit_kinds: &TANK_ONLY,
            required_unit: Some(EntityKind::Tank),
        },
    }),
};

pub(crate) static AI_1_2_WAVE_COHORTS: AiProfile = AiProfile {
    id: AI_1_2_WAVE_COHORTS_ID,
    workers: AI_1_1_TANK_MG.workers,
    supply: AI_1_1_TANK_MG.supply,
    buildings: AI_1_1_TANK_MG.buildings,
    extra_factories: Some(ExtraFactoryPolicy {
        target_count: 2,
        resource_float: AI_1_2_SECOND_FACTORY_FLOAT_THRESHOLD,
    }),
    production: AI_1_1_TANK_MG.production,
    upgrade_priorities: &NO_UPGRADES,
    attack: AI_1_1_TANK_MG.attack,
    resources: AI_1_1_TANK_MG.resources,
    expansion: AI_1_1_TANK_MG.expansion,
    defensive_machine_gunners: AI_1_1_TANK_MG.defensive_machine_gunners,
    turtle_defense: None,
    frontal_wave: FrontalWavePolicy {
        exclude_launched_ticks: Some(AI_1_2_FRONTAL_COHORT_TICKS),
        line_staging: true,
    },
    recovery_transition: AI_1_1_TANK_MG.recovery_transition,
    tech_transition: AI_1_1_TANK_MG.tech_transition,
};

pub(crate) static AI_2_0_TANK_PRESSURE: AiProfile = AiProfile {
    id: AI_2_0_TANK_PRESSURE_ID,
    workers: WorkerPolicy {
        steel_saturation_fraction: Ratio::new(1, 1),
        steel_worker_cap: None,
        extra_oil_workers: 12,
        pressure_worker_cap: None,
        pressure_until_complete: None,
    },
    supply: SupplyPolicy {
        free_supply_buffer: 8,
        emergency_depot_threshold: 3,
    },
    buildings: BuildingPolicy {
        barracks_curve: AI_1_1_TANK_MG.buildings.barracks_curve,
        factory_target: 1,
        proxy_barracks: None,
        required_tech_path: AI_1_1_TANK_MG.buildings.required_tech_path,
        max_pending_per_kind: AI_1_1_TANK_MG.buildings.max_pending_per_kind,
    },
    extra_factories: Some(ExtraFactoryPolicy {
        target_count: 2,
        resource_float: AI_2_0_SECOND_FACTORY_FLOAT_THRESHOLD,
    }),
    production: AI_1_0_TECH.production,
    upgrade_priorities: &NO_UPGRADES,
    attack: AI_1_0_TECH.attack,
    resources: ResourcePolicy {
        oil_after_steel_workers: 5,
        oil_after_full_steel_saturation: false,
        tank_adaptive: Some(TankResourcePolicy {
            max_oil_workers: 12,
            oil_workers_per_factory: 6,
            deficit_response_workers: 2,
        }),
    },
    expansion: Some(ExpansionPolicy {
        target_city_centres: 2,
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
    defensive_machine_gunners: Some(DefensiveMachineGunnerPolicy { target_count: 4 }),
    turtle_defense: None,
    frontal_wave: FrontalWavePolicy {
        exclude_launched_ticks: Some(AI_1_2_FRONTAL_COHORT_TICKS),
        line_staging: true,
    },
    recovery_transition: None,
    tech_transition: Some(TechTransitionPolicy {
        resource_float: AI_2_0_TANK_PRESSURE_FLOAT_THRESHOLD,
        required_tech_path: &AI_1_0_TANK_TECH_PATH,
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
};

pub(crate) static AI_2_1_ECONOMY_MANAGER: AiProfile = AiProfile {
    id: AI_2_1_ECONOMY_MANAGER_ID,
    ..AI_2_0_TANK_PRESSURE
};

pub(crate) static AI_TURTLE_CHOKES: AiProfile = AiProfile {
    id: AI_TURTLE_CHOKES_ID,
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
            banked_steel_threshold: 0,
            banked_steel_step: 0,
            max: 1,
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
        defensive_unit_count: 3,
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

pub(crate) fn required_profiles() -> [&'static AiProfile; 6] {
    [
        &AI_1_0_TECH,
        &AI_1_1_TANK_MG,
        &AI_1_2_WAVE_COHORTS,
        &AI_2_0_TANK_PRESSURE,
        &AI_2_1_ECONOMY_MANAGER,
        &AI_TURTLE_CHOKES,
    ]
}

pub(crate) fn profile_by_id(id: &str) -> Option<&'static AiProfile> {
    required_profiles()
        .into_iter()
        .find(|profile| profile.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_profile_ids_are_stable_and_deterministic() {
        let profiles = required_profiles();

        assert_eq!(
            profiles.map(|profile| profile.id),
            [
                AI_1_0_TECH_ID,
                AI_1_1_TANK_MG_ID,
                AI_1_2_WAVE_COHORTS_ID,
                AI_2_0_TANK_PRESSURE_ID,
                AI_2_1_ECONOMY_MANAGER_ID,
                AI_TURTLE_CHOKES_ID,
            ]
        );
        assert_eq!(profile_by_id(AI_1_0_TECH_ID).unwrap().id, AI_1_0_TECH_ID);
        assert_eq!(
            profile_by_id(AI_1_1_TANK_MG_ID).unwrap().id,
            AI_1_1_TANK_MG_ID
        );
        assert_eq!(
            profile_by_id(AI_1_2_WAVE_COHORTS_ID).unwrap().id,
            AI_1_2_WAVE_COHORTS_ID
        );
        assert_eq!(
            profile_by_id(AI_2_0_TANK_PRESSURE_ID).unwrap().id,
            AI_2_0_TANK_PRESSURE_ID
        );
        assert_eq!(
            profile_by_id(AI_2_1_ECONOMY_MANAGER_ID).unwrap().id,
            AI_2_1_ECONOMY_MANAGER_ID
        );
        assert_eq!(
            profile_by_id(AI_TURTLE_CHOKES_ID).unwrap().id,
            AI_TURTLE_CHOKES_ID
        );
        assert!(profile_by_id("tech_tree").is_none());
    }

    #[test]
    fn current_ai_profiles_do_not_train_specialist_first_pass_units() {
        let omitted_units = [EntityKind::Panzerfaust, EntityKind::ScoutPlane];
        for profile in [
            &RIFLE_FLOOD_FAST,
            &RIFLE_FLOOD_FULL_SATURATION,
            &TECH_TO_TANKS,
            &STEEL_EXPANSION_TANKS,
            &AI_1_0_TECH,
            &AI_1_1_TANK_MG,
            &AI_1_2_WAVE_COHORTS,
            &AI_2_0_TANK_PRESSURE,
            &AI_2_1_ECONOMY_MANAGER,
            &AI_TURTLE_CHOKES,
        ] {
            for unit in omitted_units {
                assert!(
                    !profile.production.unit_priorities.contains(&unit),
                    "{} base production must not train {unit:?} in the first pass",
                    profile.id
                );
                if let Some(transition) = profile.tech_transition {
                    assert!(
                        !transition.production.unit_priorities.contains(&unit),
                        "{} tech transition must not train {unit:?} in the first pass",
                        profile.id
                    );
                }
                if let Some(recovery) = profile.recovery_transition {
                    assert!(
                        !recovery.production.unit_priorities.contains(&unit),
                        "{} recovery transition must not train {unit:?} in the first pass",
                        profile.id
                    );
                }
            }
        }
    }

    #[test]
    fn retired_ai_2_0_agent_rush_profile_id_is_not_registered() {
        assert!(profile_by_id("ai_2_0_agent_rush").is_none());
    }

    #[test]
    fn retired_ai_2_0_rifle_tank_profile_id_is_not_registered() {
        assert!(profile_by_id("ai_2_0_rifle_tank").is_none());
    }

    #[test]
    fn ai_2_0_tank_pressure_is_distinct_from_ai_1_2() {
        let transition = AI_2_0_TANK_PRESSURE.tech_transition.unwrap();
        let expansion = AI_2_0_TANK_PRESSURE.expansion.unwrap();

        assert_eq!(AI_2_0_TANK_PRESSURE.id, AI_2_0_TANK_PRESSURE_ID);
        assert_eq!(
            AI_2_0_TANK_PRESSURE.buildings.barracks_curve,
            AI_1_1_TANK_MG.buildings.barracks_curve
        );
        assert_eq!(AI_2_0_TANK_PRESSURE.buildings.factory_target, 1);
        assert_eq!(
            AI_2_0_TANK_PRESSURE.extra_factories,
            Some(ExtraFactoryPolicy {
                target_count: 2,
                resource_float: AI_2_0_SECOND_FACTORY_FLOAT_THRESHOLD,
            })
        );
        assert_eq!(AI_2_0_TANK_PRESSURE.workers.extra_oil_workers, 12);
        assert_eq!(AI_2_0_TANK_PRESSURE.resources.oil_after_steel_workers, 5);
        assert_eq!(
            AI_2_0_TANK_PRESSURE.resources.tank_adaptive,
            Some(TankResourcePolicy {
                max_oil_workers: 12,
                oil_workers_per_factory: 6,
                deficit_response_workers: 2,
            })
        );
        assert_eq!(
            expansion.required_complete_building,
            EntityKind::TrainingCentre
        );
        assert_eq!(expansion.defensive_unit, EntityKind::Rifleman);
        assert_eq!(
            transition.resource_float,
            AI_2_0_TANK_PRESSURE_FLOAT_THRESHOLD
        );
        assert_eq!(transition.required_tech_path, &AI_1_0_TANK_TECH_PATH);
        assert_eq!(
            transition.production.unit_priorities,
            &[EntityKind::Tank, EntityKind::Rifleman]
        );
        assert_eq!(transition.production.save_for_first_tech_unit, None);
        assert_eq!(transition.attack.first_attack_size, 2);
        assert_eq!(transition.attack.required_unit, None);
        assert_eq!(
            AI_2_0_TANK_PRESSURE.defensive_machine_gunners,
            Some(DefensiveMachineGunnerPolicy { target_count: 4 })
        );
    }

    #[test]
    fn ai_2_1_keeps_ai_2_0_policy_values_for_manager_refactor_baseline() {
        assert_eq!(AI_2_1_ECONOMY_MANAGER.id, AI_2_1_ECONOMY_MANAGER_ID);
        assert_eq!(AI_2_1_ECONOMY_MANAGER.workers, AI_2_0_TANK_PRESSURE.workers);
        assert_eq!(AI_2_1_ECONOMY_MANAGER.supply, AI_2_0_TANK_PRESSURE.supply);
        assert_eq!(AI_2_1_ECONOMY_MANAGER.buildings, AI_2_0_TANK_PRESSURE.buildings);
        assert_eq!(
            AI_2_1_ECONOMY_MANAGER.extra_factories,
            AI_2_0_TANK_PRESSURE.extra_factories
        );
        assert_eq!(
            AI_2_1_ECONOMY_MANAGER.production,
            AI_2_0_TANK_PRESSURE.production
        );
        assert_eq!(
            AI_2_1_ECONOMY_MANAGER.resources,
            AI_2_0_TANK_PRESSURE.resources
        );
        assert_eq!(
            AI_2_1_ECONOMY_MANAGER.expansion,
            AI_2_0_TANK_PRESSURE.expansion
        );
        assert_eq!(
            AI_2_1_ECONOMY_MANAGER.tech_transition,
            AI_2_0_TANK_PRESSURE.tech_transition
        );
    }

    #[test]
    fn ai_1_1_forks_ai_1_0_without_scout_cars_extra_barracks_or_second_factory() {
        let transition = AI_1_1_TANK_MG.tech_transition.unwrap();
        let expansion = AI_1_1_TANK_MG.expansion.unwrap();

        assert_eq!(AI_1_1_TANK_MG.supply, AI_1_0_TECH.supply);
        assert_eq!(AI_1_1_TANK_MG.resources, AI_1_0_TECH.resources);
        assert_eq!(AI_1_1_TANK_MG.workers.steel_worker_cap, None);
        assert_eq!(expansion.pre_expansion_steel_worker_cap, 18);
        assert_eq!(expansion.post_expansion_steel_worker_cap, Some(36));
        assert_eq!(AI_1_1_TANK_MG.buildings.barracks_curve.max, 1);
        assert_eq!(AI_1_1_TANK_MG.buildings.factory_target, 1);
        assert_eq!(AI_1_1_TANK_MG.extra_factories, None);
        assert_eq!(
            AI_1_1_TANK_MG
                .buildings
                .barracks_curve
                .target(2_000, 30, 18),
            1
        );
        assert_eq!(transition.production.unit_priorities, &[EntityKind::Tank]);
        assert!(!transition
            .production
            .unit_priorities
            .contains(&EntityKind::ScoutCar));
        assert_eq!(
            transition.production.save_for_first_tech_unit,
            Some(EntityKind::Tank)
        );
        assert_eq!(transition.attack.required_unit, Some(EntityKind::Tank));
        assert_eq!(transition.attack.unit_kinds, &[EntityKind::Tank]);
        assert_eq!(transition.attack.first_attack_size, 1);
        assert_eq!(transition.resource_float, TANK_TECH_FLOAT_THRESHOLD);
        assert_eq!(
            AI_1_1_TANK_MG.defensive_machine_gunners,
            Some(DefensiveMachineGunnerPolicy { target_count: 4 })
        );
    }

    #[test]
    fn ai_1_2_forks_ai_1_1_with_frontal_wave_cohorts() {
        assert_eq!(AI_1_2_WAVE_COHORTS.workers, AI_1_1_TANK_MG.workers);
        assert_eq!(AI_1_2_WAVE_COHORTS.supply, AI_1_1_TANK_MG.supply);
        assert_eq!(AI_1_2_WAVE_COHORTS.buildings, AI_1_1_TANK_MG.buildings);
        assert_eq!(
            AI_1_2_WAVE_COHORTS.extra_factories,
            Some(ExtraFactoryPolicy {
                target_count: 2,
                resource_float: AI_1_2_SECOND_FACTORY_FLOAT_THRESHOLD,
            })
        );
        assert_eq!(AI_1_2_WAVE_COHORTS.production, AI_1_1_TANK_MG.production);
        assert_eq!(AI_1_2_WAVE_COHORTS.attack, AI_1_1_TANK_MG.attack);
        assert_eq!(AI_1_2_WAVE_COHORTS.resources, AI_1_1_TANK_MG.resources);
        assert_eq!(AI_1_2_WAVE_COHORTS.expansion, AI_1_1_TANK_MG.expansion);
        assert_eq!(
            AI_1_2_WAVE_COHORTS.defensive_machine_gunners,
            AI_1_1_TANK_MG.defensive_machine_gunners
        );
        assert_eq!(
            AI_1_2_WAVE_COHORTS.tech_transition,
            AI_1_1_TANK_MG.tech_transition
        );
        assert_eq!(
            AI_1_2_WAVE_COHORTS.frontal_wave,
            FrontalWavePolicy {
                exclude_launched_ticks: Some(AI_1_2_FRONTAL_COHORT_TICKS),
                line_staging: true,
            }
        );
    }

    #[test]
    fn fast_flood_attacks_with_smaller_waves_than_full_saturation() {
        assert_eq!(RIFLE_FLOOD_FAST.attack.first_attack_size, 1);
        assert_eq!(RIFLE_FLOOD_FAST.attack.wave_growth, 0);
        assert_eq!(RIFLE_FLOOD_FULL_SATURATION.attack.first_attack_size, 3);
        assert!(
            RIFLE_FLOOD_FAST.production.queue_depth
                < RIFLE_FLOOD_FULL_SATURATION.production.queue_depth
        );
        assert!(RIFLE_FLOOD_FAST.buildings.proxy_barracks.is_some());
        let recovery = RIFLE_FLOOD_FAST.recovery_transition.unwrap();
        assert_eq!(recovery.completed_building, EntityKind::Barracks);
        assert_eq!(recovery.delay_unit, EntityKind::Rifleman);
        assert_eq!(recovery.delay_unit_build_count, 7);
        assert_eq!(recovery.workers.steel_worker_cap, None);
        assert_eq!(recovery.workers.extra_oil_workers, 3);
        assert_eq!(
            recovery.required_tech_path,
            &[
                EntityKind::Barracks,
                EntityKind::TrainingCentre,
                EntityKind::ResearchComplex,
                EntityKind::Steelworks
            ]
        );
    }

    #[test]
    fn tech_to_tanks_has_oil_workers_and_factory_path() {
        assert_eq!(TECH_TO_TANKS.workers.extra_oil_workers, 3);
        assert_eq!(TECH_TO_TANKS.resources.oil_after_steel_workers, 8);
        assert_eq!(TECH_TO_TANKS.resources.tank_adaptive, None);
        assert_eq!(
            TECH_TO_TANKS.buildings.required_tech_path,
            &[
                EntityKind::Barracks,
                EntityKind::TrainingCentre,
                EntityKind::ResearchComplex,
                EntityKind::Factory,
                EntityKind::Steelworks
            ]
        );
        assert_eq!(
            TECH_TO_TANKS.production.save_for_first_tech_unit,
            Some(EntityKind::Tank)
        );
        assert_eq!(TECH_TO_TANKS.attack.first_attack_size, 1);
    }

    #[test]
    fn full_saturation_can_staff_oil_and_pivot_before_ultra_late_supply() {
        let expansion = RIFLE_FLOOD_FULL_SATURATION.expansion.unwrap();
        let transition = RIFLE_FLOOD_FULL_SATURATION.tech_transition.unwrap();

        assert_eq!(RIFLE_FLOOD_FULL_SATURATION.workers.extra_oil_workers, 6);
        assert_eq!(
            RIFLE_FLOOD_FULL_SATURATION
                .resources
                .oil_after_steel_workers,
            10
        );
        assert!(
            RIFLE_FLOOD_FULL_SATURATION
                .resources
                .oil_after_full_steel_saturation
        );
        assert_eq!(
            expansion.required_complete_building,
            EntityKind::TrainingCentre
        );
        assert_eq!(expansion.pre_expansion_steel_worker_cap, usize::MAX);
        assert_eq!(expansion.trigger_supply_used, 30);
        assert_eq!(transition.resource_float, TANK_TECH_FLOAT_THRESHOLD);
        assert_eq!(
            transition.required_tech_path,
            &[
                EntityKind::Barracks,
                EntityKind::TrainingCentre,
                EntityKind::ResearchComplex,
                EntityKind::Factory,
                EntityKind::Steelworks
            ]
        );
        assert_eq!(
            transition.production.save_for_first_tech_unit,
            Some(EntityKind::Tank)
        );
    }

    #[test]
    fn steel_expansion_tanks_expands_before_support_tech() {
        let expansion = STEEL_EXPANSION_TANKS.expansion.unwrap();

        assert_eq!(STEEL_EXPANSION_TANKS.buildings.barracks_curve.max, 4);
        assert_eq!(
            STEEL_EXPANSION_TANKS
                .buildings
                .barracks_curve
                .banked_steel_threshold,
            0
        );
        assert_eq!(
            STEEL_EXPANSION_TANKS.buildings.required_tech_path,
            &[
                EntityKind::Barracks,
                EntityKind::TrainingCentre,
                EntityKind::ResearchComplex,
                EntityKind::Steelworks
            ]
        );
        assert_eq!(
            STEEL_EXPANSION_TANKS.production.unit_priorities,
            &[EntityKind::MachineGunner, EntityKind::AntiTankGun]
        );
        assert!(STEEL_EXPANSION_TANKS.production.balance_unit_priorities);
        assert_eq!(expansion.target_city_centres, 2);
        assert_eq!(expansion.required_complete_building, EntityKind::CityCentre);
        assert_eq!(expansion.defensive_unit_count, 0);
        assert_eq!(expansion.pre_expansion_steel_worker_cap, 8);
        assert_eq!(expansion.post_expansion_steel_worker_cap, Some(24));
        assert_eq!(STEEL_EXPANSION_TANKS.workers.extra_oil_workers, 6);
        assert_eq!(STEEL_EXPANSION_TANKS.attack.first_attack_size, usize::MAX);
        assert!(STEEL_EXPANSION_TANKS.resources.tank_adaptive.is_none());
        let transition = STEEL_EXPANSION_TANKS.tech_transition.unwrap();
        assert_eq!(transition.resource_float, SUPPORT_TO_TANK_FLOAT_THRESHOLD);
        assert_eq!(
            transition.required_tech_path,
            &[
                EntityKind::Barracks,
                EntityKind::TrainingCentre,
                EntityKind::ResearchComplex,
                EntityKind::Factory,
                EntityKind::Steelworks
            ]
        );
        assert_eq!(transition.production.unit_priorities, &[EntityKind::Tank]);
        assert_eq!(
            transition.production.save_for_first_tech_unit,
            Some(EntityKind::Tank)
        );
        assert_eq!(transition.attack.first_attack_size, 3);
        assert_eq!(transition.attack.unit_kinds, &[EntityKind::Tank]);
    }

    #[test]
    fn full_saturation_requests_more_workers_before_pressure() {
        let fast_target = RIFLE_FLOOD_FAST.workers.target_steel_workers(18, 0);
        let full_target = RIFLE_FLOOD_FULL_SATURATION
            .workers
            .target_steel_workers(18, 0);

        assert_eq!(fast_target, 5);
        assert_eq!(full_target, 18);
    }
}
