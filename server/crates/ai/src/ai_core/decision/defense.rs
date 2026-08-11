use super::geometry::{clamp_to_map, dist2, normalized_direction, squared, tile_center};
use super::resources::forward_steel_cluster_center;
use super::*;

pub(super) const LOCAL_DEFENSE_RADIUS_TILES: f32 = 12.0;

pub(super) const RESOURCE_LINE_DEFENSE_RADIUS_TILES: f32 = 4.0;

pub(super) const WORKER_DEFENSE_RADIUS_TILES: f32 = 5.0;

pub(super) const BUILDING_DEFENSE_RADIUS_TILES: f32 = 6.0;

pub(super) const EXPANSION_DEFENSIVE_LINE_SPACING_TILES: f32 = 1.5;

pub(super) const EXPANSION_DEFENSIVE_LINE_REISSUE_EPS_TILES: f32 = 0.75;

const DEFENSIVE_FIRING_LANE_TILES: f32 = 14.0;
const DEFENSIVE_FIRING_POSITION_SEARCH_TILES: i32 = 6;
const JEFF_DEFENSIVE_RANGE_OVERLAP_TILES: f32 = 2.0;
const JEFF_BREAKTHROUGH_BUFFER_TILES: f32 = 2.0;
const JEFF_STATIONARY_TANK_RANGE_TILES: f32 = 14.0;
const JEFF_RESERVE_LINE_DEPTH_TILES: f32 = 4.0;

pub(super) const DEFENSIVE_PANIC_GRACE_TICKS: u32 = 90;

pub(super) const DEFENSIVE_PANIC_SUSTAINED_TICKS: u32 = 180;

pub(super) const DEFENSIVE_PANIC_SUSTAINED_BARRACKS: usize = 2;

pub(super) const DEFENSIVE_PANIC_DPS_DOMINANCE: f32 = 0.75;

pub(super) const DEFENSIVE_PANIC_ENEMY_VALUE_NUMERATOR: u32 = 3;

pub(super) const DEFENSIVE_PANIC_ENEMY_VALUE_DENOMINATOR: u32 = 4;

pub(super) const DEFENSIVE_PANIC_OIL_WORKERS: usize = 2;

pub(super) const DEFENSIVE_PANIC_RIFLE_TECH_PATH: [EntityKind; 1] = [EntityKind::Barracks];

pub(super) const DEFENSIVE_PANIC_RIFLE_UNITS: [EntityKind; 1] = [EntityKind::Rifleman];

pub(super) const DEFENSIVE_PANIC_MG_UNITS: [EntityKind; 2] =
    [EntityKind::MachineGunner, EntityKind::Rifleman];

pub(super) const DEFENSIVE_PANIC_AT_UNITS: [EntityKind; 2] =
    [EntityKind::AntiTankGun, EntityKind::Rifleman];

pub(super) const DEFENSIVE_PANIC_SUPPORT_MIX_UNITS: [EntityKind; 3] = [
    EntityKind::AntiTankGun,
    EntityKind::MachineGunner,
    EntityKind::Rifleman,
];

