use std::collections::BTreeMap;

use crate::config;
use crate::game::ai_core::observation::{
    AiBuildIntentPhase, AiEntityState, AiObservation, AiResourceSummary,
};
use crate::game::entity::EntityKind;
use crate::rules;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct BuildingCounts {
    pub(crate) existing: usize,
    pub(crate) complete: usize,
    pub(crate) incomplete: usize,
    pub(crate) intended: usize,
}

impl BuildingCounts {
    pub(crate) fn total_planned(self) -> usize {
        self.existing + self.intended
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProductionBuildingFact {
    pub(crate) id: u32,
    pub(crate) kind: EntityKind,
    pub(crate) queue_len: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EnemyBaseFact {
    pub(crate) player_id: u32,
    pub(crate) start_tile: (u32, u32),
    pub(crate) x: f32,
    pub(crate) y: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AiFacts {
    pub(crate) player_id: u32,
    pub(crate) worker_count: usize,
    pub(crate) idle_workers: Vec<u32>,
    pub(crate) gathering_workers: Vec<u32>,
    pub(crate) build_capable_workers: Vec<u32>,
    pub(crate) target_steel_workers: usize,
    pub(crate) free_supply: u32,
    pub(crate) supply_blocked_or_near_blocked: bool,
    pub(crate) supply_capped: bool,
    pub(crate) depot_in_progress: bool,
    pub(crate) committed_steel: u32,
    pub(crate) nearest_public_enemy_base: Option<EnemyBaseFact>,
    building_counts: BTreeMap<EntityKind, BuildingCounts>,
    complete_building_kinds: Vec<EntityKind>,
    production_buildings: BTreeMap<EntityKind, Vec<ProductionBuildingFact>>,
    unit_counts: BTreeMap<EntityKind, usize>,
    free_combat_units: BTreeMap<EntityKind, Vec<u32>>,
}

impl AiFacts {
    pub(crate) fn from_observation(observation: &AiObservation) -> Self {
        let mut worker_count = 0;
        let mut idle_workers = Vec::new();
        let mut gathering_workers = Vec::new();
        let mut build_capable_workers = Vec::new();
        let mut building_counts: BTreeMap<EntityKind, BuildingCounts> = BTreeMap::new();
        let mut complete_building_kinds = Vec::new();
        let mut production_buildings: BTreeMap<EntityKind, Vec<ProductionBuildingFact>> =
            BTreeMap::new();
        let mut unit_counts: BTreeMap<EntityKind, usize> = BTreeMap::new();
        let mut free_combat_units: BTreeMap<EntityKind, Vec<u32>> = BTreeMap::new();

        for entity in &observation.owned {
            if entity.kind.is_unit() {
                *unit_counts.entry(entity.kind).or_default() += 1;
            }

            if entity.kind == EntityKind::Worker {
                worker_count += 1;
                match entity.state {
                    AiEntityState::Idle => {
                        idle_workers.push(entity.id);
                        build_capable_workers.push(entity.id);
                    }
                    AiEntityState::Gather => {
                        gathering_workers.push(entity.id);
                        build_capable_workers.push(entity.id);
                    }
                    AiEntityState::Build => {}
                    _ => build_capable_workers.push(entity.id),
                }
            }

            if entity.kind.is_building() {
                let counts = building_counts.entry(entity.kind).or_default();
                counts.existing += 1;
                if entity.is_complete {
                    counts.complete += 1;
                    complete_building_kinds.push(entity.kind);
                } else {
                    counts.incomplete += 1;
                }
            }

            if entity.is_complete {
                if let Some(queue_len) = entity.production_queue_len {
                    production_buildings.entry(entity.kind).or_default().push(
                        ProductionBuildingFact {
                            id: entity.id,
                            kind: entity.kind,
                            queue_len,
                        },
                    );
                }
            }

            if is_combat_unit(entity.kind) && entity.free_for_combat {
                free_combat_units
                    .entry(entity.kind)
                    .or_default()
                    .push(entity.id);
            }
        }

        for intent in &observation.pending_builds {
            if intent.phase == AiBuildIntentPhase::ToSite {
                building_counts.entry(intent.kind).or_default().intended += 1;
            }
        }

        idle_workers.sort_unstable();
        gathering_workers.sort_unstable();
        build_capable_workers.sort_unstable();
        build_capable_workers.dedup();
        complete_building_kinds.sort_unstable();
        complete_building_kinds.dedup();
        for buildings in production_buildings.values_mut() {
            buildings.sort_by_key(|b| b.id);
        }
        for units in free_combat_units.values_mut() {
            units.sort_unstable();
        }

        let free_supply = observation
            .economy
            .supply_cap
            .saturating_sub(observation.economy.supply_used);
        let supply_capped = observation.economy.supply_cap >= config::SUPPLY_CAP_MAX;
        let depot_counts = building_counts
            .get(&EntityKind::Depot)
            .copied()
            .unwrap_or_default();
        let depot_in_progress = depot_counts.incomplete + depot_counts.intended > 0;
        let committed_steel = observation
            .pending_builds
            .iter()
            .filter(|intent| intent.phase == AiBuildIntentPhase::ToSite)
            .map(|intent| rules::economy::cost(intent.kind).0)
            .fold(0u32, u32::saturating_add);
        let nearest_public_enemy_base = nearest_public_enemy_base(observation);

        Self {
            player_id: observation.player_id,
            worker_count,
            idle_workers,
            gathering_workers,
            build_capable_workers,
            target_steel_workers: main_base_steel_saturation_target(
                observation.own_start_tile,
                observation.map.tile_size,
                observation.resources.iter().copied(),
            ),
            free_supply,
            supply_blocked_or_near_blocked: free_supply <= 2 && !supply_capped,
            supply_capped,
            depot_in_progress,
            committed_steel,
            nearest_public_enemy_base,
            building_counts,
            complete_building_kinds,
            production_buildings,
            unit_counts,
            free_combat_units,
        }
    }

    pub(crate) fn building_count(&self, kind: EntityKind) -> usize {
        self.building_counts
            .get(&kind)
            .copied()
            .unwrap_or_default()
            .total_planned()
    }

    #[allow(dead_code)]
    pub(crate) fn complete_building_count(&self, kind: EntityKind) -> usize {
        self.building_counts
            .get(&kind)
            .map(|counts| counts.complete)
            .unwrap_or(0)
    }

    #[allow(dead_code)]
    pub(crate) fn building_counts(&self, kind: EntityKind) -> BuildingCounts {
        self.building_counts.get(&kind).copied().unwrap_or_default()
    }

    #[allow(dead_code)]
    pub(crate) fn complete_building_kinds(&self) -> &[EntityKind] {
        &self.complete_building_kinds
    }

    #[allow(dead_code)]
    pub(crate) fn unit_count(&self, kind: EntityKind) -> usize {
        self.unit_counts.get(&kind).copied().unwrap_or(0)
    }

    pub(crate) fn production_buildings(&self, kind: EntityKind) -> &[ProductionBuildingFact] {
        self.production_buildings
            .get(&kind)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    #[allow(dead_code)]
    pub(crate) fn free_combat_units(&self, kind: EntityKind) -> &[u32] {
        self.free_combat_units
            .get(&kind)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

pub(crate) fn main_base_steel_saturation_target(
    start_tile: (u32, u32),
    tile_size: u32,
    resources: impl IntoIterator<Item = AiResourceSummary>,
) -> usize {
    let (hx, hy) = (
        start_tile.0 as f32 * tile_size as f32 + tile_size as f32 * 0.5,
        start_tile.1 as f32 * tile_size as f32 + tile_size as f32 * 0.5,
    );
    let max_dist_px = (config::IC_RESOURCE_MAX_DIST_TILES + 0.5) * tile_size as f32;
    let max_dist2 = max_dist_px * max_dist_px;
    resources
        .into_iter()
        .filter(|r| r.kind == EntityKind::Steel && r.remaining > 0)
        .filter(|r| {
            let dx = r.x - hx;
            let dy = r.y - hy;
            dx * dx + dy * dy <= max_dist2
        })
        .count()
}

fn nearest_public_enemy_base(observation: &AiObservation) -> Option<EnemyBaseFact> {
    let own_start = observation.own_start_tile;
    let ts = observation.map.tile_size as f32;
    let own_x = own_start.0 as f32 * ts + ts * 0.5;
    let own_y = own_start.1 as f32 * ts + ts * 0.5;

    let mut best: Option<(EnemyBaseFact, f32)> = None;
    for player in observation
        .players
        .iter()
        .filter(|p| p.id != observation.player_id && p.is_alive)
    {
        let x = player.start_tile.0 as f32 * ts + ts * 0.5;
        let y = player.start_tile.1 as f32 * ts + ts * 0.5;
        let dx = x - own_x;
        let dy = y - own_y;
        let dist2 = dx * dx + dy * dy;
        let fact = EnemyBaseFact {
            player_id: player.id,
            start_tile: player.start_tile,
            x,
            y,
        };
        let better = best
            .as_ref()
            .map(|(current, best_dist2)| {
                dist2 < *best_dist2 || (dist2 == *best_dist2 && player.id < current.player_id)
            })
            .unwrap_or(true);
        if better {
            best = Some((fact, dist2));
        }
    }
    best.map(|(fact, _)| fact)
}

fn is_combat_unit(kind: EntityKind) -> bool {
    match kind {
        EntityKind::Rifleman
        | EntityKind::MachineGunner
        | EntityKind::AtTeam
        | EntityKind::Tank => true,
        EntityKind::Worker
        | EntityKind::IndustrialCenter
        | EntityKind::Depot
        | EntityKind::Barracks
        | EntityKind::TrainingCentre
        | EntityKind::TankFactory
        | EntityKind::Steel
        | EntityKind::Oil => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ai_core::observation::{
        AiEconomy, AiEntitySummary, AiMapSummary, AiPlayerSummary,
    };
    use crate::game::entity::{EntityStore, Order};
    use crate::game::map::Map;
    use crate::game::PlayerState;
    use crate::protocol::{terrain, EntityView, MapInfo, PlayerStart, Snapshot, StartPayload};

    fn base_observation() -> AiObservation {
        AiObservation {
            player_id: 1,
            tick: 0,
            map: AiMapSummary {
                width: 64,
                height: 64,
                tile_size: config::TILE_SIZE,
            },
            economy: AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 8,
                supply_cap: 10,
            },
            own_start_tile: (10, 20),
            players: vec![AiPlayerSummary {
                id: 1,
                start_tile: (10, 20),
                is_ai: false,
                is_alive: true,
            }],
            owned: Vec::new(),
            resources: Vec::new(),
            visible_enemies: Vec::new(),
            pending_builds: Vec::new(),
        }
    }

    fn owned_entity(id: u32, kind: EntityKind, state: AiEntityState) -> AiEntitySummary {
        AiEntitySummary {
            id,
            owner: 1,
            kind,
            x: 0.0,
            y: 0.0,
            state,
            is_complete: true,
            production_queue_len: None,
            production_kind: None,
            latched_node: None,
            target_id: None,
            free_for_combat: state == AiEntityState::Idle,
        }
    }

    #[test]
    fn steel_saturation_from_live_and_snapshot_observations_agree() {
        let map = Map::generate(2, 1234);
        let (hx, hy) = (
            10.5 * config::TILE_SIZE as f32,
            20.5 * config::TILE_SIZE as f32,
        );
        let in_range = (config::IC_RESOURCE_MAX_DIST_TILES - 0.25) * config::TILE_SIZE as f32;
        let out_of_range = (config::IC_RESOURCE_MAX_DIST_TILES + 2.0) * config::TILE_SIZE as f32;

        let mut entities = EntityStore::new();
        entities
            .spawn_node(EntityKind::Steel, hx + in_range, hy)
            .unwrap();
        entities
            .spawn_node(EntityKind::Steel, hx - in_range, hy)
            .unwrap();
        entities
            .spawn_node(EntityKind::Oil, hx, hy + in_range)
            .unwrap();
        entities
            .spawn_node(EntityKind::Steel, hx, hy + out_of_range)
            .unwrap();
        let depleted = entities
            .spawn_node(EntityKind::Steel, hx, hy - in_range)
            .unwrap();
        entities
            .get_mut(depleted)
            .unwrap()
            .resource_node
            .as_mut()
            .unwrap()
            .remaining = 0;

        let players = vec![PlayerState {
            id: 1,
            name: "Alpha".into(),
            color: "#111".into(),
            start_tile: (10, 20),
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 0,
            is_ai: true,
        }];
        let mut fog = crate::game::fog::Fog::new(map.size);
        fog.recompute(&[1], &entities);
        let live = AiObservation::from_live_state(&map, &entities, &fog, &players, 1, 0).unwrap();
        let live_facts = AiFacts::from_observation(&live);

        let snapshot = Snapshot {
            tick: 0,
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 0,
            entities: live
                .resources
                .iter()
                .map(|r| {
                    let mut view = EntityView::new(
                        r.id,
                        0,
                        r.kind.to_protocol_str(),
                        r.x,
                        r.y,
                        1,
                        1,
                        crate::protocol::states::IDLE,
                    );
                    view.remaining = Some(r.remaining);
                    view
                })
                .collect(),
            resource_deltas: Vec::new(),
            events: Vec::new(),
        };
        let start = StartPayload {
            player_id: 1,
            tick: 0,
            map: MapInfo {
                width: map.size,
                height: map.size,
                tile_size: config::TILE_SIZE,
                terrain: vec![terrain::GRASS; (map.size * map.size) as usize],
                resources: Vec::new(),
            },
            players: vec![PlayerStart {
                id: 1,
                name: "Alpha".into(),
                color: "#111".into(),
                start_tile_x: 10,
                start_tile_y: 20,
            }],
        };
        let selfplay = AiObservation::from_selfplay_snapshot(&start, &snapshot, 1, []).unwrap();
        let selfplay_facts = AiFacts::from_observation(&selfplay);

        assert_eq!(live_facts.target_steel_workers, 2);
        assert_eq!(
            selfplay_facts.target_steel_workers,
            live_facts.target_steel_workers
        );
    }

    #[test]
    fn pending_build_intent_counts_once_as_planned_building() {
        let map = Map::generate(2, 1234);
        let mut entities = EntityStore::new();
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, 32.0, 32.0)
            .unwrap();
        entities
            .get_mut(worker)
            .unwrap()
            .set_order(Order::build(EntityKind::Depot, 5, 6));
        let players = vec![PlayerState {
            id: 1,
            name: "Alpha".into(),
            color: "#111".into(),
            start_tile: (8, 8),
            steel: 100,
            oil: 0,
            supply_used: 8,
            supply_cap: 10,
            is_ai: true,
        }];

        let mut fog = crate::game::fog::Fog::new(map.size);
        fog.recompute(&[1], &entities);
        let observation =
            AiObservation::from_live_state(&map, &entities, &fog, &players, 1, 0).unwrap();
        let facts = AiFacts::from_observation(&observation);
        let counts = facts.building_counts(EntityKind::Depot);

        assert_eq!(facts.building_count(EntityKind::Depot), 1);
        assert_eq!(facts.complete_building_count(EntityKind::Depot), 0);
        assert_eq!(counts.existing, 0);
        assert_eq!(counts.intended, 1);
        assert!(facts.depot_in_progress);
        assert_eq!(
            facts.committed_steel,
            rules::economy::cost(EntityKind::Depot).0
        );
    }

    #[test]
    fn production_queue_facts_are_sorted_and_stable() {
        let mut observation = base_observation();
        let mut first = owned_entity(20, EntityKind::Barracks, AiEntityState::Train);
        first.production_queue_len = Some(2);
        let mut second = owned_entity(10, EntityKind::Barracks, AiEntityState::Idle);
        second.production_queue_len = Some(0);
        observation.owned = vec![first, second];

        let facts = AiFacts::from_observation(&observation);
        let production = facts.production_buildings(EntityKind::Barracks);

        assert_eq!(
            production,
            &[
                ProductionBuildingFact {
                    id: 10,
                    kind: EntityKind::Barracks,
                    queue_len: 0
                },
                ProductionBuildingFact {
                    id: 20,
                    kind: EntityKind::Barracks,
                    queue_len: 2
                },
            ]
        );
    }

    #[test]
    fn nearest_enemy_start_tile_tie_breaks_by_player_id() {
        let mut observation = base_observation();
        observation.own_start_tile = (10, 10);
        observation.players = vec![
            AiPlayerSummary {
                id: 1,
                start_tile: (10, 10),
                is_ai: false,
                is_alive: true,
            },
            AiPlayerSummary {
                id: 3,
                start_tile: (12, 10),
                is_ai: false,
                is_alive: true,
            },
            AiPlayerSummary {
                id: 2,
                start_tile: (8, 10),
                is_ai: false,
                is_alive: true,
            },
        ];

        let facts = AiFacts::from_observation(&observation);

        assert_eq!(facts.nearest_public_enemy_base.unwrap().player_id, 2);
    }

    #[test]
    fn nearest_enemy_start_tile_ignores_dead_players() {
        let mut observation = base_observation();
        observation.own_start_tile = (10, 10);
        observation.players = vec![
            AiPlayerSummary {
                id: 1,
                start_tile: (10, 10),
                is_ai: false,
                is_alive: true,
            },
            AiPlayerSummary {
                id: 2,
                start_tile: (11, 10),
                is_ai: false,
                is_alive: false,
            },
            AiPlayerSummary {
                id: 3,
                start_tile: (14, 10),
                is_ai: false,
                is_alive: true,
            },
        ];

        let facts = AiFacts::from_observation(&observation);

        assert_eq!(facts.nearest_public_enemy_base.unwrap().player_id, 3);
    }

    #[test]
    fn free_combat_unit_selection_ignores_busy_units_and_workers() {
        let mut observation = base_observation();
        observation.owned = vec![
            owned_entity(3, EntityKind::Worker, AiEntityState::Idle),
            owned_entity(2, EntityKind::Rifleman, AiEntityState::Move),
            owned_entity(1, EntityKind::Rifleman, AiEntityState::Idle),
        ];

        let facts = AiFacts::from_observation(&observation);

        assert_eq!(facts.worker_count, 1);
        assert_eq!(facts.unit_count(EntityKind::Worker), 1);
        assert_eq!(facts.unit_count(EntityKind::Rifleman), 2);
        assert_eq!(facts.free_combat_units(EntityKind::Rifleman), &[1]);
        assert!(facts.free_combat_units(EntityKind::Worker).is_empty());
    }
}
