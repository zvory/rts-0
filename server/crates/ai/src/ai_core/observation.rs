use std::collections::BTreeMap;

use rts_rules;
use rts_sim::game::entity::{EntityKind, NEUTRAL};
use rts_sim::game::upgrade::{self, UpgradeKind};
use rts_sim::game::TeamId;
use rts_sim::protocol::{states, EntityView, Snapshot, StartPayload};

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
    pub(crate) team_id: TeamId,
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
    pub(crate) hp: u32,
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
    pub(crate) visible_allies: Vec<AiEntitySummary>,
    pub(crate) visible_enemies: Vec<AiEntitySummary>,
    pub(crate) pending_builds: Vec<AiBuildIntent>,
    pub(crate) upgrades: Vec<UpgradeKind>,
}

impl AiObservation {
    #[allow(dead_code)]
    pub(crate) fn from_selfplay_snapshot(
        start: &StartPayload,
        snapshot: &Snapshot,
        player_id: u32,
        pending_builds: impl IntoIterator<Item = AiBuildIntent>,
    ) -> Option<Self> {
        Self::from_snapshot_with_alive(start, snapshot, player_id, pending_builds, None)
    }

    pub(crate) fn from_snapshot_with_alive(
        start: &StartPayload,
        snapshot: &Snapshot,
        player_id: u32,
        pending_builds: impl IntoIterator<Item = AiBuildIntent>,
        alive_player_ids: Option<&[u32]>,
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
                team_id: p.team_id,
                start_tile: (p.start_tile_x, p.start_tile_y),
                is_ai: false,
                is_alive: alive_player_ids
                    .map(|ids| ids.contains(&p.id))
                    .unwrap_or(true),
            })
            .collect();
        players.sort_by_key(|p| p.id);
        let own_team_id = players
            .iter()
            .find(|player| player.id == player_id)
            .map(|player| player.team_id)?;

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
            .filter(|e| {
                e.owner != NEUTRAL
                    && e.owner != player_id
                    && entity_owner_is_enemy(&players, player_id, own_team_id, e.owner)
            })
            .filter(|e| !e.vision_only)
            .filter_map(AiEntitySummary::from_entity_view)
            .filter(|e| e.kind.is_unit() || e.kind.is_building())
            .collect();
        visible_enemies.sort_by_key(|e| e.id);

        let mut visible_allies: Vec<AiEntitySummary> = snapshot
            .entities
            .iter()
            .filter(|e| {
                e.owner != NEUTRAL
                    && e.owner != player_id
                    && entity_owner_is_ally(&players, own_team_id, e.owner)
            })
            .filter(|e| !e.vision_only)
            .filter_map(AiEntitySummary::from_entity_view)
            .filter(|e| e.kind.is_unit() || e.kind.is_building())
            .collect();
        visible_allies.sort_by_key(|e| e.id);

        let mut pending_builds: Vec<AiBuildIntent> = pending_builds.into_iter().collect();
        pending_builds.sort_unstable();
        pending_builds.dedup();
        let mut upgrades: Vec<UpgradeKind> = snapshot
            .upgrades
            .iter()
            .filter_map(|upgrade| upgrade.parse::<UpgradeKind>().ok())
            .collect();
        upgrades.sort_unstable();
        upgrades.dedup();

        Some(Self {
            player_id,
            tick: snapshot.tick,
            map,
            economy,
            own_start_tile,
            players,
            owned,
            resources,
            visible_allies,
            visible_enemies,
            pending_builds,
            upgrades,
        })
    }

    pub(crate) fn is_enemy_player(&self, player_id: u32) -> bool {
        let Some(own) = self
            .players
            .iter()
            .find(|player| player.id == self.player_id)
        else {
            return false;
        };
        player_id != self.player_id
            && self
                .players
                .iter()
                .find(|player| player.id == player_id)
                .map(|player| player.team_id != own.team_id || own.team_id == 0)
                .unwrap_or(false)
    }
}

fn entity_owner_is_enemy(
    players: &[AiPlayerSummary],
    player_id: u32,
    own_team_id: TeamId,
    owner: u32,
) -> bool {
    owner != player_id
        && players
            .iter()
            .find(|player| player.id == owner)
            .map(|player| player.team_id != own_team_id || own_team_id == 0)
            .unwrap_or(false)
}

fn entity_owner_is_ally(players: &[AiPlayerSummary], own_team_id: TeamId, owner: u32) -> bool {
    own_team_id != 0
        && players
            .iter()
            .find(|player| player.id == owner)
            .map(|player| player.team_id == own_team_id)
            .unwrap_or(false)
}

