#![allow(dead_code)]

use rts_sim::game::entity::EntityKind;

pub(crate) const RIFLE_FLOOD_FAST_ID: &str = "rifle_flood_fast";
pub(crate) const RIFLE_FLOOD_FULL_SATURATION_ID: &str = "rifle_flood_full_saturation";
pub(crate) const TECH_TO_TANKS_ID: &str = "tech_to_tanks";
pub(crate) const STEEL_EXPANSION_TANKS_ID: &str = "steel_expansion_tanks";
pub(crate) const AI_1_0_TECH_ID: &str = "ai_1_0_tech";

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AiProfile {
    pub(crate) id: &'static str,
    pub(crate) workers: WorkerPolicy,
    pub(crate) supply: SupplyPolicy,
    pub(crate) buildings: BuildingPolicy,
    pub(crate) production: ProductionPolicy,
    pub(crate) attack: AttackPolicy,
    pub(crate) harassment: Option<HarassmentPolicy>,
    pub(crate) resources: ResourcePolicy,
    pub(crate) expansion: Option<ExpansionPolicy>,
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
    pub(crate) proxy_barracks: Option<ProxyBarracksPolicy>,
    pub(crate) required_tech_path: &'static [EntityKind],
    pub(crate) max_pending_per_kind: usize,
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HarassmentPolicy {
    pub(crate) unit_kind: EntityKind,
    pub(crate) group_size: usize,
    pub(crate) reissue_cadence_ticks: u32,
    pub(crate) back_offset_tiles: f32,
    pub(crate) side_offset_tiles: f32,
    pub(crate) visible_threat_radius_tiles: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TechTransitionPolicy {
    pub(crate) supply_used_threshold: u32,
    pub(crate) required_tech_path: &'static [EntityKind],
    pub(crate) production: ProductionPolicy,
    pub(crate) attack: AttackPolicy,
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
const TANK_SCOUT_RIFLE: [EntityKind; 3] = [
    EntityKind::Tank,
    EntityKind::ScoutCar,
    EntityKind::Rifleman,
];
const TANK_ONLY: [EntityKind; 1] = [EntityKind::Tank];
const SUPPORT_WEAPONS: [EntityKind; 2] = [EntityKind::MachineGunner, EntityKind::AntiTankGun];
const SUPPORT_WEAPONS_AND_RIFLE: [EntityKind; 3] = [
    EntityKind::MachineGunner,
    EntityKind::AntiTankGun,
    EntityKind::Rifleman,
];

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
        proxy_barracks: Some(ProxyBarracksPolicy {
            search_radius_tiles: 28,
            min_enemy_base_distance_tiles: 18,
        }),
        required_tech_path: &FAST_TECH_PATH,
        max_pending_per_kind: 1,
    },
    production: ProductionPolicy {
        queue_depth: 2,
        unit_priorities: &RIFLE_ONLY,
        save_for_first_tech_unit: None,
        balance_unit_priorities: false,
    },
    attack: AttackPolicy {
        first_attack_size: 1,
        wave_growth: 0,
        regroup_reset_ticks: 120,
        reissue_cadence_ticks: 30,
        stage_distance_tiles: 8.0,
        unit_kinds: &RIFLE_ONLY,
        required_unit: None,
    },
    harassment: None,
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
    }),
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
        }),
    }),
    tech_transition: Some(TechTransitionPolicy {
        // If the proxy rush stalls and we accumulate supply, pivot to tanks so we can break a
        // contained game instead of bleeding riflemen into entrenched defenses.
        supply_used_threshold: 70,
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
        proxy_barracks: None,
        required_tech_path: &FULL_TECH_PATH,
        max_pending_per_kind: 1,
    },
    production: ProductionPolicy {
        queue_depth: 3,
        unit_priorities: &RIFLE_ONLY,
        save_for_first_tech_unit: None,
        balance_unit_priorities: false,
    },
    attack: AttackPolicy {
        first_attack_size: 3,
        wave_growth: 2,
        regroup_reset_ticks: 480,
        reissue_cadence_ticks: 120,
        stage_distance_tiles: 8.0,
        unit_kinds: &RIFLE_ONLY,
        required_unit: None,
    },
    harassment: None,
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
    }),
    recovery_transition: None,
    tech_transition: Some(TechTransitionPolicy {
        // Once the rifle flood has put real bodies on the field, pivot to tanks so a stalemated
        // saturation push doesn't bleed out against superior tech.
        supply_used_threshold: 50,
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
        proxy_barracks: None,
        required_tech_path: &TANK_TECH_PATH,
        max_pending_per_kind: 1,
    },
    production: ProductionPolicy {
        queue_depth: 1,
        unit_priorities: &TANK_AND_RIFLE,
        save_for_first_tech_unit: Some(EntityKind::Tank),
        balance_unit_priorities: false,
    },
    attack: AttackPolicy {
        first_attack_size: 1,
        wave_growth: 1,
        regroup_reset_ticks: 540,
        reissue_cadence_ticks: 120,
        stage_distance_tiles: 8.0,
        unit_kinds: &TANK_AND_RIFLE,
        required_unit: Some(EntityKind::Tank),
    },
    harassment: None,
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
    }),
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
        proxy_barracks: None,
        required_tech_path: &SUPPORT_TECH_PATH,
        max_pending_per_kind: 1,
    },
    production: ProductionPolicy {
        queue_depth: 3,
        unit_priorities: &SUPPORT_WEAPONS,
        save_for_first_tech_unit: Some(EntityKind::MachineGunner),
        balance_unit_priorities: true,
    },
    attack: AttackPolicy {
        first_attack_size: usize::MAX,
        wave_growth: 0,
        regroup_reset_ticks: 540,
        reissue_cadence_ticks: 120,
        stage_distance_tiles: 3.0,
        unit_kinds: &SUPPORT_WEAPONS,
        required_unit: None,
    },
    harassment: None,
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
    }),
    recovery_transition: None,
    tech_transition: Some(TechTransitionPolicy {
        supply_used_threshold: 50,
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
        proxy_barracks: None,
        required_tech_path: &FULL_TECH_PATH,
        max_pending_per_kind: 1,
    },
    production: ProductionPolicy {
        queue_depth: 2,
        unit_priorities: &RIFLE_ONLY,
        save_for_first_tech_unit: None,
        balance_unit_priorities: false,
    },
    attack: AttackPolicy {
        first_attack_size: 4,
        wave_growth: 2,
        regroup_reset_ticks: 480,
        reissue_cadence_ticks: 120,
        stage_distance_tiles: 8.0,
        unit_kinds: &RIFLE_ONLY,
        required_unit: None,
    },
    harassment: Some(HarassmentPolicy {
        unit_kind: EntityKind::ScoutCar,
        group_size: 2,
        reissue_cadence_ticks: 90,
        back_offset_tiles: 8.0,
        side_offset_tiles: 12.0,
        visible_threat_radius_tiles: 12.0,
    }),
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
    }),
    recovery_transition: None,
    tech_transition: Some(TechTransitionPolicy {
        supply_used_threshold: 30,
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

pub(crate) fn required_profiles() -> [&'static AiProfile; 1] {
    [&AI_1_0_TECH]
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
            [AI_1_0_TECH_ID]
        );
        assert_eq!(profile_by_id(AI_1_0_TECH_ID).unwrap().id, AI_1_0_TECH_ID);
        assert!(profile_by_id("tech_tree").is_none());
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
        assert_eq!(transition.supply_used_threshold, 50);
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
        assert_eq!(transition.supply_used_threshold, 50);
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