pub(super) const ALL_COMBAT_UNITS: [EntityKind; 5] = [
    EntityKind::Rifleman,
    EntityKind::MachineGunner,
    EntityKind::AntiTankGun,
    EntityKind::ScoutCar,
    EntityKind::Tank,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum DefensivePanicResponse {
    #[default]
    Riflemen,
    MachineGunners,
    AntiTankGuns,
    SupportMix,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct DefensivePanic {
    pub(super) active: bool,
    pub(super) sustained: bool,
    pub(super) response: DefensivePanicResponse,
}

pub(super) fn defensive_panic_barracks_target(panic: DefensivePanic) -> usize {
    if panic.sustained {
        DEFENSIVE_PANIC_SUSTAINED_BARRACKS
    } else {
        1
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DefensivePanicPlan {
    pub(super) required_tech_path: &'static [EntityKind],
    pub(super) production: ProductionPolicy,
    pub(super) oil_workers: usize,
}

pub(super) fn defensive_panic_plan(
    response: DefensivePanicResponse,
    facts: &AiFacts,
) -> DefensivePanicPlan {
    let machine_gunner_tech_ready = facts.complete_building_count(EntityKind::TrainingCentre) > 0;
    let at_tech_ready = facts.complete_building_count(EntityKind::Steelworks) > 0;
    match response {
        DefensivePanicResponse::Riflemen => defensive_panic_rifle_plan(),
        DefensivePanicResponse::MachineGunners if machine_gunner_tech_ready => DefensivePanicPlan {
            required_tech_path: &DEFENSIVE_PANIC_RIFLE_TECH_PATH,
            production: ProductionPolicy {
                queue_depth: 3,
                unit_priorities: &DEFENSIVE_PANIC_MG_UNITS,
                save_for_first_tech_unit: None,
                balance_unit_priorities: false,
            },
            oil_workers: DEFENSIVE_PANIC_OIL_WORKERS,
        },
        DefensivePanicResponse::AntiTankGuns if at_tech_ready => DefensivePanicPlan {
            required_tech_path: &DEFENSIVE_PANIC_RIFLE_TECH_PATH,
            production: ProductionPolicy {
                queue_depth: 3,
                unit_priorities: &DEFENSIVE_PANIC_AT_UNITS,
                save_for_first_tech_unit: None,
                balance_unit_priorities: false,
            },
            oil_workers: DEFENSIVE_PANIC_OIL_WORKERS,
        },
        DefensivePanicResponse::SupportMix if machine_gunner_tech_ready => DefensivePanicPlan {
            required_tech_path: &DEFENSIVE_PANIC_RIFLE_TECH_PATH,
            production: ProductionPolicy {
                queue_depth: 3,
                unit_priorities: &DEFENSIVE_PANIC_SUPPORT_MIX_UNITS,
                save_for_first_tech_unit: None,
                balance_unit_priorities: true,
            },
            oil_workers: DEFENSIVE_PANIC_OIL_WORKERS,
        },
        DefensivePanicResponse::MachineGunners
        | DefensivePanicResponse::AntiTankGuns
        | DefensivePanicResponse::SupportMix => defensive_panic_rifle_plan(),
    }
}

pub(super) fn defensive_panic_rifle_plan() -> DefensivePanicPlan {
    DefensivePanicPlan {
        required_tech_path: &DEFENSIVE_PANIC_RIFLE_TECH_PATH,
        production: ProductionPolicy {
            queue_depth: 3,
            unit_priorities: &DEFENSIVE_PANIC_RIFLE_UNITS,
            save_for_first_tech_unit: None,
            balance_unit_priorities: false,
        },
        oil_workers: 0,
    }
}

pub(super) fn defensive_panic_response(
    observation: &AiObservation,
) -> Option<DefensivePanicResponse> {
    let geometry = LocalDefenseGeometry::from_observation(observation);
    let enemy_value = local_enemy_unit_value(observation, &geometry);
    if enemy_value == 0 {
        return None;
    }
    let own_value = local_owned_unit_value(observation, &geometry);
    if enemy_value.saturating_mul(DEFENSIVE_PANIC_ENEMY_VALUE_DENOMINATOR)
        < own_value.saturating_mul(DEFENSIVE_PANIC_ENEMY_VALUE_NUMERATOR)
    {
        return None;
    }

    let mut local_scores = DefensiveThreatScores::default();
    let mut visible_scores = DefensiveThreatScores::default();

    for enemy in &observation.visible_enemies {
        let score = defensive_threat_dps(enemy);
        if score <= 0.0 {
            continue;
        }
        visible_scores.add(enemy.kind, score);
        if geometry.contains(enemy) {
            local_scores.add(enemy.kind, score);
        }
    }

    Some(
        if local_scores.non_empty() {
            local_scores
        } else {
            visible_scores
        }
        .response(),
    )
}

fn local_enemy_unit_value(observation: &AiObservation, geometry: &LocalDefenseGeometry) -> u32 {
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind.is_unit())
        .filter(|enemy| geometry.contains(enemy))
        .map(|enemy| unit_value(enemy.kind))
        .sum()
}

fn local_owned_unit_value(observation: &AiObservation, geometry: &LocalDefenseGeometry) -> u32 {
    observation
        .owned
        .iter()
        .filter(|entity| entity.kind.is_unit())
        .filter(|entity| geometry.contains(entity))
        .map(|entity| unit_value(entity.kind))
        .sum()
}

fn unit_value(kind: EntityKind) -> u32 {
    let (steel, oil) = rts_rules::economy::cost(kind);
    steel.saturating_add(oil)
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct DefensiveThreatScores {
    armored_dps: f32,
    infantry_dps: f32,
}

impl DefensiveThreatScores {
    fn add(&mut self, kind: EntityKind, dps: f32) {
        if kind == EntityKind::Tank {
            self.armored_dps += dps;
        } else if kind.is_unit() {
            self.infantry_dps += dps;
        }
    }

    fn non_empty(self) -> bool {
        self.armored_dps + self.infantry_dps > f32::EPSILON
    }

    fn response(self) -> DefensivePanicResponse {
        let total = self.armored_dps + self.infantry_dps;
        if total <= f32::EPSILON {
            return DefensivePanicResponse::Riflemen;
        }
        if self.armored_dps / total >= DEFENSIVE_PANIC_DPS_DOMINANCE {
            DefensivePanicResponse::AntiTankGuns
        } else if self.infantry_dps / total >= DEFENSIVE_PANIC_DPS_DOMINANCE {
            DefensivePanicResponse::MachineGunners
        } else {
            DefensivePanicResponse::SupportMix
        }
    }
}

pub(super) fn defensive_threat_dps(enemy: &AiEntitySummary) -> f32 {
    if !enemy.kind.is_unit() {
        return 0.0;
    }
    let profile = rts_rules::combat::attack_profile(enemy.kind);
    if profile.dmg == 0 || profile.cooldown == 0 {
        return 0.0;
    }
    profile.dmg as f32 / profile.cooldown as f32
}

pub(super) fn stage_main_steel_defensive_line(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
    distance_tiles: f32,
) -> Option<Vec<u32>> {
    stage_main_steel_defensive_line_with_spacing(
        actions,
        observation,
        ready_units,
        enemy_base,
        distance_tiles,
        EXPANSION_DEFENSIVE_LINE_SPACING_TILES,
        ready_units.len(),
    )
}

pub(super) fn stage_main_steel_defensive_line_with_spacing(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
    distance_tiles: f32,
    lateral_spacing_tiles: f32,
    formation_slots: usize,
) -> Option<Vec<u32>> {
    let assignments = main_steel_defensive_line_assignments(
        observation,
        ready_units,
        enemy_base,
        distance_tiles,
        lateral_spacing_tiles,
        formation_slots,
    )?;
    let units_by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|entity| (entity.id, entity))
        .collect();
    let close_enough_px =
        EXPANSION_DEFENSIVE_LINE_REISSUE_EPS_TILES * observation.map.tile_size as f32;
    let close_enough2 = squared(close_enough_px);
    let mut staged = Vec::new();

    for assignment in assignments {
        let Some(unit) = units_by_id.get(&assignment.unit_id).copied() else {
            continue;
        };
        if dist2(unit.x, unit.y, assignment.x, assignment.y) <= close_enough2 {
            continue;
        }
        if let Some(units) =
            actions::attack_move_units(actions, [assignment.unit_id], assignment.x, assignment.y)
        {
            staged.extend(units);
        }
    }

    (!staged.is_empty()).then_some(staged)
}

pub(super) fn defensive_machine_gunner_units(
    observation: &AiObservation,
    profile: &AiProfile,
) -> Vec<u32> {
    defensive_machine_gunner_units_matching(observation, profile, true)
}

/// Construction clearance must be allowed to interrupt a defensive Machine Gunner that is
/// firing, holding, entrenched, or otherwise not currently classified as free for combat.
pub(super) fn defensive_machine_gunner_units_for_build_clearance(
    observation: &AiObservation,
    profile: &AiProfile,
) -> Vec<u32> {
    defensive_machine_gunner_units_matching(observation, profile, false)
}

fn defensive_machine_gunner_units_matching(
    observation: &AiObservation,
    profile: &AiProfile,
    require_free_for_combat: bool,
) -> Vec<u32> {
    let Some(policy) = profile.defensive_machine_gunners else {
        return Vec::new();
    };
    let mut units: Vec<u32> = observation
        .owned
        .iter()
        .filter(|entity| {
            entity.kind == EntityKind::MachineGunner
                && entity.hp > 0
                && entity.is_complete
                && (!require_free_for_combat || entity.free_for_combat)
        })
        .map(|entity| entity.id)
        .collect();
    units.sort_unstable();
    if let Some(threshold) = policy.replacement_health_percent {
        units.retain(|id| {
            observation.owned.iter().any(|entity| {
                entity.id == *id && machine_gunner_meets_replacement_health(entity.hp, threshold)
            })
        });
    }
    units.truncate(policy.target_count);
    units
}

pub(super) fn machine_gunner_meets_replacement_health(hp: u32, threshold: u8) -> bool {
    let max_hp = config::unit_stats(EntityKind::MachineGunner)
        .map(|stats| stats.hp)
        .unwrap_or(0);
    hp.saturating_mul(100) >= max_hp.saturating_mul(u32::from(threshold))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_core::observation::{AiEconomy, AiResourceSummary};
    use rts_sim::protocol::{terrain, MapInfo, PlayerStart, StartPayload};

    fn los_test_observation(blocker: EntityKind) -> AiObservation {
        let tile_size = config::TILE_SIZE;
        AiObservation {
            player_id: 1,
            tick: 0,
            map: AiMapSummary {
                width: 32,
                height: 32,
                tile_size,
            },
            economy: AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 0,
                supply_cap: 100,
            },
            own_start_tile: (5, 5),
            players: Vec::new(),
            owned: vec![AiEntitySummary {
                id: 10,
                owner: 1,
                kind: blocker,
                x: 10.5 * tile_size as f32,
                y: 5.5 * tile_size as f32,
                hp: 100,
                state: AiEntityState::Idle,
                is_complete: true,
                production_queue_len: None,
                production_kind: None,
                latched_node: None,
                target_id: None,
                free_for_combat: false,
            }],
            resources: Vec::new(),
            visible_allies: Vec::new(),
            visible_enemies: Vec::new(),
            pending_builds: Vec::new(),
            upgrades: Vec::new(),
        }
    }

    fn open_test_map_analysis() -> AiMapAnalysis {
        let width = 32;
        let height = 32;
        let terrain = vec![terrain::GRASS; (width * height) as usize];
        AiMapAnalysis::analyze(&StartPayload {
            player_id: 1,
            spectator: false,
            prediction_build_id: None,
            prediction_version: 0,
            match_run_id: None,
            capabilities: Default::default(),
            diagnostics: Default::default(),
            replay: None,
            lab: None,
            observer_view: None,
            tick: 0,
            map: MapInfo {
                width,
                height,
                tile_size: config::TILE_SIZE,
                elevation: vec![0; terrain.len()],
                sun: None,
                terrain,
                resources: Vec::new(),
                doodads: Vec::new(),
                concealment_tiles: Vec::new(),
                no_vehicle_tiles: Vec::new(),
                no_building_tiles: Vec::new(),
                no_entrenchment_tiles: Vec::new(),
                damage_reduction_tiles: Vec::new(),
                slow_movement_tiles: Vec::new(),
            },
            players: vec![
                PlayerStart {
                    id: 1,
                    team_id: 1,
                    faction_id: "kriegsia".to_string(),
                    name: "P1".to_string(),
                    color: "#00f".to_string(),
                    is_ai: true,
                    start_tile_x: 5,
                    start_tile_y: 5,
                },
                PlayerStart {
                    id: 2,
                    team_id: 2,
                    faction_id: "kriegsia".to_string(),
                    name: "P2".to_string(),
                    color: "#f00".to_string(),
                    is_ai: true,
                    start_tile_x: 27,
                    start_tile_y: 27,
                },
            ],
        })
    }

    fn test_entity(id: u32, kind: EntityKind, tile_x: f32, tile_y: f32) -> AiEntitySummary {
        AiEntitySummary {
            id,
            owner: 1,
            kind,
            x: tile_x * config::TILE_SIZE as f32,
            y: tile_y * config::TILE_SIZE as f32,
            hp: config::unit_stats(kind)
                .map(|stats| stats.hp)
                .unwrap_or(300),
            state: AiEntityState::Idle,
            is_complete: true,
            production_queue_len: None,
            production_kind: None,
            latched_node: None,
            target_id: None,
            free_for_combat: true,
        }
    }

    #[test]
    fn machine_gunner_below_half_health_requires_replacement() {
        let max_hp = config::unit_stats(EntityKind::MachineGunner)
            .expect("machine gunner stats")
            .hp;
        let half_or_above = max_hp.div_ceil(2);
        assert!(machine_gunner_meets_replacement_health(half_or_above, 50));
        assert!(!machine_gunner_meets_replacement_health(
            half_or_above - 1,
            50
        ));
    }

    #[test]
    fn jeff_defensive_slots_overlap_entrenched_ranges_by_two_tiles() {
        assert_eq!(
            entrenched_coverage_spacing_tiles(EntityKind::MachineGunner, EntityKind::Rifleman),
            11.0
        );
        assert_eq!(
            entrenched_coverage_spacing_tiles(EntityKind::Rifleman, EntityKind::Rifleman),
            10.0
        );
        assert_eq!(
            entrenched_coverage_spacing_tiles(EntityKind::MachineGunner, EntityKind::MachineGunner),
            12.0
        );
        let front = jeff_rifle_lattice_slot(2, 1, 12.0, 11.0, 10.0);
        let rear = jeff_rifle_lattice_slot(2, 3, 12.0, 11.0, 10.0);
        assert_eq!(
            front,
            JeffRifleLatticeSlot {
                lateral_tiles: 17.0,
                depth_tiles: 0.0
            }
        );
        assert!((rear.lateral_tiles - 12.0).abs() < 0.001);
        assert!((rear.depth_tiles - 5.0_f32.mul_add(3.0_f32.sqrt(), 0.0)).abs() < 0.001);
        let diagonal_spacing = ((front.lateral_tiles - rear.lateral_tiles).powi(2)
            + (front.depth_tiles - rear.depth_tiles).powi(2))
        .sqrt();
        assert!((diagonal_spacing - 10.0).abs() < 0.001);

        let no_anchor_left = jeff_rifle_lattice_slot(0, 0, 12.0, 11.0, 10.0);
        let no_anchor_right = jeff_rifle_lattice_slot(0, 1, 12.0, 11.0, 10.0);
        let no_anchor_rear = jeff_rifle_lattice_slot(0, 2, 12.0, 11.0, 10.0);
        assert_eq!(no_anchor_left.lateral_tiles, -5.0);
        assert_eq!(no_anchor_right.lateral_tiles, 5.0);
        assert_eq!(no_anchor_rear.lateral_tiles, 0.0);
        assert!((no_anchor_rear.depth_tiles - 5.0_f32.mul_add(3.0_f32.sqrt(), 0.0)).abs() < 0.001);
    }

    #[test]
    fn jeff_breakthrough_preserves_covering_tank_then_pulls_only_mobile_value_needed() {
        let analysis = open_test_map_analysis();
        let mut observation = los_test_observation(EntityKind::Depot);
        observation.owned = vec![
            test_entity(1, EntityKind::ResourceDepot, 5.5, 5.5),
            test_entity(2, EntityKind::Tank, 5.5, 5.5),
            test_entity(3, EntityKind::ScoutCar, 20.5, 20.5),
            test_entity(4, EntityKind::Rifleman, 30.5, 30.5),
        ];
        observation.visible_enemies = vec![test_entity(30, EntityKind::Rifleman, 8.5, 8.5)];
        observation.upgrades.push(UpgradeKind::Entrenchment);

        let covered =
            jeff_breakthrough_response(&observation, Some(&analysis), &BTreeSet::new(), Some(2));
        assert_eq!(covered.targets, vec![30]);
        assert!(covered.responders.is_empty());

        observation
            .visible_enemies
            .push(test_entity(31, EntityKind::Tank, 9.5, 8.5));
        let partially_covered =
            jeff_breakthrough_response(&observation, Some(&analysis), &BTreeSet::new(), Some(2));
        assert_eq!(partially_covered.targets, vec![30, 31]);
        assert_eq!(partially_covered.responders, vec![3]);

        observation.visible_enemies.truncate(1);
        observation.owned[1].x = 30.5 * config::TILE_SIZE as f32;
        observation.owned[1].y = 30.5 * config::TILE_SIZE as f32;
        let uncovered =
            jeff_breakthrough_response(&observation, Some(&analysis), &BTreeSet::new(), Some(2));
        assert_eq!(uncovered.targets, vec![30]);
        assert_eq!(uncovered.responders, vec![3]);
    }

    #[test]
    fn defensive_firing_lane_rejects_opaque_building_but_not_pump_jack() {
        let ts = config::TILE_SIZE as f32;
        let origin = (5.5 * ts, 5.5 * ts);
        assert!(!defensive_firing_lane_is_clear(
            &los_test_observation(EntityKind::Depot),
            None,
            origin,
            (1.0, 0.0),
            14.0,
        ));
        assert!(defensive_firing_lane_is_clear(
            &los_test_observation(EntityKind::PumpJack),
            None,
            origin,
            (1.0, 0.0),
            14.0,
        ));
    }

    #[test]
    fn defensive_firing_sector_rejects_a_building_masking_its_flank() {
        let mut observation = los_test_observation(EntityKind::Depot);
        let ts = observation.map.tile_size as f32;
        let origin = (5.5 * ts, 5.5 * ts);
        observation.owned[0].y = 7.0 * ts;
        assert!(defensive_firing_lane_is_clear(
            &observation,
            None,
            origin,
            (1.0, 0.0),
            14.0,
        ));
        assert!(!defensive_firing_sector_is_clear(
            &observation,
            None,
            origin,
            (1.0, 0.0),
            14.0,
        ));
    }

    #[test]
    fn defensive_assignment_shifts_out_from_behind_building() {
        let observation = los_test_observation(EntityKind::Depot);
        let ts = observation.map.tile_size as f32;
        let original = DefensiveLineAssignment {
            unit_id: 20,
            x: 5.5 * ts,
            y: 5.5 * ts,
        };
        let adjusted = clear_firing_assignment(
            &observation,
            None,
            original,
            EnemyBaseFact {
                player_id: 2,
                start_tile: (25, 5),
                x: 25.5 * ts,
                y: 5.5 * ts,
            },
            14.0,
        )
        .expect("nearby clear firing position");
        assert_ne!((adjusted.x, adjusted.y), (original.x, original.y));
        let direction = normalized_direction((adjusted.x, adjusted.y), (25.5 * ts, 5.5 * ts))
            .expect("adjusted assignment direction");
        assert!(defensive_firing_sector_is_clear(
            &observation,
            None,
            (adjusted.x, adjusted.y),
            direction,
            14.0,
        ));
    }

    #[test]
    fn machine_gunner_screen_moves_in_front_of_forward_factory() {
        let observation = los_test_observation(EntityKind::Factory);
        let ts = observation.map.tile_size as f32;
        let assignment = DefensiveLineAssignment {
            unit_id: 20,
            x: 5.5 * ts,
            y: 5.5 * ts,
        };
        let adjusted = clear_machine_gunner_screen_assignment(
            &observation,
            None,
            assignment,
            EnemyBaseFact {
                player_id: 2,
                start_tile: (25, 5),
                x: 25.5 * ts,
                y: 5.5 * ts,
            },
        )
        .expect("clear Machine Gunner position in front of Factory");

        assert!(adjusted.x > observation.owned[0].x);
        assert_eq!(adjusted.y, assignment.y);
    }

    #[test]
    fn infantry_at_home_defends_a_forward_building_under_attack() {
        let mut observation = los_test_observation(EntityKind::Factory);
        let ts = observation.map.tile_size as f32;
        observation.owned[0].x = 24.5 * ts;
        observation.owned[0].y = 5.5 * ts;
        for (id, kind) in [(20, EntityKind::Rifleman), (21, EntityKind::MachineGunner)] {
            observation.owned.push(AiEntitySummary {
                id,
                owner: 1,
                kind,
                x: 5.5 * ts,
                y: 5.5 * ts,
                hp: config::unit_stats(kind).expect("infantry stats").hp,
                state: AiEntityState::Idle,
                is_complete: true,
                production_queue_len: None,
                production_kind: None,
                latched_node: None,
                target_id: None,
                free_for_combat: true,
            });
        }
        observation.visible_enemies.push(AiEntitySummary {
            id: 30,
            owner: 2,
            kind: EntityKind::Rifleman,
            x: 27.5 * ts,
            y: 5.5 * ts,
            hp: 100,
            state: AiEntityState::Attack,
            is_complete: true,
            production_queue_len: None,
            production_kind: None,
            latched_node: None,
            target_id: Some(10),
            free_for_combat: true,
        });

        assert_eq!(local_defense_target(&observation), Some(30));
        assert_eq!(local_defense_units(&observation, &[20, 21]), vec![20, 21]);
    }

    #[test]
    fn first_machine_gunner_reserves_a_flank_slot_in_two_unit_formation() {
        let mut observation = los_test_observation(EntityKind::Factory);
        let tile_size = observation.map.tile_size as f32;
        observation.resources.push(AiResourceSummary {
            id: 100,
            kind: EntityKind::Steel,
            x: 7.5 * tile_size,
            y: 5.5 * tile_size,
            remaining: 625,
        });
        let enemy_base = EnemyBaseFact {
            player_id: 2,
            start_tile: (25, 5),
            x: 25.5 * observation.map.tile_size as f32,
            y: 5.5 * observation.map.tile_size as f32,
        };
        let centered =
            main_steel_defensive_line_assignments(&observation, &[20], enemy_base, 6.0, 4.5, 1)
                .expect("centered assignment")[0];
        let reserved =
            main_steel_defensive_line_assignments(&observation, &[20], enemy_base, 6.0, 4.5, 2)
                .expect("reserved flank assignment")[0];
        let offset_tiles = dist2(centered.x, centered.y, reserved.x, reserved.y).sqrt()
            / observation.map.tile_size as f32;

        assert!((offset_tiles - 2.25).abs() < 0.001);
    }
}

pub(super) fn stage_defensive_machine_gunner_perimeter(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    profile: &AiProfile,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
) -> Option<Vec<u32>> {
    let policy = profile.defensive_machine_gunners?;
    stage_machine_gunner_defensive_line(
        actions,
        observation,
        map_analysis,
        ready_units,
        enemy_base,
        policy.perimeter_distance_tiles,
        (policy.lateral_spacing_tiles, policy.target_count),
    )
}

pub(super) fn stage_home_machine_gunner_screen(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
    distance_tiles: f32,
    lateral_spacing_tiles: f32,
) -> Option<Vec<u32>> {
    let assignments = main_steel_defensive_line_assignments(
        observation,
        ready_units,
        enemy_base,
        distance_tiles,
        lateral_spacing_tiles,
        ready_units.len(),
    )?
    .into_iter()
    .filter_map(|assignment| {
        clear_machine_gunner_screen_assignment(observation, map_analysis, assignment, enemy_base)
    })
    .collect::<Vec<_>>();
    let by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|entity| (entity.id, entity))
        .collect();
    let tolerance = EXPANSION_DEFENSIVE_LINE_REISSUE_EPS_TILES * observation.map.tile_size as f32;
    let tolerance2 = squared(tolerance);
    let mut staged = Vec::new();
    for assignment in assignments {
        let Some(unit) = by_id.get(&assignment.unit_id).copied() else {
            continue;
        };
        let command = if dist2(unit.x, unit.y, assignment.x, assignment.y) <= tolerance2 {
            actions::hold_position_units(actions, [assignment.unit_id])
        } else {
            actions::move_units(actions, [assignment.unit_id], assignment.x, assignment.y)
        };
        if let Some(units) = command {
            staged.extend(units);
        }
    }
    staged.sort_unstable();
    staged.dedup();
    (!staged.is_empty()).then_some(staged)
}

pub(super) fn stage_home_rifleman_screen(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    ready_units: &[u32],
    armor_id: u32,
    enemy_base: EnemyBaseFact,
    forward_tiles: f32,
    lateral_spacing_tiles: f32,
) -> Option<Vec<u32>> {
    let armor = observation
        .owned
        .iter()
        .find(|entity| entity.id == armor_id)?;
    let direction = normalized_direction((armor.x, armor.y), (enemy_base.x, enemy_base.y))?;
    let perpendicular = (-direction.1, direction.0);
    let tile_size = observation.map.tile_size as f32;
    let center = clamp_to_map(
        (
            armor.x + direction.0 * forward_tiles * tile_size,
            armor.y + direction.1 * forward_tiles * tile_size,
        ),
        observation.map,
    );
    let by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|entity| (entity.id, entity))
        .collect();
    let tolerance = EXPANSION_DEFENSIVE_LINE_REISSUE_EPS_TILES * tile_size;
    let tolerance2 = squared(tolerance);
    // A single dense rank creates ideal overpenetration lanes for Tanks. Keep
    // the cheap Riflemen as first contact, but stagger them across two ranks.
    let rank_count = 2usize;
    let mut staged = Vec::new();
    for (index, unit_id) in ready_units.iter().copied().enumerate() {
        let Some(unit) = by_id.get(&unit_id).copied() else {
            continue;
        };
        let rank = index % rank_count;
        let slot = index / rank_count;
        let rank_len = ready_units.len().saturating_sub(rank).div_ceil(rank_count);
        let middle = rank_len.saturating_sub(1) as f32 / 2.0;
        let lateral =
            (slot as f32 - middle + rank as f32 * 0.5) * lateral_spacing_tiles * tile_size;
        let depth = rank as f32 * 1.5 * tile_size;
        let target = clamp_to_map(
            (
                center.0 + perpendicular.0 * lateral + direction.0 * depth,
                center.1 + perpendicular.1 * lateral + direction.1 * depth,
            ),
            observation.map,
        );
        let command = if dist2(unit.x, unit.y, target.0, target.1) <= tolerance2 {
            actions::hold_position_units(actions, [unit_id])
        } else {
            actions::move_units(actions, [unit_id], target.0, target.1)
        };
        if let Some(units) = command {
            staged.extend(units);
        }
    }
    staged.sort_unstable();
    staged.dedup();
    (!staged.is_empty()).then_some(staged)
}

