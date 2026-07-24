use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::game::entity::{Entity, EntityKind, EntityStore, WeaponSetup};
use crate::game::fog::Fog;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::protocol::RememberedAntiTankGunView;
use crate::rules::projection;

use super::Game;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct AntiTankGunMemoryEntry {
    pub(crate) id: u32,
    pub(crate) owner: u32,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) facing: f32,
    pub(crate) observed_tick: u32,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub(crate) struct AntiTankGunMemory {
    entries: HashMap<(u32, u32), AntiTankGunMemoryEntry>,
}

impl AntiTankGunMemory {
    pub(crate) fn refresh(
        &mut self,
        player_ids: &[u32],
        entities: &EntityStore,
        fog: &Fog,
        smokes: &SmokeCloudStore,
        teams: &TeamRelations,
        tick: u32,
    ) {
        for &player_id in player_ids {
            self.refresh_player(player_id, entities, fog, smokes, teams, tick);
        }
    }

    fn refresh_player(
        &mut self,
        player_id: u32,
        entities: &EntityStore,
        fog: &Fog,
        smokes: &SmokeCloudStore,
        teams: &TeamRelations,
        tick: u32,
    ) {
        let team_players = teams.same_team_player_ids(player_id);
        let mut observed_ids = HashSet::new();
        for entity in entities.iter() {
            let visible = team_players.iter().copied().any(|team_player| {
                projection::entity_visible_to_with_smoke(team_player, entity, fog, smokes)
            });
            if !visible {
                continue;
            }
            observed_ids.insert(entity.id);
            let key = (player_id, entity.id);
            match memory_entry_for_visible_enemy(player_id, entity, teams, tick) {
                Some(entry) => {
                    self.entries.insert(key, entry);
                }
                None => {
                    self.entries.remove(&key);
                }
            }
        }

        self.entries.retain(|&(viewer, entity_id), memory| {
            if viewer != player_id || observed_ids.contains(&entity_id) {
                return true;
            }
            !team_players.iter().copied().any(|team_player| {
                fog.is_visible_without_firing_reveal_world(team_player, memory.x, memory.y)
            })
        });
    }

    pub(crate) fn entries_for_player(
        &self,
        player_id: u32,
    ) -> impl Iterator<Item = &AntiTankGunMemoryEntry> {
        self.entries
            .iter()
            .filter_map(move |(&(viewer, _), entry)| (viewer == player_id).then_some(entry))
    }

    pub(in crate::game) fn from_checkpoint_entries(
        entries: Vec<(u32, u32, AntiTankGunMemoryEntry)>,
    ) -> Self {
        Self {
            entries: entries
                .into_iter()
                .map(|(player_id, entity_id, entry)| ((player_id, entity_id), entry))
                .collect(),
        }
    }

    pub(in crate::game) fn checkpoint_entries(&self) -> Vec<(u32, u32, AntiTankGunMemoryEntry)> {
        let mut entries = self
            .entries
            .iter()
            .map(|(&(player_id, entity_id), entry)| (player_id, entity_id, entry.clone()))
            .collect::<Vec<_>>();
        entries.sort_by_key(|(player_id, entity_id, _)| (*player_id, *entity_id));
        entries
    }
}

impl Game {
    pub(in crate::game) fn remembered_anti_tank_gun_views_for(
        &self,
        player: u32,
        memory_players: &[u32],
        fog: &Fog,
        teams: &TeamRelations,
    ) -> Vec<RememberedAntiTankGunView> {
        let mut views: Vec<RememberedAntiTankGunView> = Vec::new();
        for &memory_player in memory_players {
            for entry in self
                .state
                .anti_tank_gun_memory
                .entries_for_player(memory_player)
            {
                if self.live_entity_projects(player, memory_players, entry.id, fog, teams) {
                    continue;
                }
                let view = RememberedAntiTankGunView {
                    id: entry.id,
                    owner: entry.owner,
                    x: entry.x,
                    y: entry.y,
                    facing: entry.facing,
                    observed_tick: entry.observed_tick,
                };
                match views.iter_mut().find(|existing| existing.id == view.id) {
                    Some(existing) if view.observed_tick > existing.observed_tick => {
                        *existing = view;
                    }
                    Some(_) => {}
                    None => views.push(view),
                }
            }
        }
        views.sort_by_key(|view| view.id);
        views
    }
}

fn memory_entry_for_visible_enemy(
    player_id: u32,
    entity: &Entity,
    teams: &TeamRelations,
    observed_tick: u32,
) -> Option<AntiTankGunMemoryEntry> {
    if entity.kind != EntityKind::AntiTankGun
        || entity.weapon_setup() != WeaponSetup::Deployed
        || !teams.is_enemy_owner(entity.owner, player_id)
    {
        return None;
    }
    let facing = entity
        .emplacement_facing()
        .or_else(|| entity.weapon_facing())?;
    facing.is_finite().then_some(AntiTankGunMemoryEntry {
        id: entity.id,
        owner: entity.owner,
        x: entity.pos_x,
        y: entity.pos_y,
        facing,
        observed_tick,
    })
}
