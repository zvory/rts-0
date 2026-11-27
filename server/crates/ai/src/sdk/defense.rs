//! Cached geometric cut/garrison hybrid for economy defense.
//!
//! This is a policy, not a guarantee of stopping an attack. Cuts separate six-tile base cores
//! and two-tile resource envelopes from a 32-tile exterior; garrisons leave approaches open.
//! Units share asset value by ground-route detours, then inverse-cube enemy approach urgency.
//! Unknown resource amounts remain unknown: valuation alone uses a discounted 312.5-Steel
//! potential estimate. The public diagnostics distinguish known stock and unknown node counts.
//! All coordinates in public results except `cut_tiles` are world pixels.
mod flow;
mod geometry;
mod grid;
mod scoring;
#[cfg(test)]
mod tests;
use super::{
    AiBaseSiteId, AiCompletion, AiFrame, AiMap, AiResourceAmount, AiTerrain, EntityKind,
    WorldQueries,
};
use grid::{dist2, Grid, FAR};
use rts_rules::math::GameplayMath as _;
use std::collections::{BTreeMap, BTreeSet};

/// Include all policy helpers in profile fingerprints without exposing private module layout.
pub const DEFENSE_PLANNER_SOURCE: &[&str] = &[
    include_str!("defense.rs"),
    include_str!("defense/grid.rs"),
    include_str!("defense/geometry.rs"),
    include_str!("defense/flow.rs"),
    include_str!("defense/scoring.rs"),
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefenseMode {
    Cut,
    Garrison,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DefenseSlot {
    pub position: (f32, f32),
    pub sector_id: usize,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DefenseSector {
    pub id: usize,
    pub mode: DefenseMode,
    pub base_ids: Vec<AiBaseSiteId>,
    pub cut_tiles: Vec<(u32, u32)>,
    pub center: (f32, f32),
    pub asset_value: f32,
    /// Normalized economic-urgency weight (including unsupported sectors).
    pub priority: f32,
    pub enemy_approach_seconds: Option<f32>,
    pub known_remaining_steel: f32,
    pub unknown_resource_count: f32,
    pub active_mine_share: f32,
    pub production_share: f32,
    pub tech_share: f32,
    pub supported: bool,
    pub slots: Vec<DefenseSlot>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DefensePlan {
    pub sectors: Vec<DefenseSector>,
    /// Priority-ordered slots, also present in each sector. Never fabricate slots to meet budget.
    pub slots: Vec<DefenseSlot>,
    pub unallocated_budget: usize,
}
#[derive(Clone)]
struct SectorGeometry {
    mode: DefenseMode,
    bases: Vec<AiBaseSiteId>,
    points: Vec<usize>,
    center: usize,
}
#[derive(Default)]
pub struct DefensePlanner {
    firing_range_tiles: Option<f32>,
    map: Option<AiMap>,
    player: u32,
    topology_identity: Vec<u64>,
    grid: Option<Grid>,
    fields: BTreeMap<AiBaseSiteId, Vec<u32>>,
    time_fields: BTreeMap<usize, Vec<u32>>,
    cuts: BTreeMap<Vec<AiBaseSiteId>, Option<geometry::Cut>>,
    geometry: Vec<SectorGeometry>,
    owned: Vec<AiBaseSiteId>,
    geometry_capacity: usize,
    enemy: Vec<u32>,
    last_plan: Option<DefensePlan>,
    last_tick: u32,
    last_budget: usize,
    last_assets: Vec<(u32, EntityKind, (i32, i32), bool)>,
    /// Number of max-flow computations; useful for profiling cache behavior.
    pub geometry_builds: u64,
}
impl DefensePlanner {
    pub fn new() -> Self {
        Self::default()
    }
    /// Restrict placements to a caller's conservative firing range (tiles), e.g. the shortest
    /// range in a mixed infantry group. Invalid values use the authoritative MG range minus0.2.
    pub fn with_firing_range_tiles(range: f32) -> Self {
        Self {
            firing_range_tiles: range
                .is_finite()
                .then_some(range)
                .filter(|r| *r > 0.0 && *r <= 64.0),
            ..Self::default()
        }
    }
    /// Site IDs are revalidated against currently owned, completed Depots. Unknown/duplicate IDs
    /// are ignored. Static topology/flows survive budget changes; geometry selection depends on
    /// budget, economic weights refresh every 90 simulation ticks, and known building changes
    /// invalidate slot legality immediately. Current units do not obstruct reserved slots.
    pub fn plan(
        &mut self,
        frame: &AiFrame,
        owned_completed_sites: &[AiBaseSiteId],
        unit_budget: usize,
    ) -> DefensePlan {
        let topology_identity = frame
            .base_sites()
            .iter()
            .flat_map(|b| {
                let (tag, id) = match b.id {
                    AiBaseSiteId::Start(id) => (0, id),
                    AiBaseSiteId::ResourceCluster(id) => (1, id),
                };
                [
                    tag,
                    id as u64,
                    b.position.0.to_bits() as u64,
                    b.position.1.to_bits() as u64,
                ]
            })
            .chain(frame.players().iter().flat_map(|p| {
                [
                    p.id as u64,
                    p.team_id as u64,
                    p.start_tile.0 as u64,
                    p.start_tile.1 as u64,
                ]
            }))
            .chain(frame.resources().iter().flat_map(|r| {
                [
                    r.id as u64,
                    r.position.0.to_bits() as u64,
                    r.position.1.to_bits() as u64,
                ]
            }))
            .collect::<Vec<_>>();
        if self.map.as_ref() != Some(frame.map())
            || self.player != frame.player_id()
            || self.topology_identity != topology_identity
        {
            let firing_range_tiles = self.firing_range_tiles;
            *self = Self::default();
            self.firing_range_tiles = firing_range_tiles;
            self.topology_identity = topology_identity;
            self.map = Some(frame.map().clone());
            self.player = frame.player_id();
            self.grid = Some(Grid::new(frame.map()));
            if let Some(g) = &self.grid {
                let sources = frame
                    .players()
                    .iter()
                    .filter(|p| {
                        p.id != frame.player_id()
                            && (p.team_id == 0 || p.team_id != frame.team_id())
                    })
                    .filter_map(|p| {
                        g.near((
                            (p.start_tile.0 as f32 + 0.5) * g.size,
                            (p.start_tile.1 as f32 + 0.5) * g.size,
                        ))
                    })
                    .collect::<Vec<_>>();
                self.enemy = g.field(&sources, true);
            }
        }
        let query = WorldQueries::new(frame);
        let owned: Vec<_> = owned_completed_sites
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|id| {
                query.known_base_depots(*id).iter().any(|d| {
                    d.owner == frame.player_id()
                        && frame
                            .owned()
                            .iter()
                            .any(|e| e.id == d.entity_id && e.completion == AiCompletion::Complete)
                })
            })
            .collect();
        // Asset identity/placement/activity changes immediately refresh slots and values. Node
        // amounts can change every tick; their expensive route-share rescoring is intentionally 3s.
        let assets = frame
            .owned()
            .iter()
            .filter(|e| e.kind.is_building())
            .chain(
                frame
                    .visible_allies()
                    .iter()
                    .filter(|e| e.kind.is_building()),
            )
            .chain(
                frame
                    .visible_enemies()
                    .iter()
                    .filter(|e| e.kind.is_building()),
            )
            .chain(frame.tank_traps())
            .map(|e| {
                (
                    e.id,
                    e.kind,
                    ((e.position.0 * 16.) as i32, (e.position.1 * 16.) as i32),
                    e.extractor_active.unwrap_or(false),
                )
            })
            .collect::<Vec<_>>();
        if owned == self.owned
            && unit_budget == self.last_budget
            && assets == self.last_assets
            && frame.tick().saturating_sub(self.last_tick) < 90
        {
            if let Some(p) = &self.last_plan {
                return p.clone();
            }
        }
        if owned != self.owned || unit_budget != self.geometry_capacity || self.last_plan.is_none()
        {
            self.owned = owned;
            self.geometry_capacity = unit_budget;
            self.geometry = geometry::build(self, frame);
        }
        let result = scoring::score(self, frame, unit_budget);
        self.last_assets = assets;
        self.last_budget = unit_budget;
        self.last_tick = frame.tick();
        self.last_plan = Some(result.clone());
        result
    }
}