pub(super) fn stage_jeff_attack_path_defense(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    machine_gunners: &[u32],
    riflemen: &[u32],
) -> Option<Vec<u32>> {
    let analysis = map_analysis?;
    let base_count = observation
        .owned
        .iter()
        .filter(|entity| {
            entity.kind == EntityKind::ResourceDepot && entity.is_complete && entity.hp > 0
        })
        .count()
        .clamp(1, 2);
    let report = analysis.likely_attack_paths(observation.player_id, base_count, 3);
    let mut routes = report.paths.iter().collect::<Vec<_>>();
    routes.sort_by_key(|path| (path.alternative_rank, path.defended_base_id, path.id));
    if routes.is_empty() {
        return None;
    }

    let mut machine_gunners = machine_gunners.to_vec();
    machine_gunners.sort_unstable();
    machine_gunners.dedup();
    machine_gunners.truncate(2);
    let machine_gunner_count = machine_gunners.len();
    let mut riflemen = riflemen.to_vec();
    riflemen.sort_unstable();
    riflemen.dedup();

    let entities_by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|entity| (entity.id, entity))
        .collect();
    let tile_size = observation.map.tile_size.max(1) as f32;
    let tolerance2 = squared(EXPANSION_DEFENSIVE_LINE_REISSUE_EPS_TILES * tile_size);
    let mut staged = Vec::new();
    let rifle_from_mg_spacing =
        entrenched_coverage_spacing_tiles(EntityKind::MachineGunner, EntityKind::Rifleman);
    let rifle_spacing =
        entrenched_coverage_spacing_tiles(EntityKind::Rifleman, EntityKind::Rifleman);
    let machine_gunner_spacing =
        entrenched_coverage_spacing_tiles(EntityKind::MachineGunner, EntityKind::MachineGunner);
    let primary_routes = routes
        .iter()
        .copied()
        .filter(|path| path.alternative_rank == 1)
        .collect::<Vec<_>>();
    let center_routes = if primary_routes.is_empty() {
        &routes
    } else {
        &primary_routes
    };

    for (index, unit_id) in machine_gunners.into_iter().enumerate() {
        let route_index = index % center_routes.len();
        let route = center_routes[route_index];
        let units_on_route = (route_index..machine_gunner_count)
            .step_by(center_routes.len())
            .count();
        let slot_on_route = index / center_routes.len();
        let center_index = units_on_route.saturating_sub(1) as f32 * 0.5;
        let lateral_offset_tiles = (slot_on_route as f32 - center_index) * machine_gunner_spacing;
        let Some(lane) = attack_lane_for_path(route, tile_size) else {
            continue;
        };
        let Some(target) = clear_attack_path_assignment(
            observation,
            analysis,
            lane.center,
            lane.toward_attacker,
            lateral_offset_tiles,
        ) else {
            continue;
        };
        let Some(unit) = entities_by_id.get(&unit_id).copied() else {
            continue;
        };
        let command = if dist2(unit.x, unit.y, target.0, target.1) <= tolerance2 {
            actions::hold_position_units(actions, [unit_id])
        } else {
            actions::attack_move_units(actions, [unit_id], target.0, target.1)
        };
        if let Some(units) = command {
            staged.extend(units);
        }
    }

    // Riflemen cover the edges of every plausible lane, not just the primary route. Extra units
    // alternate between a front 010101 rank and a rear 101010 rank. The half-slot lateral stagger
    // and equilateral-triangle depth preserve the requested two-tile range overlap diagonally as
    // well as along each rank.
    let flank_routes = &routes;
    for (index, unit_id) in riflemen.into_iter().enumerate() {
        let route_index = (index / 2) % flank_routes.len();
        let pair_on_route = index / (flank_routes.len() * 2);
        let slot_on_route = pair_on_route * 2 + index % 2;
        let machine_gunners_on_route = center_routes
            .iter()
            .position(|center_path| center_path.id == flank_routes[route_index].id)
            .map(|center_route_index| {
                (center_route_index..machine_gunner_count)
                    .step_by(center_routes.len())
                    .count()
            })
            .unwrap_or(0);
        let slot = jeff_rifle_lattice_slot(
            machine_gunners_on_route,
            slot_on_route,
            machine_gunner_spacing,
            rifle_from_mg_spacing,
            rifle_spacing,
        );
        let Some(lane) = attack_lane_for_path(flank_routes[route_index], tile_size) else {
            continue;
        };
        let rank_center = (
            lane.center.0 - lane.toward_attacker.0 * slot.depth_tiles * tile_size,
            lane.center.1 - lane.toward_attacker.1 * slot.depth_tiles * tile_size,
        );
        let Some(target) = clear_attack_path_assignment(
            observation,
            analysis,
            rank_center,
            lane.toward_attacker,
            slot.lateral_tiles,
        ) else {
            continue;
        };
        let Some(unit) = entities_by_id.get(&unit_id).copied() else {
            continue;
        };
        let command = if dist2(unit.x, unit.y, target.0, target.1) <= tolerance2 {
            actions::hold_position_units(actions, [unit_id])
        } else {
            actions::attack_move_units(actions, [unit_id], target.0, target.1)
        };
        if let Some(units) = command {
            staged.extend(units);
        }
    }

    staged.sort_unstable();
    staged.dedup();
    (!staged.is_empty()).then_some(staged)
}

