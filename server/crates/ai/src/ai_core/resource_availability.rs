#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use crate::ai_core::observation::{AiEntityState, AiObservation};
use crate::config;
use rts_sim::game::entity::EntityKind;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ResourceNodeAvailability {
    pub(crate) id: u32,
    pub(crate) kind: EntityKind,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) remaining: u32,
    pub(crate) has_remaining: bool,
    pub(crate) mineable_now: bool,
    pub(crate) nearest_completed_mining_city_centre: Option<u32>,
    pub(crate) latched_worker_count: usize,
    pub(crate) occupied: bool,
    pub(crate) pre_reserved: bool,
    pub(crate) future_expansion_candidate: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ResourceAvailability {
    nodes: Vec<ResourceNodeAvailability>,
    by_id: BTreeMap<u32, usize>,
    occupied_by_kind: BTreeMap<EntityKind, usize>,
    latched_by_kind: BTreeMap<EntityKind, usize>,
}

impl ResourceAvailability {
    pub(crate) fn from_observation(
        observation: &AiObservation,
        pre_reserved_nodes: &BTreeSet<u32>,
    ) -> Self {
        let completed_city_centres = completed_city_centres(observation);
        let latched_workers_by_node = latched_workers_by_node(observation);
        let mut occupied_by_kind = BTreeMap::new();
        let mut latched_by_kind = BTreeMap::new();

        let mut nodes: Vec<ResourceNodeAvailability> = observation
            .resources
            .iter()
            .map(|resource| {
                let nearest_completed_mining_city_centre = nearest_completed_mining_city_centre(
                    resource.x,
                    resource.y,
                    &completed_city_centres,
                );
                let latched_worker_count = latched_workers_by_node
                    .get(&resource.id)
                    .copied()
                    .unwrap_or(0);
                let has_remaining = resource.remaining > 0;
                let mineable_now = has_remaining && nearest_completed_mining_city_centre.is_some();
                let occupied = latched_worker_count > 0;
                if occupied {
                    *occupied_by_kind.entry(resource.kind).or_default() += 1;
                }
                if latched_worker_count > 0 {
                    *latched_by_kind.entry(resource.kind).or_default() += latched_worker_count;
                }
                ResourceNodeAvailability {
                    id: resource.id,
                    kind: resource.kind,
                    x: resource.x,
                    y: resource.y,
                    remaining: resource.remaining,
                    has_remaining,
                    mineable_now,
                    nearest_completed_mining_city_centre,
                    latched_worker_count,
                    occupied,
                    pre_reserved: pre_reserved_nodes.contains(&resource.id),
                    future_expansion_candidate: resource.kind.is_node()
                        && has_remaining
                        && !mineable_now,
                }
            })
            .collect();
        nodes.sort_by_key(|node| node.id);
        let by_id = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id, index))
            .collect();

        Self {
            nodes,
            by_id,
            occupied_by_kind,
            latched_by_kind,
        }
    }

    pub(crate) fn nodes(&self) -> &[ResourceNodeAvailability] {
        &self.nodes
    }

    pub(crate) fn node(&self, id: u32) -> Option<&ResourceNodeAvailability> {
        self.by_id.get(&id).map(|index| &self.nodes[*index])
    }

    pub(crate) fn free_mineable_nodes(
        &self,
        kind: EntityKind,
    ) -> impl Iterator<Item = &ResourceNodeAvailability> {
        self.nodes.iter().filter(move |node| {
            node.kind == kind && node.mineable_now && !node.occupied && !node.pre_reserved
        })
    }

    pub(crate) fn occupied_node_count(&self, kind: EntityKind) -> usize {
        self.occupied_by_kind.get(&kind).copied().unwrap_or(0)
    }

    pub(crate) fn latched_worker_count(&self, kind: EntityKind) -> usize {
        self.latched_by_kind.get(&kind).copied().unwrap_or(0)
    }

    pub(crate) fn current_steel_saturation_target(&self) -> usize {
        self.nodes
            .iter()
            .filter(|node| {
                node.kind == EntityKind::Steel
                    && node.mineable_now
                    && node.has_remaining
                    && !node.pre_reserved
            })
            .count()
    }

    pub(crate) fn has_free_mineable_oil(&self) -> bool {
        self.free_mineable_nodes(EntityKind::Oil).next().is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MiningCityCentre {
    id: u32,
    x: f32,
    y: f32,
}

fn completed_city_centres(observation: &AiObservation) -> Vec<MiningCityCentre> {
    observation
        .owned
        .iter()
        .filter(|entity| {
            entity.kind == EntityKind::CityCentre
                && entity.is_complete
                && entity.state != AiEntityState::Dead
        })
        .map(|entity| MiningCityCentre {
            id: entity.id,
            x: entity.x,
            y: entity.y,
        })
        .collect()
}

fn latched_workers_by_node(observation: &AiObservation) -> BTreeMap<u32, usize> {
    let mut counts = BTreeMap::new();
    for worker in observation
        .owned
        .iter()
        .filter(|entity| entity.kind == EntityKind::Worker)
    {
        if let Some(node) = worker.latched_node {
            *counts.entry(node).or_default() += 1;
        }
    }
    counts
}

fn nearest_completed_mining_city_centre(
    x: f32,
    y: f32,
    city_centres: &[MiningCityCentre],
) -> Option<u32> {
    let range_px = config::MINING_CC_RANGE_TILES * config::TILE_SIZE as f32;
    let range2 = range_px * range_px + 0.01;
    city_centres
        .iter()
        .filter_map(|cc| {
            let d = dist2(x, y, cc.x, cc.y);
            (d <= range2).then_some((cc.id, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)))
        .map(|(id, _)| id)
}

fn dist2(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = ax - bx;
    let dy = ay - by;
    dx * dx + dy * dy
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_core::observation::{
        AiEconomy, AiEntitySummary, AiMapSummary, AiPlayerSummary, AiResourceSummary,
    };

    fn observation() -> AiObservation {
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
                supply_used: 0,
                supply_cap: 10,
            },
            own_start_tile: (10, 10),
            players: vec![AiPlayerSummary {
                id: 1,
                team_id: 1,
                start_tile: (10, 10),
                is_ai: true,
                is_alive: true,
            }],
            owned: Vec::new(),
            resources: Vec::new(),
            visible_allies: Vec::new(),
            visible_enemies: Vec::new(),
            pending_builds: Vec::new(),
            upgrades: Vec::new(),
        }
    }

    fn entity(id: u32, kind: EntityKind, x: f32, y: f32) -> AiEntitySummary {
        AiEntitySummary {
            id,
            owner: 1,
            kind,
            x,
            y,
            state: AiEntityState::Idle,
            is_complete: true,
            production_queue_len: None,
            production_kind: None,
            latched_node: None,
            target_id: None,
            free_for_combat: false,
        }
    }

    fn city_centre(id: u32, x: f32, y: f32, complete: bool) -> AiEntitySummary {
        let mut entity = entity(id, EntityKind::CityCentre, x, y);
        entity.is_complete = complete;
        entity
    }

    fn worker(id: u32, latched_node: Option<u32>) -> AiEntitySummary {
        let mut entity = entity(id, EntityKind::Worker, 0.0, 0.0);
        entity.latched_node = latched_node;
        entity.state = if latched_node.is_some() {
            AiEntityState::Gather
        } else {
            AiEntityState::Idle
        };
        entity
    }

    fn resource(id: u32, kind: EntityKind, x: f32, y: f32, remaining: u32) -> AiResourceSummary {
        AiResourceSummary {
            id,
            kind,
            x,
            y,
            remaining,
        }
    }

    #[test]
    fn resource_in_completed_city_centre_range_is_mineable() {
        let mut observation = observation();
        let ts = config::TILE_SIZE as f32;
        observation.owned = vec![city_centre(10, 100.0, 100.0, true)];
        observation.resources = vec![
            resource(
                2,
                EntityKind::Steel,
                100.0 + config::MINING_CC_RANGE_TILES * ts,
                100.0,
                100,
            ),
            resource(
                1,
                EntityKind::Steel,
                100.0,
                100.0 + (config::MINING_CC_RANGE_TILES + 0.25) * ts,
                100,
            ),
        ];

        let availability = ResourceAvailability::from_observation(&observation, &BTreeSet::new());

        assert_eq!(availability.nodes()[0].id, 1);
        assert!(!availability.node(1).unwrap().mineable_now);
        assert!(availability.node(1).unwrap().future_expansion_candidate);
        assert!(availability.node(2).unwrap().mineable_now);
        assert_eq!(
            availability
                .node(2)
                .unwrap()
                .nearest_completed_mining_city_centre,
            Some(10)
        );
    }

    #[test]
    fn incomplete_city_centre_does_not_make_resource_mineable() {
        let mut observation = observation();
        observation.owned = vec![city_centre(10, 100.0, 100.0, false)];
        observation.resources = vec![resource(1, EntityKind::Oil, 100.0, 100.0, 100)];

        let availability = ResourceAvailability::from_observation(&observation, &BTreeSet::new());

        assert!(!availability.node(1).unwrap().mineable_now);
        assert_eq!(
            availability
                .node(1)
                .unwrap()
                .nearest_completed_mining_city_centre,
            None
        );
        assert!(!availability.has_free_mineable_oil());
    }

    #[test]
    fn depleted_resource_is_not_free_or_mineable() {
        let mut observation = observation();
        observation.owned = vec![city_centre(10, 100.0, 100.0, true)];
        observation.resources = vec![resource(1, EntityKind::Steel, 100.0, 100.0, 0)];

        let availability = ResourceAvailability::from_observation(&observation, &BTreeSet::new());

        let node = availability.node(1).unwrap();
        assert!(!node.has_remaining);
        assert!(!node.mineable_now);
        assert_eq!(availability.current_steel_saturation_target(), 0);
        assert_eq!(
            availability.free_mineable_nodes(EntityKind::Steel).count(),
            0
        );
    }

    #[test]
    fn latched_workers_and_pre_reserved_nodes_count_as_occupied_not_free() {
        let mut observation = observation();
        observation.owned = vec![
            city_centre(10, 100.0, 100.0, true),
            worker(20, Some(1)),
            worker(21, Some(1)),
        ];
        observation.resources = vec![
            resource(1, EntityKind::Steel, 100.0, 100.0, 100),
            resource(2, EntityKind::Steel, 101.0, 100.0, 100),
            resource(3, EntityKind::Oil, 102.0, 100.0, 100),
        ];
        let pre_reserved_nodes = BTreeSet::from([2]);

        let availability =
            ResourceAvailability::from_observation(&observation, &pre_reserved_nodes);

        let latched = availability.node(1).unwrap();
        assert_eq!(latched.latched_worker_count, 2);
        assert!(latched.occupied);
        assert_eq!(availability.occupied_node_count(EntityKind::Steel), 1);
        assert_eq!(availability.latched_worker_count(EntityKind::Steel), 2);
        assert!(availability.node(2).unwrap().pre_reserved);
        assert_eq!(
            availability.free_mineable_nodes(EntityKind::Steel).count(),
            0
        );
        assert!(availability.has_free_mineable_oil());
    }

    #[test]
    fn nearest_city_centre_tie_breaks_by_id() {
        let mut observation = observation();
        observation.owned = vec![
            city_centre(11, 90.0, 100.0, true),
            city_centre(10, 110.0, 100.0, true),
        ];
        observation.resources = vec![resource(1, EntityKind::Steel, 100.0, 100.0, 100)];

        let availability = ResourceAvailability::from_observation(&observation, &BTreeSet::new());

        assert_eq!(
            availability
                .node(1)
                .unwrap()
                .nearest_completed_mining_city_centre,
            Some(10)
        );
    }
}
