use super::geometry::{clamp_to_map, dist2, normalized_direction, squared, tile_center};
use super::resources::forward_steel_cluster_center;
use super::*;

pub(super) const LOCAL_DEFENSE_RADIUS_TILES: f32 = 12.0;

pub(super) const RESOURCE_LINE_DEFENSE_RADIUS_TILES: f32 = 4.0;

pub(super) const WORKER_DEFENSE_RADIUS_TILES: f32 = 5.0;

pub(super) const EXPANSION_DEFENSIVE_LINE_SPACING_TILES: f32 = 1.5;

pub(super) const EXPANSION_DEFENSIVE_LINE_REISSUE_EPS_TILES: f32 = 0.75;

pub(super) const DEFENSIVE_MG_PERIMETER_DISTANCE_TILES: f32 = 20.0;

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
    let assignments = main_steel_defensive_line_assignments(
        observation,
        ready_units,
        enemy_base,
        distance_tiles,
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
    let Some(policy) = profile.defensive_machine_gunners else {
        return Vec::new();
    };
    let mut units =
        actions::select_ready_combat_units(&observation.owned, &[EntityKind::MachineGunner]);
    units.truncate(policy.target_count);
    units
}

pub(super) fn stage_defensive_machine_gunner_perimeter(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    ready_units: &[u32],
    enemy_base: EnemyBaseFact,
) -> Option<Vec<u32>> {
    stage_main_steel_defensive_line(
        actions,
        observation,
        ready_units,
        enemy_base,
        DEFENSIVE_MG_PERIMETER_DISTANCE_TILES,
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
    let spacing = EXPANSION_DEFENSIVE_LINE_SPACING_TILES * tile_size;
    let mut units = ready_units.to_vec();
    units.sort_unstable();
    units.dedup();
    let center_index = (units.len().saturating_sub(1)) as f32 * 0.5;

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
    let radius = (config::CC_RESOURCE_MAX_DIST_TILES + 1.5) * observation.map.tile_size as f32;
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
    home_resources: Vec<(f32, f32)>,
    workers: Vec<(f32, f32)>,
}

impl LocalDefenseGeometry {
    fn from_observation(observation: &AiObservation) -> Self {
        let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
        let tile_size = observation.map.tile_size as f32;
        let base_radius2 = squared(LOCAL_DEFENSE_RADIUS_TILES * tile_size);
        let resource_radius2 = squared(RESOURCE_LINE_DEFENSE_RADIUS_TILES * tile_size);
        let worker_radius2 = squared(WORKER_DEFENSE_RADIUS_TILES * tile_size);
        let home_resource_radius2 = squared((config::CC_RESOURCE_MAX_DIST_TILES + 1.5) * tile_size);
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

        Self {
            own_base,
            base_radius2,
            resource_radius2,
            worker_radius2,
            home_resources,
            workers,
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
    }

    fn base_dist2(&self, entity: &AiEntitySummary) -> f32 {
        dist2(entity.x, entity.y, self.own_base.0, self.own_base.1)
    }
}