fn entrenched_coverage_spacing_tiles(left: EntityKind, right: EntityKind) -> f32 {
    let range = |kind| {
        config::unit_stats(kind)
            .map(|stats| stats.range_tiles as f32)
            .unwrap_or(0.0)
            + config::ENTRENCHMENT_RANGE_BONUS_TILES as f32
    };
    (range(left) + range(right) - JEFF_DEFENSIVE_RANGE_OVERLAP_TILES).max(1.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct JeffRifleLatticeSlot {
    lateral_tiles: f32,
    depth_tiles: f32,
}

fn jeff_rifle_lattice_slot(
    machine_gunners_on_route: usize,
    slot_index: usize,
    machine_gunner_spacing: f32,
    rifle_from_mg_spacing: f32,
    rifle_spacing: f32,
) -> JeffRifleLatticeSlot {
    let first_rifle_offset = if machine_gunners_on_route == 0 {
        rifle_spacing * 0.5
    } else {
        let machine_gunner_half_span =
            machine_gunners_on_route.saturating_sub(1) as f32 * machine_gunner_spacing * 0.5;
        machine_gunner_half_span + rifle_from_mg_spacing
    };
    let half_spacing = rifle_spacing * 0.5;
    let rear_depth = (rifle_spacing.powi(2) - half_spacing.powi(2)).sqrt();

    let (lateral_tiles, rear_rank) = if machine_gunners_on_route == 0 {
        match slot_index {
            0 => (-first_rifle_offset, false),
            1 => (first_rifle_offset, false),
            2 => (0.0, true),
            _ => {
                let tail = slot_index - 3;
                let shell = (tail / 4) as f32;
                match tail % 4 {
                    0 => (-(first_rifle_offset + (shell + 1.0) * rifle_spacing), false),
                    1 => (first_rifle_offset + (shell + 1.0) * rifle_spacing, false),
                    2 => (-((shell + 1.0) * rifle_spacing), true),
                    _ => ((shell + 1.0) * rifle_spacing, true),
                }
            }
        }
    } else {
        let shell = (slot_index / 4) as f32;
        let side = if slot_index.is_multiple_of(2) {
            -1.0
        } else {
            1.0
        };
        if slot_index % 4 < 2 {
            (side * (first_rifle_offset + shell * rifle_spacing), false)
        } else {
            (
                side * (first_rifle_offset - half_spacing + shell * rifle_spacing),
                true,
            )
        }
    };

    JeffRifleLatticeSlot {
        lateral_tiles,
        depth_tiles: if rear_rank { rear_depth } else { 0.0 },
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct AttackPathLane {
    center: (f32, f32),
    toward_attacker: (f32, f32),
}

fn attack_lane_for_path(
    path: &crate::ai_core::map_analysis::AiAttackPath,
    tile_size: f32,
) -> Option<AttackPathLane> {
    let intercept = (path.intercept_tile_x, path.intercept_tile_y);
    let intercept_index = path
        .tiles
        .iter()
        .position(|tile| *tile == [intercept.0, intercept.1])
        .unwrap_or_else(|| {
            path.tiles
                .iter()
                .enumerate()
                .min_by_key(|(_, tile)| {
                    let dx = i64::from(tile[0]) - i64::from(intercept.0);
                    let dy = i64::from(tile[1]) - i64::from(intercept.1);
                    dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy))
                })
                .map(|(index, _)| index)
                .unwrap_or(0)
        });
    let before = path.tiles.get(intercept_index.saturating_sub(3))?;
    let after = path
        .tiles
        .get((intercept_index + 3).min(path.tiles.len().saturating_sub(1)))?;
    let toward_attacker = normalized_direction(
        (after[0] as f32, after[1] as f32),
        (before[0] as f32, before[1] as f32),
    )?;
    Some(AttackPathLane {
        center: (
            (intercept.0 as f32 + 0.5) * tile_size,
            (intercept.1 as f32 + 0.5) * tile_size,
        ),
        toward_attacker,
    })
}

fn clear_attack_path_assignment(
    observation: &AiObservation,
    analysis: &AiMapAnalysis,
    center: (f32, f32),
    toward_attacker: (f32, f32),
    lateral_offset_tiles: f32,
) -> Option<(f32, f32)> {
    let tile_size = observation.map.tile_size.max(1) as f32;
    let perpendicular = (-toward_attacker.1, toward_attacker.0);
    let desired = (
        center.0 + perpendicular.0 * lateral_offset_tiles * tile_size,
        center.1 + perpendicular.1 * lateral_offset_tiles * tile_size,
    );
    let mut offsets = vec![0.0_f32];
    for radius in 1..=DEFENSIVE_FIRING_POSITION_SEARCH_TILES {
        offsets.extend([radius as f32, -(radius as f32)]);
    }
    offsets
        .into_iter()
        .map(|offset| {
            clamp_to_map(
                (
                    desired.0 + perpendicular.0 * offset * tile_size,
                    desired.1 + perpendicular.1 * offset * tile_size,
                ),
                observation.map,
            )
        })
        .find(|candidate| {
            defensive_position_is_open(observation, Some(analysis), candidate.0, candidate.1)
                && defensive_firing_sector_is_clear(
                    observation,
                    Some(analysis),
                    *candidate,
                    toward_attacker,
                    DEFENSIVE_FIRING_LANE_TILES,
                )
        })
}

fn stage_machine_gunner_defensive_line(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
    distance_tiles: f32,
    formation: (f32, usize),
) -> Option<Vec<u32>> {
    let (lateral_spacing_tiles, formation_slots) = formation;
    let assignments = main_steel_defensive_line_assignments(
        observation,
        ready_units,
        enemy_base,
        distance_tiles,
        lateral_spacing_tiles,
        formation_slots,
    )?;
    let units_by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|entity| (entity.id, entity))
        .collect();
    let close_enough =
        EXPANSION_DEFENSIVE_LINE_REISSUE_EPS_TILES * observation.map.tile_size as f32;
    let close_enough2 = squared(close_enough);
    let mut staged = Vec::new();
    for assignment in assignments.into_iter().filter_map(|assignment| {
        clear_machine_gunner_screen_assignment(observation, map_analysis, assignment, enemy_base)
    }) {
        let Some(unit) = units_by_id.get(&assignment.unit_id).copied() else {
            continue;
        };
        if dist2(unit.x, unit.y, assignment.x, assignment.y) <= close_enough2 {
            continue;
        }
        if let Some(units) =
            actions::attack_move_units(actions, [assignment.unit_id], assignment.x, assignment.y)
        {
            staged.extend(units);
        }
    }
    (!staged.is_empty()).then_some(staged)
}

fn clear_machine_gunner_screen_assignment(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    assignment: DefensiveLineAssignment,
    enemy_base: EnemyBaseFact,
) -> Option<DefensiveLineAssignment> {
    let direction =
        normalized_direction((assignment.x, assignment.y), (enemy_base.x, enemy_base.y))?;
    let tile_size = observation.map.tile_size as f32;
    let perpendicular = (-direction.1, direction.0);
    let minimum_forward_tiles = observation
        .owned
        .iter()
        .filter(|entity| entity.kind == EntityKind::Factory && entity.is_complete && entity.hp > 0)
        .filter_map(|factory| {
            let stats = config::building_stats(factory.kind)?;
            let delta = (factory.x - assignment.x, factory.y - assignment.y);
            let forward_tiles = (delta.0 * direction.0 + delta.1 * direction.1) / tile_size;
            if forward_tiles <= 0.0 {
                return None;
            }
            let lateral_tiles =
                ((delta.0 * perpendicular.0 + delta.1 * perpendicular.1) / tile_size).abs();
            let half_lateral = (perpendicular.0.abs() * stats.foot_w as f32
                + perpendicular.1.abs() * stats.foot_h as f32)
                * 0.5;
            if lateral_tiles > half_lateral + 1.0 {
                return None;
            }
            let half_forward = (direction.0.abs() * stats.foot_w as f32
                + direction.1.abs() * stats.foot_h as f32)
                * 0.5;
            Some((forward_tiles + half_forward + 1.0).ceil() as usize)
        })
        .max()
        .unwrap_or(0);
    let search_end = minimum_forward_tiles.max(DEFENSIVE_FIRING_POSITION_SEARCH_TILES as usize);
    for forward_tiles in minimum_forward_tiles..=search_end {
        let (x, y) = clamp_to_map(
            (
                assignment.x + direction.0 * forward_tiles as f32 * tile_size,
                assignment.y + direction.1 * forward_tiles as f32 * tile_size,
            ),
            observation.map,
        );
        if defensive_position_is_open(observation, map_analysis, x, y)
            && defensive_firing_sector_is_clear(
                observation,
                map_analysis,
                (x, y),
                direction,
                DEFENSIVE_FIRING_LANE_TILES,
            )
        {
            return Some(DefensiveLineAssignment {
                unit_id: assignment.unit_id,
                x,
                y,
            });
        }
    }
    clear_firing_assignment(
        observation,
        map_analysis,
        assignment,
        enemy_base,
        DEFENSIVE_FIRING_LANE_TILES,
    )
}

pub(super) fn stage_home_anti_tank_line(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    profile: &AiProfile,
    enemy_base: EnemyBaseFact,
    map_analysis: Option<&AiMapAnalysis>,
) -> Option<Vec<u32>> {
    let policy = profile.home_anti_tank?;
    let units = actions::select_ready_combat_units(&observation.owned, &[EntityKind::AntiTankGun]);
    let assignments = main_steel_defensive_line_assignments(
        observation,
        &units,
        enemy_base,
        policy.anti_tank_position_tiles,
        policy.lateral_spacing_tiles,
        units.len(),
    )?;
    let by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|entity| (entity.id, entity))
        .collect();
    let close_enough = observation.map.tile_size as f32;
    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    let facing_target = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::Tank)
        .min_by(|a, b| {
            dist2(a.x, a.y, own_base.0, own_base.1)
                .total_cmp(&dist2(b.x, b.y, own_base.0, own_base.1))
        })
        .map(|enemy| (enemy.x, enemy.y))
        .unwrap_or((enemy_base.x, enemy_base.y));
    let mut staged = Vec::new();
    for assignment in assignments {
        let Some(assignment) = clear_firing_assignment(
            observation,
            map_analysis,
            assignment,
            enemy_base,
            DEFENSIVE_FIRING_LANE_TILES,
        ) else {
            continue;
        };
        let Some(unit) = by_id.get(&assignment.unit_id).copied() else {
            continue;
        };
        let needs_move =
            dist2(unit.x, unit.y, assignment.x, assignment.y) > close_enough * close_enough;
        if needs_move {
            if let Some(units) =
                actions::move_units(actions, [assignment.unit_id], assignment.x, assignment.y)
            {
                staged.extend(units);
            }
        }
        if let Some(units) = actions::setup_anti_tank_guns(
            actions,
            [assignment.unit_id],
            facing_target.0,
            facing_target.1,
            needs_move,
        ) {
            staged.extend(units);
        }
    }
    staged.sort_unstable();
    staged.dedup();
    (!staged.is_empty()).then_some(staged)
}

