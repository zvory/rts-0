use rts_sim::game::entity::EntityKind;
use rts_sim::game::upgrade::UpgradeKind;

mod jeffs_ai;
mod turtle;

pub(crate) use self::jeffs_ai::{JEFFS_AI, JEFFS_AI_ID};
pub(crate) use self::turtle::AI_TURTLE;

/// Canonical identities are also the only accepted persisted/profile-selection ids.
pub(crate) const AI_2_1_ID: &str = "ai_2_1";
pub(crate) const AI_TURTLE_ID: &str = "ai_turtle";

const FRONTAL_COHORT_TICKS: u32 = 3_600;
const AI_2_1_TANK_PRESSURE_FLOAT_THRESHOLD: ResourceFloatThreshold = ResourceFloatThreshold {
    steel: 275,
    oil: 100,
};
const AI_2_1_SECOND_FACTORY_FLOAT_THRESHOLD: ResourceFloatThreshold = ResourceFloatThreshold {
    steel: 500,
    oil: 325,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AiProfile {
    pub(crate) id: &'static str,
    pub(crate) workers: WorkerPolicy,
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
    pub(crate) tech_transition: Option<TechTransitionPolicy>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WorkerPolicy {
    pub(crate) steel_saturation_fraction: Ratio,
    pub(crate) steel_worker_cap: Option<usize>,
    pub(crate) extra_oil_workers: usize,
}

impl WorkerPolicy {
    pub(crate) fn target_steel_workers(self, main_base_steel_saturation: usize) -> usize {
        let mut target = self
            .steel_saturation_fraction
            .apply_ceil(main_base_steel_saturation);
        if let Some(cap) = self.steel_worker_cap {
            target = target.min(cap);
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
pub(crate) struct BuildingPolicy {
    pub(crate) barracks_curve: BarracksCurve,
    pub(crate) factory_target: usize,
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
    pub(crate) gun_works_target: usize,
    pub(crate) gun_works_resource_float: ResourceFloatThreshold,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResourcePolicy {
    pub(crate) oil_after_steel_workers: usize,
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
const NO_UPGRADES: [UpgradeKind; 0] = [];
const BASE_TECH_PATH: [EntityKind; 1] = [EntityKind::Barracks];
const TANK_TECH_PATH: [EntityKind; 4] = [
    EntityKind::Barracks,
    EntityKind::TrainingCentre,
    EntityKind::ResearchComplex,
    EntityKind::Factory,
];

/// The promoted pressure profile. Its policy values deliberately preserve the prior AI 2.1
/// behavior, but the profile now owns them directly rather than inheriting a retired AI 2.0.
pub(crate) static AI_2_1: AiProfile = AiProfile {
    id: AI_2_1_ID,
    workers: WorkerPolicy {
        steel_saturation_fraction: Ratio::new(1, 1),
        steel_worker_cap: None,
        extra_oil_workers: 12,
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
        resource_float: AI_2_1_SECOND_FACTORY_FLOAT_THRESHOLD,
    }),
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
        exclude_launched_ticks: Some(FRONTAL_COHORT_TICKS),
        line_staging: true,
    },
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
};

pub(crate) fn required_profiles() -> [&'static AiProfile; 3] {
    [&AI_2_1, &JEFFS_AI, &AI_TURTLE]
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
    fn required_profile_ids_are_canonical_and_deterministic() {
        assert_eq!(
            required_profiles().map(|profile| profile.id),
            [AI_2_1_ID, JEFFS_AI_ID, AI_TURTLE_ID]
        );
        assert_eq!(profile_by_id(AI_2_1_ID).unwrap().id, AI_2_1_ID);
        assert_eq!(profile_by_id(AI_TURTLE_ID).unwrap().id, AI_TURTLE_ID);
    }

    #[test]
    fn ai_2_1_retains_the_promoted_pressure_policy() {
        let transition = AI_2_1
            .tech_transition
            .expect("AI 2.1 has a tank transition");

        assert_eq!(AI_2_1.workers.extra_oil_workers, 12);
        assert_eq!(AI_2_1.resources.oil_after_steel_workers, 5);
        assert_eq!(
            AI_2_1.extra_factories,
            Some(ExtraFactoryPolicy {
                target_count: 2,
                resource_float: AI_2_1_SECOND_FACTORY_FLOAT_THRESHOLD,
            })
        );
        assert_eq!(
            transition.resource_float,
            AI_2_1_TANK_PRESSURE_FLOAT_THRESHOLD
        );
        assert_eq!(transition.required_tech_path, &TANK_TECH_PATH);
    }

    #[test]
    fn unsupported_profile_ids_are_not_registered() {
        assert!(profile_by_id("unsupported_profile").is_none());
    }
}
