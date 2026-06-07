use std::collections::BTreeMap;

use crate::game::entity::{BuildPhase, Entity, EntityKind, EntityStore, Order, NEUTRAL};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::PlayerState;
use crate::protocol::{states, EntityView, Snapshot, StartPayload};
use crate::rules;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AiMapSummary {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) tile_size: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AiEconomy {
    pub(crate) steel: u32,
    pub(crate) oil: u32,
    pub(crate) supply_used: u32,
    pub(crate) supply_cap: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AiPlayerSummary {
    pub(crate) id: u32,
    pub(crate) start_tile: (u32, u32),
    pub(crate) is_ai: bool,
    pub(crate) is_alive: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AiEntityState {
    Idle,
    Move,
    Attack,
    Gather,
    Build,
    Train,
    Construct,
    Dead,
    Unknown,
}

impl AiEntityState {
    pub(crate) fn from_protocol_state(state: &str) -> Self {
        match state {
            states::IDLE => Self::Idle,
            states::MOVE => Self::Move,
            states::ATTACK => Self::Attack,
            states::GATHER => Self::Gather,
            states::BUILD => Self::Build,
            states::TRAIN => Self::Train,
            states::CONSTRUCT => Self::Construct,
            states::DEAD => Self::Dead,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AiBuildIntentPhase {
    ToSite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct AiBuildIntent {
    pub(crate) worker_id: u32,
    pub(crate) kind: EntityKind,
    pub(crate) tile_x: u32,
    pub(crate) tile_y: u32,
    pub(crate) phase: AiBuildIntentPhase,
}

impl AiBuildIntent {
    pub(crate) fn to_site(worker_id: u32, kind: EntityKind, tile_x: u32, tile_y: u32) -> Self {
        Self {
            worker_id,
            kind,
            tile_x,
            tile_y,
            phase: AiBuildIntentPhase::ToSite,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AiEntitySummary {
    pub(crate) id: u32,
    pub(crate) owner: u32,
    pub(crate) kind: EntityKind,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) state: AiEntityState,
    pub(crate) is_complete: bool,
    pub(crate) production_queue_len: Option<usize>,
    pub(crate) production_kind: Option<EntityKind>,
    pub(crate) latched_node: Option<u32>,
    pub(crate) target_id: Option<u32>,
    pub(crate) free_for_combat: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AiResourceSummary {
    pub(crate) id: u32,
    pub(crate) kind: EntityKind,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) remaining: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AiObservation {
    pub(crate) player_id: u32,
    pub(crate) tick: u32,
    pub(crate) map: AiMapSummary,
    pub(crate) economy: AiEconomy,
    pub(crate) own_start_tile: (u32, u32),
    pub(crate) players: Vec<AiPlayerSummary>,
    pub(crate) owned: Vec<AiEntitySummary>,
    pub(crate) resources: Vec<AiResourceSummary>,
    pub(crate) visible_enemies: Vec<AiEntitySummary>,
    pub(crate) pending_builds: Vec<AiBuildIntent>,
}

impl AiObservation {
    pub(crate) fn from_live_state(
        map: &Map,
        entities: &EntityStore,
        fog: &Fog,
        players: &[PlayerState],
        player_id: u32,
        tick: u32,
    ) -> Option<Self> {
        let me = players.iter().find(|p| p.id == player_id)?;
        let map_summary = AiMapSummary {
            width: map.size,
            height: map.size,
            tile_size: crate::config::TILE_SIZE,
        };
        let economy = AiEconomy {
            steel: me.steel,
            oil: me.oil,
            supply_used: me.supply_used,
            supply_cap: me.supply_cap,
        };

        let mut player_summaries: Vec<AiPlayerSummary> = players
            .iter()
            .map(|p| AiPlayerSummary {
                id: p.id,
                start_tile: p.start_tile,
                is_ai: p.is_ai,
                is_alive: entities.player_alive(p.id),
            })
            .collect();
        player_summaries.sort_by_key(|p| p.id);

        let mut owned: Vec<AiEntitySummary> = entities
            .iter()
            .filter(|e| e.owner == player_id && (e.is_unit() || e.is_building()))
            .map(AiEntitySummary::from_live_entity)
            .collect();
        owned.sort_by_key(|e| e.id);

        let mut resources: Vec<AiResourceSummary> = entities
            .iter()
            .filter(|e| e.owner == NEUTRAL && e.is_node())
            .filter_map(AiResourceSummary::from_live_entity)
            .collect();
        resources.sort_by_key(|r| r.id);

        let mut pending_builds: Vec<AiBuildIntent> = entities
            .iter()
            .filter(|e| e.owner == player_id && e.kind == EntityKind::Worker)
            .filter_map(pending_build_intent_from_live_worker)
            .collect();
        pending_builds.sort_unstable();
        pending_builds.dedup();

        let mut visible_enemies: Vec<AiEntitySummary> = entities
            .iter()
            .filter(|e| e.owner != NEUTRAL && e.owner != player_id)
            .filter(|e| e.is_unit() || e.is_building())
            .filter(|e| fog.is_visible_world(player_id, e.pos_x, e.pos_y))
            .map(AiEntitySummary::from_live_entity)
            .collect();
        visible_enemies.sort_by_key(|e| e.id);

        Some(Self {
            player_id,
            tick,
            map: map_summary,
            economy,
            own_start_tile: me.start_tile,
            players: player_summaries,
            owned,
            resources,
            // Live AI only receives entities visible through its authoritative fog grid. It can
            // react to scouted pressure without learning hidden enemy positions.
            visible_enemies,
            pending_builds,
        })
    }

    pub(crate) fn from_selfplay_snapshot(
        start: &StartPayload,
        snapshot: &Snapshot,
        player_id: u32,
        pending_builds: impl IntoIterator<Item = AiBuildIntent>,
    ) -> Option<Self> {
        let own_start_tile = start
            .players
            .iter()
            .find(|p| p.id == player_id)
            .map(|p| (p.start_tile_x, p.start_tile_y))?;
        let map = AiMapSummary {
            width: start.map.width,
            height: start.map.height,
            tile_size: start.map.tile_size,
        };
        let economy = AiEconomy {
            steel: snapshot.steel,
            oil: snapshot.oil,
            supply_used: snapshot.supply_used,
            supply_cap: snapshot.supply_cap,
        };

        let mut players: Vec<AiPlayerSummary> = start
            .players
            .iter()
            .map(|p| AiPlayerSummary {
                id: p.id,
                start_tile: (p.start_tile_x, p.start_tile_y),
                is_ai: false,
                is_alive: true,
            })
            .collect();
        players.sort_by_key(|p| p.id);

        let mut owned: Vec<AiEntitySummary> = snapshot
            .entities
            .iter()
            .filter(|e| e.owner == player_id)
            .filter_map(AiEntitySummary::from_entity_view)
            .filter(|e| e.kind.is_unit() || e.kind.is_building())
            .collect();
        owned.sort_by_key(|e| e.id);

        let mut resources_by_id: BTreeMap<u32, AiResourceSummary> = start
            .map
            .resources
            .iter()
            .filter_map(|resource| {
                let kind: EntityKind = resource.kind.parse().ok()?;
                kind.is_node().then_some((
                    resource.id,
                    AiResourceSummary {
                        id: resource.id,
                        kind,
                        x: resource.x,
                        y: resource.y,
                        // Static start-payload resources are known positions, not known current
                        // state. Treat them as available until a visible delta says otherwise.
                        remaining: 1,
                    },
                ))
            })
            .collect();
        for delta in &snapshot.resource_deltas {
            if let Some(resource) = resources_by_id.get_mut(&delta.id) {
                resource.remaining = delta.remaining;
            }
        }
        for resource in snapshot
            .entities
            .iter()
            .filter(|e| e.owner == NEUTRAL)
            .filter_map(AiResourceSummary::from_entity_view)
        {
            resources_by_id.insert(resource.id, resource);
        }
        let mut resources: Vec<AiResourceSummary> = resources_by_id.into_values().collect();
        resources.sort_by_key(|r| r.id);

        let mut visible_enemies: Vec<AiEntitySummary> = snapshot
            .entities
            .iter()
            .filter(|e| e.owner != NEUTRAL && e.owner != player_id)
            .filter(|e| !e.vision_only)
            .filter_map(AiEntitySummary::from_entity_view)
            .filter(|e| e.kind.is_unit() || e.kind.is_building())
            .collect();
        visible_enemies.sort_by_key(|e| e.id);

        let mut pending_builds: Vec<AiBuildIntent> = pending_builds.into_iter().collect();
        pending_builds.sort_unstable();
        pending_builds.dedup();

        Some(Self {
            player_id,
            tick: snapshot.tick,
            map,
            economy,
            own_start_tile,
            players,
            owned,
            resources,
            visible_enemies,
            pending_builds,
        })
    }
}

impl AiEntitySummary {
    fn from_live_entity(entity: &Entity) -> Self {
        Self {
            id: entity.id,
            owner: entity.owner,
            kind: entity.kind,
            x: entity.pos_x,
            y: entity.pos_y,
            state: AiEntityState::from_protocol_state(entity.state_str()),
            is_complete: !entity.under_construction(),
            production_queue_len: production_queue_len(entity.kind, entity.prod_queue().len()),
            production_kind: entity.prod_queue().first().map(|item| item.unit),
            latched_node: live_latched_node(entity),
            target_id: entity.target_id(),
            free_for_combat: live_free_for_combat(entity),
        }
    }

    fn from_entity_view(view: &EntityView) -> Option<Self> {
        let kind: EntityKind = view.kind.parse().ok()?;
        let state = AiEntityState::from_protocol_state(&view.state);
        Some(Self {
            id: view.id,
            owner: view.owner,
            kind,
            x: view.x,
            y: view.y,
            state,
            is_complete: view.build_progress.is_none(),
            production_queue_len: production_queue_len(kind, view.prod_queue.unwrap_or(0) as usize),
            production_kind: view
                .prod_kind
                .as_deref()
                .and_then(|kind| kind.parse::<EntityKind>().ok()),
            latched_node: view.latched_node,
            target_id: view.target_id,
            free_for_combat: snapshot_free_for_combat(state, view.target_id),
        })
    }
}

impl AiResourceSummary {
    fn from_live_entity(entity: &Entity) -> Option<Self> {
        entity.kind.is_node().then_some(Self {
            id: entity.id,
            kind: entity.kind,
            x: entity.pos_x,
            y: entity.pos_y,
            remaining: entity.remaining().unwrap_or(0),
        })
    }

    fn from_entity_view(view: &EntityView) -> Option<Self> {
        let kind: EntityKind = view.kind.parse().ok()?;
        kind.is_node().then_some(Self {
            id: view.id,
            kind,
            x: view.x,
            y: view.y,
            remaining: view.remaining.unwrap_or(0),
        })
    }
}

fn production_queue_len(kind: EntityKind, queue_len: usize) -> Option<usize> {
    (!rules::economy::trainable_units(kind).is_empty()).then_some(queue_len)
}

fn live_latched_node(entity: &Entity) -> Option<u32> {
    (entity.kind == EntityKind::Worker)
        .then(|| entity.order().gather_node())
        .flatten()
}

fn live_free_for_combat(entity: &Entity) -> bool {
    match entity.order() {
        Order::Idle => true,
        Order::AttackMove(_) => entity.path_is_empty() && entity.target_id().is_none(),
        _ => false,
    }
}

fn snapshot_free_for_combat(state: AiEntityState, target_id: Option<u32>) -> bool {
    target_id.is_none()
        && matches!(
            state,
            AiEntityState::Idle | AiEntityState::Move | AiEntityState::Attack
        )
}

fn pending_build_intent_from_live_worker(worker: &Entity) -> Option<AiBuildIntent> {
    if worker.build_phase() != Some(BuildPhase::ToSite) {
        return None;
    }
    let (kind, tile_x, tile_y) = worker.order().build_intent_tile()?;
    crate::config::building_stats(kind)?;
    Some(AiBuildIntent::to_site(worker.id, kind, tile_x, tile_y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{terrain, EntityView, MapInfo, PlayerStart};

    fn empty_snapshot(tick: u32) -> Snapshot {
        Snapshot {
            tick,
            steel: 100,
            oil: 25,
            supply_used: 3,
            supply_cap: 10,
            entities: Vec::new(),
            resource_deltas: Vec::new(),
            smokes: Vec::new(),
            events: Vec::new(),
            player_resources: Vec::new(),
            net_status: crate::protocol::SnapshotNetStatus::default(),
        }
    }

    fn start_payload() -> StartPayload {
        StartPayload {
            player_id: 1,
            spectator: false,
            tick: 0,
            map: MapInfo {
                width: 64,
                height: 64,
                tile_size: crate::config::TILE_SIZE,
                terrain: vec![terrain::GRASS; 64 * 64],
                resources: Vec::new(),
            },
            players: vec![
                PlayerStart {
                    id: 2,
                    name: "Bravo".into(),
                    color: "#222".into(),
                    start_tile_x: 48,
                    start_tile_y: 48,
                },
                PlayerStart {
                    id: 1,
                    name: "Alpha".into(),
                    color: "#111".into(),
                    start_tile_x: 8,
                    start_tile_y: 8,
                },
            ],
        }
    }

    #[test]
    fn selfplay_observation_sorts_visible_inputs() {
        let mut snapshot = empty_snapshot(42);
        snapshot.entities = vec![
            EntityView::new(
                30,
                0,
                crate::protocol::kind_to_wire(EntityKind::Steel),
                64.0,
                64.0,
                1,
                1,
                states::IDLE,
            ),
            EntityView::new(
                20,
                2,
                crate::protocol::kind_to_wire(EntityKind::Rifleman),
                96.0,
                96.0,
                1,
                1,
                states::IDLE,
            ),
            EntityView::new(
                10,
                1,
                crate::protocol::kind_to_wire(EntityKind::Worker),
                32.0,
                32.0,
                1,
                1,
                states::IDLE,
            ),
        ];
        let start = start_payload();

        let observation = AiObservation::from_selfplay_snapshot(
            &start,
            &snapshot,
            1,
            [AiBuildIntent::to_site(10, EntityKind::Depot, 9, 10)],
        )
        .unwrap();

        assert_eq!(observation.player_id, 1);
        assert_eq!(observation.tick, 42);
        assert_eq!(observation.own_start_tile, (8, 8));
        assert_eq!(
            observation.players.iter().map(|p| p.id).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            observation.owned.iter().map(|e| e.id).collect::<Vec<_>>(),
            vec![10]
        );
        assert_eq!(
            observation
                .visible_enemies
                .iter()
                .map(|e| e.id)
                .collect::<Vec<_>>(),
            vec![20]
        );
        assert_eq!(
            observation
                .resources
                .iter()
                .map(|r| r.id)
                .collect::<Vec<_>>(),
            vec![30]
        );
        assert_eq!(observation.pending_builds.len(), 1);
    }

    #[test]
    fn selfplay_moving_combat_units_can_rejoin_profile_waves() {
        let staged = EntityView::new(
            10,
            1,
            crate::protocol::kind_to_wire(EntityKind::Rifleman),
            32.0,
            32.0,
            1,
            1,
            states::MOVE,
        );
        let attack_moving = EntityView::new(
            12,
            1,
            crate::protocol::kind_to_wire(EntityKind::Tank),
            96.0,
            32.0,
            1,
            1,
            states::ATTACK,
        );
        let mut engaged = EntityView::new(
            11,
            1,
            crate::protocol::kind_to_wire(EntityKind::Rifleman),
            64.0,
            32.0,
            1,
            1,
            states::MOVE,
        );
        engaged.target_id = Some(20);
        let mut snapshot = empty_snapshot(42);
        snapshot.entities = vec![staged, engaged, attack_moving];
        let start = start_payload();

        let observation = AiObservation::from_selfplay_snapshot(&start, &snapshot, 1, []).unwrap();

        let staged = observation.owned.iter().find(|e| e.id == 10).unwrap();
        let attack_moving = observation.owned.iter().find(|e| e.id == 12).unwrap();
        let engaged = observation.owned.iter().find(|e| e.id == 11).unwrap();
        assert!(staged.free_for_combat);
        assert!(attack_moving.free_for_combat);
        assert!(!engaged.free_for_combat);
    }

    #[test]
    fn live_observation_uses_public_enemy_starts_without_enemy_units() {
        let map = Map::generate(2, 1234);
        let mut entities = EntityStore::new();
        entities
            .spawn_unit(1, EntityKind::Worker, 32.0, 32.0)
            .unwrap();
        entities
            .spawn_unit(2, EntityKind::Rifleman, 256.0, 256.0)
            .unwrap();
        let players = vec![
            PlayerState {
                id: 1,
                name: "Alpha".into(),
                color: "#111".into(),
                start_tile: (8, 8),
                steel: 100,
                oil: 0,
                supply_used: 1,
                supply_cap: 10,
                is_ai: true,
                score: crate::game::ScoreState::default(),
            },
            PlayerState {
                id: 2,
                name: "Bravo".into(),
                color: "#222".into(),
                start_tile: (48, 48),
                steel: 100,
                oil: 0,
                supply_used: 1,
                supply_cap: 10,
                is_ai: false,
                score: crate::game::ScoreState::default(),
            },
        ];
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2], &entities, &map);

        let observation =
            AiObservation::from_live_state(&map, &entities, &fog, &players, 1, 9).unwrap();

        assert_eq!(
            observation.players.iter().map(|p| p.id).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(observation.visible_enemies.is_empty());
        assert_eq!(
            observation.owned.iter().map(|e| e.id).collect::<Vec<_>>(),
            vec![1]
        );
    }

    #[test]
    fn live_observation_includes_only_fog_visible_enemies() {
        let map = Map::generate(2, 1234);
        let mut entities = EntityStore::new();
        entities
            .spawn_unit(1, EntityKind::Worker, 32.0, 32.0)
            .unwrap();
        let visible_enemy = entities
            .spawn_unit(2, EntityKind::Rifleman, 64.0, 32.0)
            .unwrap();
        entities
            .spawn_unit(2, EntityKind::Rifleman, 1_024.0, 1_024.0)
            .unwrap();
        let players = vec![
            PlayerState {
                id: 1,
                name: "Alpha".into(),
                color: "#111".into(),
                start_tile: (8, 8),
                steel: 100,
                oil: 0,
                supply_used: 1,
                supply_cap: 10,
                is_ai: true,
                score: crate::game::ScoreState::default(),
            },
            PlayerState {
                id: 2,
                name: "Bravo".into(),
                color: "#222".into(),
                start_tile: (48, 48),
                steel: 100,
                oil: 0,
                supply_used: 1,
                supply_cap: 10,
                is_ai: false,
                score: crate::game::ScoreState::default(),
            },
        ];
        let mut fog = Fog::new(map.size);
        fog.recompute(&[1, 2], &entities, &map);

        let observation =
            AiObservation::from_live_state(&map, &entities, &fog, &players, 1, 9).unwrap();

        assert_eq!(
            observation
                .visible_enemies
                .iter()
                .map(|enemy| enemy.id)
                .collect::<Vec<_>>(),
            vec![visible_enemy]
        );
    }
}