pub(super) fn home_defensive_tank_is_positioned(
    observation: &AiObservation,
    tank_id: u32,
    enemy_base: EnemyBaseFact,
    distance_tiles: f32,
    map_analysis: Option<&AiMapAnalysis>,
) -> bool {
    let Some(assignment) = main_steel_defensive_line_assignments(
        observation,
        &[tank_id],
        enemy_base,
        distance_tiles,
        EXPANSION_DEFENSIVE_LINE_SPACING_TILES,
        1,
    )
    .and_then(|assignments| assignments.into_iter().next()) else {
        return false;
    };
    let Some(assignment) = clear_firing_assignment(
        observation,
        map_analysis,
        assignment,
        enemy_base,
        DEFENSIVE_FIRING_LANE_TILES,
    ) else {
        return false;
    };
    let Some(tank) = observation.owned.iter().find(|entity| entity.id == tank_id) else {
        return false;
    };
    let tolerance = observation.map.tile_size as f32;
    dist2(tank.x, tank.y, assignment.x, assignment.y) <= tolerance * tolerance
}

pub(super) fn stage_home_defensive_tank(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    tank_id: u32,
    enemy_base: EnemyBaseFact,
    distance_tiles: f32,
    map_analysis: Option<&AiMapAnalysis>,
) -> Option<Vec<u32>> {
    let assignment = main_steel_defensive_line_assignments(
        observation,
        &[tank_id],
        enemy_base,
        distance_tiles,
        EXPANSION_DEFENSIVE_LINE_SPACING_TILES,
        1,
    )?
    .into_iter()
    .next()?;
    let assignment = clear_firing_assignment(
        observation,
        map_analysis,
        assignment,
        enemy_base,
        DEFENSIVE_FIRING_LANE_TILES,
    )?;
    if home_defensive_tank_is_positioned(
        observation,
        tank_id,
        enemy_base,
        distance_tiles,
        map_analysis,
    ) {
        actions::hold_position_units(actions, [tank_id])
    } else {
        actions::move_units(actions, [tank_id], assignment.x, assignment.y)
    }
}