impl AiEntitySummary {
    fn from_entity_view(view: &EntityView) -> Option<Self> {
        let kind: EntityKind = view.kind.parse().ok()?;
        let state = AiEntityState::from_protocol_state(&view.state);
        Some(Self {
            id: view.id,
            owner: view.owner,
            kind,
            x: view.x,
            y: view.y,
            hp: view.hp,
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
    (!rts_rules::economy::trainable_units(kind).is_empty()
        || !upgrade::researchable_upgrades(kind).is_empty())
    .then_some(queue_len)
}

fn snapshot_free_for_combat(state: AiEntityState, target_id: Option<u32>) -> bool {
    target_id.is_none()
        && matches!(
            state,
            AiEntityState::Idle | AiEntityState::Move | AiEntityState::Attack
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_sim::protocol::{terrain, EntityView, MapInfo, PlayerStart};

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
            ability_objects: Vec::new(),
            trenches: Vec::new(),
            visible_tiles: Vec::new(),
            remembered_buildings: Vec::new(),
            events: Vec::new(),
            upgrades: Vec::new(),
            player_resources: Vec::new(),
            production_queue: Vec::new(),
            net_status: rts_sim::protocol::SnapshotNetStatus::default(),
        }
    }

    fn start_payload() -> StartPayload {
        StartPayload {
            player_id: 1,
            spectator: false,
            prediction_build_id: None,
            prediction_version: 0,
            match_run_id: None,
            capabilities: Default::default(),
            diagnostics: Default::default(),
            replay: None,
            lab: None,
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
                    team_id: 2,
                    faction_id: "kriegsia".to_string(),
                    name: "Bravo".into(),
                    color: "#222".into(),
                    start_tile_x: 48,
                    start_tile_y: 48,
                },
                PlayerStart {
                    id: 1,
                    team_id: 1,
                    faction_id: "kriegsia".to_string(),
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
                rts_sim::protocol::kind_to_wire(EntityKind::Steel),
                64.0,
                64.0,
                1,
                1,
                states::IDLE,
            ),
            EntityView::new(
                20,
                2,
                rts_sim::protocol::kind_to_wire(EntityKind::Rifleman),
                96.0,
                96.0,
                1,
                1,
                states::IDLE,
            ),
            EntityView::new(
                10,
                1,
                rts_sim::protocol::kind_to_wire(EntityKind::Worker),
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
    fn selfplay_observation_classifies_visible_allies_separately_from_enemies() {
        let mut start = start_payload();
        start.players.push(PlayerStart {
            id: 3,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Charlie".into(),
            color: "#333".into(),
            start_tile_x: 10,
            start_tile_y: 10,
        });
        let mut snapshot = empty_snapshot(7);
        snapshot.entities = vec![
            EntityView::new(
                20,
                2,
                rts_sim::protocol::kind_to_wire(EntityKind::Rifleman),
                96.0,
                96.0,
                1,
                1,
                states::IDLE,
            ),
            EntityView::new(
                30,
                3,
                rts_sim::protocol::kind_to_wire(EntityKind::Rifleman),
                64.0,
                64.0,
                1,
                1,
                states::IDLE,
            ),
        ];

        let observation = AiObservation::from_selfplay_snapshot(&start, &snapshot, 1, []).unwrap();

        assert_eq!(
            observation
                .visible_enemies
                .iter()
                .map(|entity| entity.id)
                .collect::<Vec<_>>(),
            vec![20]
        );
        assert_eq!(
            observation
                .visible_allies
                .iter()
                .map(|entity| entity.id)
                .collect::<Vec<_>>(),
            vec![30]
        );
    }

    #[test]
    fn research_buildings_are_available_to_production_facts() {
        let mut research_complex = EntityView::new(
            40,
            1,
            rts_sim::protocol::kind_to_wire(EntityKind::ResearchComplex),
            128.0,
            128.0,
            1,
            1,
            states::IDLE,
        );
        research_complex.prod_queue = Some(0);
        let mut snapshot = empty_snapshot(88);
        snapshot.entities = vec![research_complex];
        let start = start_payload();

        let observation = AiObservation::from_selfplay_snapshot(&start, &snapshot, 1, []).unwrap();
        let building = observation
            .owned
            .iter()
            .find(|entity| entity.id == 40)
            .unwrap();

        assert_eq!(building.kind, EntityKind::ResearchComplex);
        assert_eq!(building.production_queue_len, Some(0));
    }

    #[test]
    fn selfplay_moving_combat_units_can_rejoin_profile_waves() {
        let staged = EntityView::new(
            10,
            1,
            rts_sim::protocol::kind_to_wire(EntityKind::Rifleman),
            32.0,
            32.0,
            1,
            1,
            states::MOVE,
        );
        let attack_moving = EntityView::new(
            12,
            1,
            rts_sim::protocol::kind_to_wire(EntityKind::Tank),
            96.0,
            32.0,
            1,
            1,
            states::ATTACK,
        );
        let mut engaged = EntityView::new(
            11,
            1,
            rts_sim::protocol::kind_to_wire(EntityKind::Rifleman),
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
}
