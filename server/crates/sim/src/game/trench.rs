use std::collections::{BTreeMap, BTreeSet};

use crate::config;
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::protocol::TrenchView;

pub(crate) const MAX_TRENCHES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Trench {
    pub(crate) id: u32,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) radius_tiles: f32,
}

impl Trench {
    fn radius_px(self) -> f32 {
        self.radius_tiles * config::TILE_SIZE as f32
    }

    fn contains_point(self, x: f32, y: f32) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let dx = x - self.x;
        let dy = y - self.y;
        let radius = self.radius_px();
        dx * dx + dy * dy <= radius * radius
    }

    fn to_view(self) -> TrenchView {
        TrenchView {
            id: self.id,
            x: self.x,
            y: self.y,
            radius_tiles: self.radius_tiles,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TrenchStore {
    next_id: u32,
    trenches: Vec<Trench>,
    discovered_by_player: BTreeMap<u32, BTreeSet<u32>>,
}

impl Default for TrenchStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TrenchStore {
    pub(crate) fn new() -> Self {
        Self {
            next_id: 1,
            trenches: Vec::new(),
            discovered_by_player: BTreeMap::new(),
        }
    }

    pub(crate) fn create(&mut self, map: &Map, x: f32, y: f32) -> Option<u32> {
        let radius_tiles = config::ENTRENCHMENT_TRENCH_RADIUS_TILES;
        if self.trenches.len() >= MAX_TRENCHES
            || !x.is_finite()
            || !y.is_finite()
            || !radius_tiles.is_finite()
            || radius_tiles <= 0.0
        {
            return None;
        }
        let world = map.world_size_px();
        if x < 0.0 || y < 0.0 || x >= world || y >= world {
            return None;
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.trenches.push(Trench {
            id,
            x,
            y,
            radius_tiles,
        });
        Some(id)
    }

    pub(crate) fn refresh_memory_for_player(&mut self, player: u32, fog: &Fog) {
        for trench in &self.trenches {
            if trench_visible_to_player(*trench, player, fog) {
                self.discovered_by_player
                    .entry(player)
                    .or_default()
                    .insert(trench.id);
            }
        }
    }

    pub(crate) fn views_for(
        &self,
        player: u32,
        fog: &Fog,
        fogged: bool,
        memory_players: &[u32],
    ) -> Vec<TrenchView> {
        let mut views = Vec::new();
        for trench in &self.trenches {
            if !fogged
                || trench_visible_to_player(*trench, player, fog)
                || self.trench_remembered_by_any(trench.id, memory_players)
            {
                views.push(trench.to_view());
            }
        }
        views.sort_by_key(|view| view.id);
        views
    }

    fn trench_remembered_by_any(&self, trench_id: u32, memory_players: &[u32]) -> bool {
        memory_players.iter().any(|player| {
            self.discovered_by_player
                .get(player)
                .is_some_and(|ids| ids.contains(&trench_id))
        })
    }
}

fn trench_visible_to_player(trench: Trench, player: u32, fog: &Fog) -> bool {
    if fog.is_visible_world(player, trench.x, trench.y) {
        return true;
    }
    let radius_tiles = trench.radius_tiles.ceil().max(0.0) as i32;
    let ts = config::TILE_SIZE as f32;
    let cx = (trench.x / ts).floor() as i32;
    let cy = (trench.y / ts).floor() as i32;
    for dy in -radius_tiles..=radius_tiles {
        for dx in -radius_tiles..=radius_tiles {
            let tx = cx + dx;
            let ty = cy + dy;
            if tx < 0 || ty < 0 {
                continue;
            }
            let center_x = (tx as f32 + 0.5) * ts;
            let center_y = (ty as f32 + 0.5) * ts;
            if trench.contains_point(center_x, center_y)
                && fog.is_visible(player, tx as u32, ty as u32)
            {
                return true;
            }
        }
    }
    false
}