fn clear_firing_assignment(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    assignment: DefensiveLineAssignment,
    enemy_base: EnemyBaseFact,
    lane_tiles: f32,
) -> Option<DefensiveLineAssignment> {
    let Some(direction) =
        normalized_direction((assignment.x, assignment.y), (enemy_base.x, enemy_base.y))
    else {
        return Some(assignment);
    };
    let perpendicular = (-direction.1, direction.0);
    let tile_size = observation.map.tile_size as f32;
    let mut offsets = vec![(0.0, 0.0)];
    for radius in 1..=DEFENSIVE_FIRING_POSITION_SEARCH_TILES {
        let r = radius as f32;
        offsets.extend([
            (perpendicular.0 * r, perpendicular.1 * r),
            (-perpendicular.0 * r, -perpendicular.1 * r),
            (direction.0 * r, direction.1 * r),
            (
                (perpendicular.0 + direction.0) * r,
                (perpendicular.1 + direction.1) * r,
            ),
            (
                (-perpendicular.0 + direction.0) * r,
                (-perpendicular.1 + direction.1) * r,
            ),
            (-direction.0 * r, -direction.1 * r),
        ]);
    }
    offsets
        .into_iter()
        .map(|offset| {
            let (x, y) = clamp_to_map(
                (
                    assignment.x + offset.0 * tile_size,
                    assignment.y + offset.1 * tile_size,
                ),
                observation.map,
            );
            DefensiveLineAssignment {
                unit_id: assignment.unit_id,
                x,
                y,
            }
        })
        .find(|candidate| {
            let candidate_origin = (candidate.x, candidate.y);
            let Some(candidate_direction) =
                normalized_direction(candidate_origin, (enemy_base.x, enemy_base.y))
            else {
                return false;
            };
            defensive_position_is_open(observation, map_analysis, candidate.x, candidate.y)
                && defensive_firing_sector_is_clear(
                    observation,
                    map_analysis,
                    candidate_origin,
                    candidate_direction,
                    lane_tiles,
                )
        })
}

fn defensive_firing_sector_is_clear(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    origin: (f32, f32),
    direction: (f32, f32),
    lane_tiles: f32,
) -> bool {
    let perpendicular = (-direction.1, direction.0);
    // A clear centre ray alone can thread a building corner while most of an
    // emplaced weapon's useful approach sector remains masked. Require clear
    // centre, left, and right rays spanning four tiles at maximum range.
    let half_width_tiles = 2.0;
    [-half_width_tiles, 0.0, half_width_tiles]
        .into_iter()
        .all(|offset| {
            let endpoint_direction = (
                direction.0 * lane_tiles + perpendicular.0 * offset,
                direction.1 * lane_tiles + perpendicular.1 * offset,
            );
            let Some(ray_direction) = normalized_direction((0.0, 0.0), endpoint_direction) else {
                return false;
            };
            defensive_firing_lane_is_clear(
                observation,
                map_analysis,
                origin,
                ray_direction,
                lane_tiles,
            )
        })
}

fn defensive_position_is_open(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    x: f32,
    y: f32,
) -> bool {
    let tile = world_tile(observation.map, x, y);
    map_analysis
        .map(|analysis| analysis.tile_is_passable(tile.0, tile.1))
        .unwrap_or(true)
        && !dynamic_los_blocking_tiles(observation).contains(&tile)
}

fn defensive_firing_lane_is_clear(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    origin: (f32, f32),
    direction: (f32, f32),
    lane_tiles: f32,
) -> bool {
    let tile_size = observation.map.tile_size as f32;
    let endpoint = clamp_to_map(
        (
            origin.0 + direction.0 * lane_tiles * tile_size,
            origin.1 + direction.1 * lane_tiles * tile_size,
        ),
        observation.map,
    );
    let blockers = dynamic_los_blocking_tiles(observation);
    let samples = (lane_tiles * 4.0).ceil().max(1.0) as usize;
    (1..=samples).all(|step| {
        let t = step as f32 / samples as f32;
        let tile = world_tile(
            observation.map,
            origin.0 + (endpoint.0 - origin.0) * t,
            origin.1 + (endpoint.1 - origin.1) * t,
        );
        !blockers.contains(&tile)
            && !map_analysis
                .map(|analysis| analysis.tile_blocks_line_of_sight(tile.0, tile.1))
                .unwrap_or(false)
    })
}

