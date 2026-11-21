//! Authoritative last-observed resource quantities, including durable depletion records.
use super::{
    entity::{EntityKind, EntityStore},
    fog::Fog,
};
use crate::protocol::ResourceDelta;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::game) struct ResourceMemory {
    pub(in crate::game) nodes: BTreeMap<u32, ResourceKnowledge>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::game) struct ResourceKnowledge {
    // Older checkpoints can recover this from a still-live node during restore.
    #[serde(default)]
    pub(in crate::game) kind: Option<EntityKind>,
    pub(in crate::game) x: f32,
    pub(in crate::game) y: f32,
    pub(in crate::game) initial_remaining: u32,
    pub(in crate::game) observations: BTreeMap<u32, ResourceObservation>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::game) struct ResourceObservation {
    pub(in crate::game) remaining: u32,
    pub(in crate::game) tick: u32,
}

impl ResourceMemory {
    pub(in crate::game) fn seed(&mut self, entities: &EntityStore) {
        for entity in entities.iter().filter(|e| e.is_node()) {
            let node = self
                .nodes
                .entry(entity.id)
                .or_insert_with(|| ResourceKnowledge {
                    kind: Some(entity.kind),
                    x: entity.pos_x(),
                    y: entity.pos_y(),
                    initial_remaining: entity.remaining().unwrap_or(0),
                    observations: BTreeMap::new(),
                });
            node.kind.get_or_insert(entity.kind);
        }
    }

    pub(in crate::game) fn refresh_for_player(
        &mut self,
        player: u32,
        viewers: &[u32],
        entities: &EntityStore,
        fog: &Fog,
        tick: u32,
    ) {
        for (&id, node) in &mut self.nodes {
            if !viewers
                .iter()
                .any(|&viewer| fog.is_visible_world(viewer, node.x, node.y))
            {
                continue;
            }
            let remaining = entities.get(id).and_then(|e| e.remaining()).unwrap_or(0);
            node.observations
                .insert(player, ResourceObservation { remaining, tick });
        }
    }

    pub(in crate::game) fn project(
        &self,
        players: &[u32],
        omniscient: bool,
        entities: &EntityStore,
    ) -> Vec<ResourceDelta> {
        self.nodes
            .iter()
            .map(|(&id, node)| {
                let remaining = if omniscient {
                    entities.get(id).and_then(|e| e.remaining()).unwrap_or(0)
                } else {
                    players
                        .iter()
                        .filter_map(|p| node.observations.get(p))
                        .max_by_key(|observation| {
                            (observation.tick, std::cmp::Reverse(observation.remaining))
                        })
                        .map_or(node.initial_remaining, |observation| observation.remaining)
                };
                ResourceDelta { id, remaining }
            })
            .collect()
    }
}
