use std::collections::{BTreeMap, BTreeSet};

use rts_rules::faction::UpgradeKind;
use rts_rules::terrain::*;
use rts_rules::EntityKind;
use rts_sim::game::entity::NEUTRAL;
use rts_sim::game::upgrade;
use rts_sim::protocol::{states, EntityView, Snapshot, StartPayload};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AiEconomy {
    pub steel: u32,
    pub oil: u32,
    pub supply_used: u32,
    pub supply_cap: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiTerrain {
    Grass,
    Rock,
    Water,
    RoadBare,
    RoadHorizontal,
    RoadVertical,
    RoadDiagonalNwSe,
    RoadDiagonalNeSw,
    GravelA,
    GravelB,
    GravelC,
    DirtA,
    DirtB,
    DirtC,
    MudA,
    MudB,
    MudC,
    FrostedGround,
    Unknown(u8),
}

impl AiTerrain {
    fn from_code(code: u8) -> Self {
        match code {
            MAP_TERRAIN_GRASS => Self::Grass,
            MAP_TERRAIN_ROCK => Self::Rock,
            MAP_TERRAIN_WATER => Self::Water,
            MAP_TERRAIN_ROAD_BARE => Self::RoadBare,
            MAP_TERRAIN_ROAD_HORIZONTAL => Self::RoadHorizontal,
            MAP_TERRAIN_ROAD_VERTICAL => Self::RoadVertical,
            MAP_TERRAIN_ROAD_DIAGONAL_NW_SE => Self::RoadDiagonalNwSe,
            MAP_TERRAIN_ROAD_DIAGONAL_NE_SW => Self::RoadDiagonalNeSw,
            MAP_TERRAIN_GRAVEL_A => Self::GravelA,
            MAP_TERRAIN_GRAVEL_B => Self::GravelB,
            MAP_TERRAIN_GRAVEL_C => Self::GravelC,
            MAP_TERRAIN_DIRT_A => Self::DirtA,
            MAP_TERRAIN_DIRT_B => Self::DirtB,
            MAP_TERRAIN_DIRT_C => Self::DirtC,
            MAP_TERRAIN_MUD_A => Self::MudA,
            MAP_TERRAIN_MUD_B => Self::MudB,
            MAP_TERRAIN_MUD_C => Self::MudC,
            MAP_TERRAIN_FROSTED_GROUND => Self::FrostedGround,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiMap {
    pub width: u32,
    pub height: u32,
    pub tile_size: u32,
    pub terrain: Vec<AiTerrain>,
    pub no_building_tiles: BTreeSet<(u32, u32)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiPlayer {
    pub id: u32,
    pub team_id: u32,
    pub faction_id: String,
    pub is_ai: bool,
    pub is_alive: bool,
    pub start_tile: (u32, u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AiEntityState {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AiHealth {
    pub current: u32,
    pub maximum: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AiCompletion {
    Complete,
    UnderConstruction {
        progress: f32,
        /// Known only for entities owned by the frame's player.
        active: Option<bool>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AiProduction {
    /// Owner/allied queue detail; `None` for opponents.
    pub queue_len: Option<usize>,
    pub current_kind: Option<EntityKind>,
    pub current_upgrade: Option<UpgradeKind>,
    pub progress: Option<f32>,
    /// Owner/allied payment state; `None` for opponents.
    pub waiting_for_resources: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AiEntity {
    pub id: u32,
    pub owner: u32,
    pub kind: EntityKind,
    pub position: (f32, f32),
    pub health: AiHealth,
    pub state: AiEntityState,
    pub completion: AiCompletion,
    pub production: Option<AiProduction>,
    pub latched_resource: Option<u32>,
    pub target: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiResourceAmount {
    Unknown,
    Known(u32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AiResource {
    pub id: u32,
    pub kind: EntityKind,
    pub position: (f32, f32),
    pub remaining: AiResourceAmount,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AiRememberedContact {
    pub id: u32,
    pub owner: u32,
    pub kind: EntityKind,
    pub position: (f32, f32),
    pub observed_tick: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AiBuildObservationPhase {
    TravelingToSite,
}

/// Controller-inferred submitted-build bookkeeping. This is observation, not confirmation that
/// the simulation accepted the build request or that the site is legal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AiBuildObservation {
    pub worker_id: u32,
    pub kind: EntityKind,
    pub tile_x: u32,
    pub tile_y: u32,
    pub phase: AiBuildObservationPhase,
}

/// An owned, deterministic, player-scoped view delivered to a strategy decision.
///
/// Collections are sorted by stable ids (and build observations by their full tuple). Current
/// contacts and remembered contacts are separate; a remembered contact must never be interpreted
/// as presently visible. Static resource positions are public, but their amount remains `Unknown`
/// until a recipient-scoped snapshot reveals it.
#[derive(Clone, Debug, PartialEq)]
pub struct AiFrame {
    player_id: u32,
    faction_id: String,
    tick: u32,
    team_id: u32,
    economy: AiEconomy,
    map: AiMap,
    players: Vec<AiPlayer>,
    owned: Vec<AiEntity>,
    visible_allies: Vec<AiEntity>,
    visible_enemies: Vec<AiEntity>,
    remembered_contacts: Vec<AiRememberedContact>,
    resources: Vec<AiResource>,
    submitted_builds: Vec<AiBuildObservation>,
    completed_upgrades: Vec<UpgradeKind>,
}

impl AiFrame {
    pub fn player_id(&self) -> u32 {
        self.player_id
    }
    pub fn faction_id(&self) -> &str {
        &self.faction_id
    }
    pub fn tick(&self) -> u32 {
        self.tick
    }
    pub fn team_id(&self) -> u32 {
        self.team_id
    }
    pub fn economy(&self) -> AiEconomy {
        self.economy
    }
    pub fn map(&self) -> &AiMap {
        &self.map
    }
    pub fn players(&self) -> &[AiPlayer] {
        &self.players
    }
    pub fn owned(&self) -> &[AiEntity] {
        &self.owned
    }
    pub fn visible_allies(&self) -> &[AiEntity] {
        &self.visible_allies
    }
    pub fn visible_enemies(&self) -> &[AiEntity] {
        &self.visible_enemies
    }
    pub fn remembered_contacts(&self) -> &[AiRememberedContact] {
        &self.remembered_contacts
    }
    pub fn resources(&self) -> &[AiResource] {
        &self.resources
    }
    pub fn submitted_builds(&self) -> &[AiBuildObservation] {
        &self.submitted_builds
    }
    pub fn completed_upgrades(&self) -> &[UpgradeKind] {
        &self.completed_upgrades
    }

    pub(crate) fn from_host(
        start: &StartPayload,
        snapshot: &Snapshot,
        player_id: u32,
        submitted_builds: impl IntoIterator<Item = AiBuildObservation>,
        alive_player_ids: Option<&[u32]>,
    ) -> Option<Self> {
        let own = start.players.iter().find(|player| player.id == player_id)?;
        let team_id = own.team_id;
        let mut players = start
            .players
            .iter()
            .map(|player| AiPlayer {
                id: player.id,
                team_id: player.team_id,
                faction_id: player.faction_id.clone(),
                is_ai: player.is_ai,
                is_alive: alive_player_ids
                    .map(|ids| ids.contains(&player.id))
                    .unwrap_or(true),
                start_tile: (player.start_tile_x, player.start_tile_y),
            })
            .collect::<Vec<_>>();
        players.sort_by_key(|player| player.id);

        let mut owned = snapshot
            .entities
            .iter()
            .filter(|entity| entity.owner == player_id)
            .filter_map(|entity| normalize_entity(entity, true, true))
            .filter(gameplay_entity)
            .collect::<Vec<_>>();
        owned.sort_by_key(|entity| entity.id);
        let mut visible_allies = snapshot
            .entities
            .iter()
            .filter(|entity| entity.owner != NEUTRAL && entity.owner != player_id)
            .filter(|entity| !entity.vision_only)
            .filter(|entity| is_ally(&players, team_id, entity.owner))
            .filter_map(|entity| normalize_entity(entity, false, true))
            .filter(gameplay_entity)
            .collect::<Vec<_>>();
        visible_allies.sort_by_key(|entity| entity.id);
        let mut visible_enemies = snapshot
            .entities
            .iter()
            .filter(|entity| entity.owner != NEUTRAL && entity.owner != player_id)
            .filter(|entity| !entity.vision_only)
            .filter(|entity| is_enemy(&players, player_id, team_id, entity.owner))
            .filter_map(|entity| normalize_entity(entity, false, false))
            .filter(gameplay_entity)
            .collect::<Vec<_>>();
        visible_enemies.sort_by_key(|entity| entity.id);

        let mut resources = start
            .map
            .resources
            .iter()
            .filter_map(|resource| {
                let kind = resource.kind.parse::<EntityKind>().ok()?;
                kind.is_node().then_some((
                    resource.id,
                    AiResource {
                        id: resource.id,
                        kind,
                        position: (resource.x, resource.y),
                        remaining: AiResourceAmount::Unknown,
                    },
                ))
            })
            .collect::<BTreeMap<_, _>>();
        for delta in &snapshot.resource_deltas {
            if let Some(resource) = resources.get_mut(&delta.id) {
                resource.remaining = AiResourceAmount::Known(delta.remaining);
            }
        }
        for entity in snapshot
            .entities
            .iter()
            .filter(|entity| entity.owner == NEUTRAL)
        {
            let Some(kind) = entity
                .kind
                .parse::<EntityKind>()
                .ok()
                .filter(|kind| kind.is_node())
            else {
                continue;
            };
            resources.insert(
                entity.id,
                AiResource {
                    id: entity.id,
                    kind,
                    position: (entity.x, entity.y),
                    remaining: AiResourceAmount::Known(entity.remaining.unwrap_or(0)),
                },
            );
        }

        let mut remembered_contacts =
            snapshot
                .remembered_buildings
                .iter()
                .filter_map(|contact| {
                    Some(AiRememberedContact {
                        id: contact.id,
                        owner: contact.owner,
                        kind: contact.kind.parse().ok()?,
                        position: (contact.x, contact.y),
                        observed_tick: contact.observed_tick,
                    })
                })
                .chain(snapshot.remembered_anti_tank_guns.iter().map(|contact| {
                    AiRememberedContact {
                        id: contact.id,
                        owner: contact.owner,
                        kind: EntityKind::AntiTankGun,
                        position: (contact.x, contact.y),
                        observed_tick: contact.observed_tick,
                    }
                }))
                .collect::<Vec<_>>();
        let current_contact_ids = owned
            .iter()
            .chain(&visible_allies)
            .chain(&visible_enemies)
            .map(|entity| entity.id)
            .collect::<BTreeSet<_>>();
        remembered_contacts.retain(|contact| !current_contact_ids.contains(&contact.id));
        remembered_contacts.sort_by_key(|contact| (contact.id, contact.observed_tick));
        remembered_contacts.dedup_by_key(|contact| contact.id);

        let mut submitted_builds = submitted_builds.into_iter().collect::<Vec<_>>();
        submitted_builds.sort_unstable();
        submitted_builds.dedup();
        let mut completed_upgrades = snapshot
            .upgrades
            .iter()
            .filter_map(|id| id.parse::<UpgradeKind>().ok())
            .collect::<Vec<_>>();
        completed_upgrades.sort_unstable();
        completed_upgrades.dedup();

        Some(Self {
            player_id,
            faction_id: own.faction_id.clone(),
            tick: snapshot.tick,
            team_id,
            economy: AiEconomy {
                steel: snapshot.steel,
                oil: snapshot.oil,
                supply_used: snapshot.supply_used,
                supply_cap: snapshot.supply_cap,
            },
            map: AiMap {
                width: start.map.width,
                height: start.map.height,
                tile_size: start.map.tile_size,
                terrain: start
                    .map
                    .terrain
                    .iter()
                    .copied()
                    .map(AiTerrain::from_code)
                    .collect(),
                no_building_tiles: start
                    .map
                    .no_building_tiles
                    .iter()
                    .map(|tile| (tile.x, tile.y))
                    .collect(),
            },
            players,
            owned,
            visible_allies,
            visible_enemies,
            remembered_contacts,
            resources: resources.into_values().collect(),
            submitted_builds,
            completed_upgrades,
        })
    }
}

fn normalize_entity(
    view: &EntityView,
    completion_activity_known: bool,
    production_details_known: bool,
) -> Option<AiEntity> {
    let kind = view.kind.parse::<EntityKind>().ok()?;
    let state = match view.state.as_str() {
        states::IDLE => AiEntityState::Idle,
        states::MOVE => AiEntityState::Move,
        states::ATTACK => AiEntityState::Attack,
        states::GATHER => AiEntityState::Gather,
        states::BUILD => AiEntityState::Build,
        states::TRAIN => AiEntityState::Train,
        states::CONSTRUCT => AiEntityState::Construct,
        states::DEAD => AiEntityState::Dead,
        _ => AiEntityState::Unknown,
    };
    let can_produce = !rts_rules::economy::trainable_units(kind).is_empty()
        || !upgrade::researchable_upgrades(kind).is_empty();
    Some(AiEntity {
        id: view.id,
        owner: view.owner,
        kind,
        position: (view.x, view.y),
        health: AiHealth {
            current: view.hp,
            maximum: view.max_hp,
        },
        state,
        completion: view
            .build_progress
            .map_or(AiCompletion::Complete, |progress| {
                AiCompletion::UnderConstruction {
                    progress,
                    active: completion_activity_known.then_some(view.build_active),
                }
            }),
        production: can_produce.then_some(AiProduction {
            queue_len: production_details_known.then_some(view.prod_queue.unwrap_or(0) as usize),
            current_kind: view.prod_kind.as_deref().and_then(|id| id.parse().ok()),
            current_upgrade: view.prod_upgrade.as_deref().and_then(|id| id.parse().ok()),
            progress: view.prod_progress,
            waiting_for_resources: production_details_known.then_some(view.prod_waiting),
        }),
        latched_resource: view.latched_node,
        target: view.target_id,
    })
}

fn gameplay_entity(entity: &AiEntity) -> bool {
    entity.kind.is_unit() || entity.kind.is_building()
}

fn is_enemy(players: &[AiPlayer], player_id: u32, own_team: u32, owner: u32) -> bool {
    owner != player_id
        && players
            .iter()
            .find(|player| player.id == owner)
            .map(|player| player.team_id != own_team || own_team == 0)
            .unwrap_or(false)
}

fn is_ally(players: &[AiPlayer], own_team: u32, owner: u32) -> bool {
    own_team != 0
        && players
            .iter()
            .find(|player| player.id == owner)
            .map(|player| player.team_id == own_team)
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_sim::game::{Game, PlayerInit};

    fn players() -> Vec<PlayerInit> {
        vec![
            PlayerInit {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".to_string(),
                name: "One".to_string(),
                color: "#111111".to_string(),
                is_ai: true,
            },
            PlayerInit {
                id: 2,
                team_id: 2,
                faction_id: "kriegsia".to_string(),
                name: "Two".to_string(),
                color: "#222222".to_string(),
                is_ai: true,
            },
        ]
    }

    #[test]
    fn real_game_frame_excludes_hidden_entities_and_keeps_unknown_resource_amounts() {
        let game = Game::new_without_ai_controllers(&players(), 41);
        let mut start = game.start_payload();
        start.map.resources.push(rts_sim::protocol::ResourceNode {
            id: 999_999,
            kind: rts_sim::protocol::kinds::OIL.to_string(),
            x: start.map.tile_size as f32 * 0.5,
            y: start.map.tile_size as f32 * 0.5,
        });
        let fogged = game.snapshot_for(1);
        let full = game.snapshot_full_for(1);
        let hidden = full
            .entities
            .iter()
            .find(|entity| {
                entity.owner == 2 && !fogged.entities.iter().any(|seen| seen.id == entity.id)
            })
            .expect("starting enemy entity is hidden from player one")
            .clone();

        let frame = AiFrame::from_host(&start, &fogged, 1, [], Some(&[1, 2])).unwrap();
        assert!(frame.visible_enemies().is_empty());
        assert_eq!(
            frame
                .resources()
                .iter()
                .find(|resource| resource.id == 999_999)
                .map(|resource| resource.remaining),
            Some(AiResourceAmount::Unknown)
        );

        let mut hidden_variant = fogged.clone();
        let mut memory_only = hidden.clone();
        memory_only.vision_only = true;
        hidden_variant.entities.push(memory_only);
        let variant = AiFrame::from_host(&start, &hidden_variant, 1, [], Some(&[1, 2])).unwrap();
        assert_eq!(frame, variant);

        let remembered = rts_sim::protocol::RememberedBuildingView {
            id: hidden.id,
            owner: hidden.owner,
            kind: hidden.kind.clone(),
            x: hidden.x,
            y: hidden.y,
            footprint: Vec::new(),
            observed_tick: fogged.tick,
        };
        let mut remembered_variant = fogged.clone();
        remembered_variant
            .remembered_buildings
            .push(remembered.clone());
        let remembered_frame =
            AiFrame::from_host(&start, &remembered_variant, 1, [], Some(&[1, 2])).unwrap();
        assert!(remembered_frame.visible_enemies().is_empty());
        assert_eq!(remembered_frame.remembered_contacts()[0].id, hidden.id);

        remembered_variant.entities.push(hidden);
        let visible_frame =
            AiFrame::from_host(&start, &remembered_variant, 1, [], Some(&[1, 2])).unwrap();
        assert!(visible_frame
            .visible_enemies()
            .iter()
            .any(|entity| entity.id == remembered.id));
        assert!(visible_frame.remembered_contacts().is_empty());
    }

    #[test]
    fn public_and_legacy_resource_knowledge_are_deliberately_distinct() {
        let game = Game::new_without_ai_controllers(&players(), 43);
        let start = game.start_payload();
        let snapshot = game.snapshot_for(1);
        let frame = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1, 2])).unwrap();
        let legacy = crate::ai_core::observation::AiObservation::from_frame(&frame).unwrap();

        assert!(frame.players().iter().all(|player| player.is_ai));
        assert!(legacy.players.iter().all(|player| !player.is_ai));
        for resource in frame.resources() {
            if resource.remaining == AiResourceAmount::Unknown {
                assert_eq!(
                    legacy
                        .resources
                        .iter()
                        .find(|legacy| legacy.id == resource.id)
                        .unwrap()
                        .remaining,
                    1
                );
            }
        }
    }

    #[test]
    fn redacted_entity_details_remain_unknown_without_changing_legacy_projection() {
        let game = Game::new_without_ai_controllers(&players(), 47);
        let start = game.start_payload();
        let mut snapshot = game.snapshot_for(1);

        let owned_producer = snapshot
            .entities
            .iter_mut()
            .find(|entity| entity.owner == 1 && entity.kind == "resource_depot")
            .expect("player one starts with a resource depot");
        let producer_kind = owned_producer.kind.parse::<EntityKind>().unwrap();
        owned_producer.build_progress = Some(0.5);
        owned_producer.build_active = false;
        owned_producer.prod_upgrade = Some(UpgradeKind::TankUnlock.to_protocol_str().to_string());

        let hidden_enemy = game
            .snapshot_full_for(1)
            .entities
            .into_iter()
            .find(|entity| entity.owner == 2 && entity.kind == "resource_depot")
            .expect("player two starts with a resource depot");
        assert!(!snapshot
            .entities
            .iter()
            .any(|entity| entity.id == hidden_enemy.id));
        snapshot.entities.push(hidden_enemy);

        let frame = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1, 2])).unwrap();
        let owned = frame
            .owned()
            .iter()
            .find(|entity| entity.kind == producer_kind)
            .unwrap();
        assert_eq!(
            owned.completion,
            AiCompletion::UnderConstruction {
                progress: 0.5,
                active: Some(false),
            }
        );
        assert_eq!(owned.production.unwrap().queue_len, Some(0));
        assert_eq!(
            owned.production.unwrap().current_upgrade,
            Some(UpgradeKind::TankUnlock)
        );
        assert_eq!(owned.production.unwrap().waiting_for_resources, Some(false));

        let enemy = frame
            .visible_enemies()
            .iter()
            .find(|entity| entity.kind == producer_kind)
            .unwrap();
        assert_eq!(enemy.production.unwrap().queue_len, None);
        assert_eq!(enemy.production.unwrap().waiting_for_resources, None);

        let projected = crate::ai_core::observation::AiObservation::from_frame(&frame);
        let direct = crate::ai_core::observation::AiObservation::from_snapshot_with_alive(
            &start,
            &snapshot,
            1,
            [],
            Some(&[1, 2]),
        );
        assert_eq!(projected, direct);
    }
}