fn dynamic_los_blocking_tiles(observation: &AiObservation) -> BTreeSet<(u32, u32)> {
    observation
        .owned
        .iter()
        .chain(observation.visible_allies.iter())
        .chain(observation.visible_enemies.iter())
        .filter(|entity| entity.hp > 0 && rts_rules::blocks_line_of_sight(entity.kind))
        .flat_map(|entity| building_footprint_tiles(observation.map, entity))
        .collect()
}

fn building_footprint_tiles(map: AiMapSummary, entity: &AiEntitySummary) -> Vec<(u32, u32)> {
    let Some(stats) = config::building_stats(entity.kind) else {
        return Vec::new();
    };
    let center = world_tile(map, entity.x, entity.y);
    let origin_x = center.0 as i32 - stats.foot_w as i32 / 2;
    let origin_y = center.1 as i32 - stats.foot_h as i32 / 2;
    let mut tiles = Vec::new();
    for dy in 0..stats.foot_h as i32 {
        for dx in 0..stats.foot_w as i32 {
            let x = origin_x + dx;
            let y = origin_y + dy;
            if x >= 0 && y >= 0 && x < map.width as i32 && y < map.height as i32 {
                tiles.push((x as u32, y as u32));
            }
        }
    }
    tiles
}

fn world_tile(map: AiMapSummary, x: f32, y: f32) -> (u32, u32) {
    let tile_size = map.tile_size.max(1) as f32;
    let tile_x = (x / tile_size).floor().max(0.0) as u32;
    let tile_y = (y / tile_size).floor().max(0.0) as u32;
    (
        tile_x.min(map.width.saturating_sub(1)),
        tile_y.min(map.height.saturating_sub(1)),
    )
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DefensiveLineAssignment {
    unit_id: u32,
    x: f32,
    y: f32,
}

pub(super) fn main_steel_defensive_line_assignments(
    observation: &AiObservation,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
    distance_tiles: f32,
    lateral_spacing_tiles: f32,
    formation_slots: usize,
) -> Option<Vec<DefensiveLineAssignment>> {
    if ready_units.is_empty() {
        return None;
    }
    let steel_center = main_steel_cluster_center(observation)?;
    let enemy = (enemy_base.x, enemy_base.y);
    let (dir_x, dir_y) = normalized_direction(steel_center, enemy)?;
    let tile_size = observation.map.tile_size as f32;
    if tile_size <= 0.0 {
        return None;
    }
    let front_distance = distance_tiles.max(1.0) * tile_size;
    let line_center = clamp_to_map(
        (
            steel_center.0 + dir_x * front_distance,
            steel_center.1 + dir_y * front_distance,
        ),
        observation.map,
    );
    let perp = (-dir_y, dir_x);
    let spacing = lateral_spacing_tiles.max(0.0) * tile_size;
    let mut units = ready_units.to_vec();
    units.sort_unstable();
    units.dedup();
    let center_index = (formation_slots.max(units.len()).saturating_sub(1)) as f32 * 0.5;

    let assignments = units
        .into_iter()
        .enumerate()
        .map(|(index, unit_id)| {
            let offset = (index as f32 - center_index) * spacing;
            let (x, y) = clamp_to_map(
                (
                    line_center.0 + perp.0 * offset,
                    line_center.1 + perp.1 * offset,
                ),
                observation.map,
            );
            DefensiveLineAssignment { unit_id, x, y }
        })
        .collect();
    Some(assignments)
}

pub(super) fn main_steel_cluster_center(observation: &AiObservation) -> Option<(f32, f32)> {
    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    let radius = (config::START_RESOURCE_MAX_DIST_TILES + 1.5) * observation.map.tile_size as f32;
    let radius2 = squared(radius);
    forward_steel_cluster_center(
        observation
            .resources
            .iter()
            .filter(|resource| dist2(resource.x, resource.y, own_base.0, own_base.1) <= radius2),
        own_base,
        observation.map,
    )
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct JeffBreakthroughResponse {
    pub(super) targets: Vec<u32>,
    pub(super) responders: Vec<u32>,
}

pub(super) fn jeff_breakthrough_response(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    unavailable_units: &BTreeSet<u32>,
    home_defensive_tank: Option<u32>,
) -> JeffBreakthroughResponse {
    let Some(analysis) = map_analysis else {
        return JeffBreakthroughResponse::default();
    };
    let base_count = completed_resource_depot_count(observation).clamp(1, 2);
    let report = analysis.likely_attack_paths(observation.player_id, base_count, 1);
    let tile_size = observation.map.tile_size.max(1) as f32;
    let building_danger2 = squared(BUILDING_DEFENSE_RADIUS_TILES * tile_size);
    let buildings = observation
        .owned
        .iter()
        .filter(|entity| entity.kind.is_building() && entity.hp > 0)
        .collect::<Vec<_>>();
    let mut threats = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind.is_unit() && enemy.hp > 0)
        .filter(|enemy| {
            let immediate_building_danger = buildings.iter().any(|building| {
                dist2(enemy.x, enemy.y, building.x, building.y) <= building_danger2
            });
            immediate_building_danger
                || report.paths.iter().any(|path| {
                    let Some(base) = report
                        .bases
                        .iter()
                        .find(|base| base.id == path.defended_base_id)
                    else {
                        return false;
                    };
                    let base_world = (
                        (base.tile_x as f32 + 0.5) * tile_size,
                        (base.tile_y as f32 + 0.5) * tile_size,
                    );
                    let intercept_world = (
                        (path.intercept_tile_x as f32 + 0.5) * tile_size,
                        (path.intercept_tile_y as f32 + 0.5) * tile_size,
                    );
                    let enemy_distance = dist2(enemy.x, enemy.y, base_world.0, base_world.1).sqrt();
                    let line_distance = dist2(
                        intercept_world.0,
                        intercept_world.1,
                        base_world.0,
                        base_world.1,
                    )
                    .sqrt();
                    enemy_distance <= line_distance + JEFF_BREAKTHROUGH_BUFFER_TILES * tile_size
                })
        })
        .collect::<Vec<_>>();
    threats.sort_by(|left, right| {
        nearest_building_distance2(left, &buildings)
            .total_cmp(&nearest_building_distance2(right, &buildings))
            .then_with(|| left.id.cmp(&right.id))
    });
    if threats.is_empty() {
        return JeffBreakthroughResponse::default();
    }

    let required_value = threats
        .iter()
        .map(|enemy| unit_value(enemy.kind))
        .sum::<u32>();
    let entrenched = observation.upgrades.contains(&UpgradeKind::Entrenchment);
    let covering_units = observation
        .owned
        .iter()
        .filter(|unit| unit.hp > 0 && unit.is_complete && unit.state != AiEntityState::Move)
        .filter(|unit| {
            matches!(
                unit.kind,
                EntityKind::Rifleman | EntityKind::MachineGunner | EntityKind::AntiTankGun
            ) || Some(unit.id) == home_defensive_tank
        })
        .filter(|unit| {
            let range_tiles = preserved_defensive_range_tiles(unit.kind, entrenched);
            let range2 = squared(range_tiles * tile_size);
            threats
                .iter()
                .any(|enemy| dist2(unit.x, unit.y, enemy.x, enemy.y) <= range2)
        })
        .map(|unit| unit.id)
        .collect::<BTreeSet<_>>();
    let covered_value = observation
        .owned
        .iter()
        .filter(|unit| covering_units.contains(&unit.id))
        .map(|unit| unit_value(unit.kind))
        .sum::<u32>();
    let mut uncovered_value = required_value.saturating_sub(covered_value);
    if uncovered_value == 0 {
        return JeffBreakthroughResponse {
            targets: threats.iter().map(|enemy| enemy.id).collect(),
            responders: Vec::new(),
        };
    }

    let armored_threat = threats.iter().any(|enemy| {
        matches!(
            enemy.kind,
            EntityKind::Tank | EntityKind::ScoutCar | EntityKind::CommandCar
        )
    });
    let mut candidates = observation
        .owned
        .iter()
        .filter(|unit| {
            unit.kind.is_unit()
                && unit.hp > 0
                && unit.is_complete
                && unit.free_for_combat
                && defensive_threat_dps(unit) > 0.0
                && unit.kind != EntityKind::AntiTankGun
        })
        .filter(|unit| !unavailable_units.contains(&unit.id))
        // These units already contribute their stationary or entrenched firepower. Selecting one
        // again would double-count its value and throw away the positional benefit we are trying
        // to preserve.
        .filter(|unit| !covering_units.contains(&unit.id))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        responder_priority(left, armored_threat, home_defensive_tank)
            .cmp(&responder_priority(
                right,
                armored_threat,
                home_defensive_tank,
            ))
            .then_with(|| {
                nearest_threat_distance2(left, &threats)
                    .total_cmp(&nearest_threat_distance2(right, &threats))
            })
            .then_with(|| left.id.cmp(&right.id))
    });
    let mut responders = Vec::new();
    for candidate in candidates {
        responders.push(candidate.id);
        uncovered_value = uncovered_value.saturating_sub(unit_value(candidate.kind));
        if uncovered_value == 0 {
            break;
        }
    }

    JeffBreakthroughResponse {
        targets: threats.iter().map(|enemy| enemy.id).collect(),
        responders,
    }
}

fn completed_resource_depot_count(observation: &AiObservation) -> usize {
    observation
        .owned
        .iter()
        .filter(|entity| {
            entity.kind == EntityKind::ResourceDepot && entity.is_complete && entity.hp > 0
        })
        .count()
}

fn preserved_defensive_range_tiles(kind: EntityKind, entrenched: bool) -> f32 {
    match kind {
        EntityKind::Tank => JEFF_STATIONARY_TANK_RANGE_TILES,
        EntityKind::AntiTankGun => config::ANTI_TANK_GUN_DEPLOYED_RANGE_TILES as f32,
        EntityKind::Rifleman | EntityKind::MachineGunner if entrenched => {
            config::unit_stats(kind)
                .map(|stats| stats.range_tiles as f32)
                .unwrap_or(0.0)
                + config::ENTRENCHMENT_RANGE_BONUS_TILES as f32
        }
        _ => config::unit_stats(kind)
            .map(|stats| stats.range_tiles as f32)
            .unwrap_or(0.0),
    }
}

fn responder_priority(
    unit: &AiEntitySummary,
    armored_threat: bool,
    home_defensive_tank: Option<u32>,
) -> (u8, u8) {
    let preservation = if Some(unit.id) == home_defensive_tank {
        4
    } else {
        match unit.kind {
            EntityKind::ScoutCar | EntityKind::CommandCar => 0,
            EntityKind::Tank | EntityKind::Panzerfaust => 1,
            EntityKind::Rifleman => 2,
            EntityKind::MachineGunner | EntityKind::AntiTankGun => 3,
            _ => 2,
        }
    };
    let matchup = if armored_threat {
        match unit.kind {
            EntityKind::AntiTankGun | EntityKind::Panzerfaust | EntityKind::Tank => 0,
            _ => 1,
        }
    } else {
        match unit.kind {
            EntityKind::MachineGunner | EntityKind::Rifleman | EntityKind::ScoutCar => 0,
            _ => 1,
        }
    };
    (preservation, matchup)
}

fn nearest_building_distance2(entity: &AiEntitySummary, buildings: &[&AiEntitySummary]) -> f32 {
    buildings
        .iter()
        .map(|building| dist2(entity.x, entity.y, building.x, building.y))
        .min_by(f32::total_cmp)
        .unwrap_or(f32::MAX)
}

fn nearest_threat_distance2(entity: &AiEntitySummary, threats: &[&AiEntitySummary]) -> f32 {
    threats
        .iter()
        .map(|threat| dist2(entity.x, entity.y, threat.x, threat.y))
        .min_by(f32::total_cmp)
        .unwrap_or(f32::MAX)
}

pub(super) fn stage_jeff_borrowed_defenders_return(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    borrowed_units: &BTreeSet<u32>,
    home_defensive_tank: Option<u32>,
) -> Option<Vec<u32>> {
    let analysis = map_analysis?;
    let report = analysis.likely_attack_paths(
        observation.player_id,
        completed_resource_depot_count(observation).clamp(1, 2),
        1,
    );
    let path = report
        .paths
        .iter()
        .find(|path| path.alternative_rank == 1)?;
    let tile_size = observation.map.tile_size.max(1) as f32;
    let lane = attack_lane_for_path(path, tile_size)?;
    let perpendicular = (-lane.toward_attacker.1, lane.toward_attacker.0);
    let ready = borrowed_units
        .iter()
        .filter_map(|id| observation.owned.iter().find(|unit| unit.id == *id))
        .filter(|unit| {
            unit.free_for_combat
                && !matches!(unit.kind, EntityKind::Rifleman | EntityKind::MachineGunner)
                && Some(unit.id) != home_defensive_tank
        })
        .collect::<Vec<_>>();
    let center_index = ready.len().saturating_sub(1) as f32 * 0.5;
    let tolerance2 = squared(EXPANSION_DEFENSIVE_LINE_REISSUE_EPS_TILES * tile_size);
    let mut staged = Vec::new();
    for (index, unit) in ready.into_iter().enumerate() {
        let lateral = (index as f32 - center_index) * 3.0 * tile_size;
        let target = clamp_to_map(
            (
                lane.center.0 - lane.toward_attacker.0 * JEFF_RESERVE_LINE_DEPTH_TILES * tile_size
                    + perpendicular.0 * lateral,
                lane.center.1 - lane.toward_attacker.1 * JEFF_RESERVE_LINE_DEPTH_TILES * tile_size
                    + perpendicular.1 * lateral,
            ),
            observation.map,
        );
        let command = if dist2(unit.x, unit.y, target.0, target.1) <= tolerance2 {
            actions::hold_position_units(actions, [unit.id])
        } else {
            actions::move_units(actions, [unit.id], target.0, target.1)
        };
        if let Some(units) = command {
            staged.extend(units);
        }
    }
    staged.sort_unstable();
    staged.dedup();
    (!staged.is_empty()).then_some(staged)
}

pub(super) fn local_defense_target(observation: &AiObservation) -> Option<u32> {
    let geometry = LocalDefenseGeometry::from_observation(observation);
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind.is_unit() || enemy.kind.is_building())
        .filter_map(|enemy| {
            geometry
                .contains(enemy)
                .then_some((enemy.id, geometry.base_dist2(enemy)))
        })
        .min_by(|(left_id, left_dist), (right_id, right_dist)| {
            left_dist
                .total_cmp(right_dist)
                .then_with(|| left_id.cmp(right_id))
        })
        .map(|(id, _)| id)
}

pub(super) fn local_defense_units(observation: &AiObservation, ready_units: &[u32]) -> Vec<u32> {
    let geometry = LocalDefenseGeometry::from_observation(observation);
    let ready: BTreeSet<u32> = ready_units.iter().copied().collect();
    observation
        .owned
        .iter()
        .filter(|entity| ready.contains(&entity.id))
        .filter(|entity| geometry.contains(entity))
        .map(|entity| entity.id)
        .collect()
}

pub(super) fn local_defense_targets(observation: &AiObservation) -> BTreeSet<u32> {
    let geometry = LocalDefenseGeometry::from_observation(observation);
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| geometry.contains(enemy))
        .map(|enemy| enemy.id)
        .collect()
}

pub(super) struct LocalDefenseGeometry {
    own_base: (f32, f32),
    base_radius2: f32,
    resource_radius2: f32,
    worker_radius2: f32,
    building_radius2: f32,
    home_resources: Vec<(f32, f32)>,
    workers: Vec<(f32, f32)>,
    buildings: Vec<(f32, f32)>,
}

impl LocalDefenseGeometry {
    fn from_observation(observation: &AiObservation) -> Self {
        let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
        let tile_size = observation.map.tile_size as f32;
        let base_radius2 = squared(LOCAL_DEFENSE_RADIUS_TILES * tile_size);
        let resource_radius2 = squared(RESOURCE_LINE_DEFENSE_RADIUS_TILES * tile_size);
        let worker_radius2 = squared(WORKER_DEFENSE_RADIUS_TILES * tile_size);
        let building_radius2 = squared(BUILDING_DEFENSE_RADIUS_TILES * tile_size);
        let home_resource_radius2 =
            squared((config::START_RESOURCE_MAX_DIST_TILES + 1.5) * tile_size);
        let home_resources = observation
            .resources
            .iter()
            .filter(|resource| {
                matches!(resource.kind, EntityKind::Steel | EntityKind::Oil)
                    && dist2(resource.x, resource.y, own_base.0, own_base.1)
                        <= home_resource_radius2
            })
            .map(|resource| (resource.x, resource.y))
            .collect();
        let workers = observation
            .owned
            .iter()
            .filter(|entity| entity.kind == EntityKind::Worker)
            .map(|worker| (worker.x, worker.y))
            .collect();
        let buildings = observation
            .owned
            .iter()
            .filter(|entity| entity.kind.is_building() && entity.hp > 0)
            .map(|building| (building.x, building.y))
            .collect();

        Self {
            own_base,
            base_radius2,
            resource_radius2,
            worker_radius2,
            building_radius2,
            home_resources,
            workers,
            buildings,
        }
    }

    fn contains(&self, entity: &AiEntitySummary) -> bool {
        self.base_dist2(entity) <= self.base_radius2
            || self
                .home_resources
                .iter()
                .any(|(x, y)| dist2(entity.x, entity.y, *x, *y) <= self.resource_radius2)
            || self
                .workers
                .iter()
                .any(|(x, y)| dist2(entity.x, entity.y, *x, *y) <= self.worker_radius2)
            || self
                .buildings
                .iter()
                .any(|(x, y)| dist2(entity.x, entity.y, *x, *y) <= self.building_radius2)
    }

    fn base_dist2(&self, entity: &AiEntitySummary) -> f32 {
        dist2(entity.x, entity.y, self.own_base.0, self.own_base.1)
    }
}
