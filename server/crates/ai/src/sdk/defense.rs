//! Cached geometric cut/garrison hybrid for economy defense.
//!
//! This is a policy, not a guarantee of stopping an attack. Cuts separate six-tile base cores
//! and two-tile resource envelopes from a 32-tile exterior; garrisons leave approaches open.
//! Units share asset value by ground-route detours, then inverse-cube enemy approach urgency.
//! Unknown resource amounts remain unknown: valuation alone uses a 312.5-Steel
//! remaining-stock estimate (half a full patch). The public diagnostics distinguish known stock and unknown node counts.
//! All coordinates in public results except `cut_tiles` are world pixels.
mod flow;
mod geometry;
mod grid;
mod scoring;
#[cfg(test)]
mod tests;
use super::{
    AiBaseSiteId, AiCompletion, AiFrame, AiMap, AiResourceAmount, AiTerrain, AiTile, EntityKind,
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
/// Space reserved for a future building and an Engineer access ring. Slot generation treats
/// this like known occupancy, before allocating distinct defensive posts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DefenseReservation {
    pub building: EntityKind,
    pub tile: AiTile,
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
    /// Fastest public enemy-start ground approach, at the authoritative MG's base speed.
    /// Roads/slow terrain count; current enemy positions, upgrades and smoke do not.
    pub enemy_approach_seconds: Option<f32>,
    /// Economic diagnostics are fractional conserved asset shares, not literal counts.
    pub known_remaining_steel: f32,
    /// Steel nodes whose remaining quantity is unknown; no estimated stock enters the field above.
    pub unknown_resource_count: f32,
    pub active_mine_share: f32,
    pub production_share: f32,
    pub tech_share: f32,
    /// At least one reachable, known-unoccupied, entrenchable firing position exists. Does not
    /// certify that every cut tile is covered, a particular unit can get there, or smoke is clear.
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
#[derive(PartialEq)]
struct AssetSignature {
    id: u32,
    kind: EntityKind,
    position: (i32, i32),
    active: bool,
    complete: bool,
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
    enemy_sources: Vec<usize>,
    last_plan: Option<DefensePlan>,
    reservations: Vec<DefenseReservation>,
    prospective: Option<AiBaseSiteId>,
    last_tick: u32,
    last_budget: usize,
    last_assets: Vec<AssetSignature>,
    /// Number of max-flow computations; useful for profiling cache behavior.
    pub geometry_builds: u64,
}
impl DefensePlanner {
    pub fn new() -> Self {
        Self::default()
    }
    /// Restrict placements to a caller's conservative firing range (tiles), e.g. the shortest
    /// range in a mixed infantry group. Invalid values use the authoritative MG range minus 0.2.
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
    /// Slots are reachable from the union of owned base perimeters, not from each individual
    /// caller unit. The caller owns assignment, movement recovery and transient combat decisions.
    /// Static full-map distance buffers are kept per base/current economic asset, and asset fields
    /// are evicted on destruction/relocation. Map replacement resets every cache.
    pub fn plan(
        &mut self,
        frame: &AiFrame,
        owned_completed_sites: &[AiBaseSiteId],
        unit_budget: usize,
    ) -> DefensePlan {
        self.plan_sites(frame, owned_completed_sites, None, unit_budget, &[], false)
    }
    /// Plan a defensive line for one public, reachable expansion landmark before a Depot
    /// exists. Known hostile occupancy is rejected. This is a prospective policy input,
    /// not an assertion of ownership or authoritative building/movement legality.
    pub fn plan_expansion(
        &mut self,
        frame: &AiFrame,
        site: AiBaseSiteId,
        unit_budget: usize,
    ) -> DefensePlan {
        self.plan_expansion_reserved(frame, site, unit_budget, &[])
    }
    /// Reserve intended construction footprints before scoring/allocating defensive slots.
    /// Reservations affect both cut and garrison plans and invalidate cached placements.
    pub fn plan_expansion_reserved(
        &mut self,
        frame: &AiFrame,
        site: AiBaseSiteId,
        unit_budget: usize,
        reservations: &[DefenseReservation],
    ) -> DefensePlan {
        self.plan_with_expansion(frame, &[], site, unit_budget, reservations)
    }
    /// Defend every completed owned site while including one prospective expansion. The
    /// expansion is a policy input, not invented ownership. One supported expansion post is
    /// allocated first when budget permits; the remaining staffing follows economic weights.
    pub fn plan_with_expansion(
        &mut self,
        frame: &AiFrame,
        owned_completed_sites: &[AiBaseSiteId],
        expansion: AiBaseSiteId,
        unit_budget: usize,
        reservations: &[DefenseReservation],
    ) -> DefensePlan {
        let mut requested = owned_completed_sites.to_vec();
        requested.push(expansion);
        self.plan_sites(
            frame,
            &requested,
            Some(expansion),
            unit_budget,
            reservations,
            false,
        )
    }
    /// Plan a deliberate claim of a reachable public site, including known hostile occupation.
    /// This is positioning policy only; occupied footprints remain illegal for construction.
    pub fn plan_claim(
        &mut self,
        frame: &AiFrame,
        owned: &[AiBaseSiteId],
        site: AiBaseSiteId,
        budget: usize,
        reservations: &[DefenseReservation],
    ) -> DefensePlan {
        let mut requested = owned.to_vec();
        requested.push(site);
        self.plan_sites(frame, &requested, Some(site), budget, reservations, true)
    }
    fn plan_sites(
        &mut self,
        frame: &AiFrame,
        requested: &[AiBaseSiteId],
        prospective: Option<AiBaseSiteId>,
        unit_budget: usize,
        reservations: &[DefenseReservation],
        allow_hostile: bool,
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
        }
        if self.reservations != reservations {
            self.reservations = reservations.to_vec();
            self.last_plan = None;
        }
        // Public living-player/team/start changes affect threat approach, not static cuts.
        if let Some(g) = &self.grid {
            let mut sources = frame
                .players()
                .iter()
                .filter(|p| {
                    p.is_alive
                        && p.id != frame.player_id()
                        && (p.team_id == 0 || p.team_id != frame.team_id())
                })
                .filter_map(|p| {
                    g.near((
                        (p.start_tile.0 as f32 + 0.5) * g.size,
                        (p.start_tile.1 as f32 + 0.5) * g.size,
                    ))
                })
                .collect::<Vec<_>>();
            sources.sort_unstable();
            sources.dedup();
            if self.enemy.is_empty() || sources != self.enemy_sources {
                self.enemy = g.field(&sources, true);
                self.enemy_sources = sources;
                self.last_plan = None;
            }
        }
        let query = WorldQueries::new(frame);
        let prospective = prospective.filter(|id| {
            query.ranked_bases(frame.player_id()).iter().any(|b| {
                b.site.id == *id
                    && b.distance_from_main.is_some()
                    && !matches!(b.site.id, AiBaseSiteId::Start(p) if p != frame.player_id())
            }) && (allow_hostile
                || query
                    .known_base_depots(*id)
                    .iter()
                    .all(|d| d.owner == frame.player_id()))
        });
        if self.prospective != prospective {
            self.prospective = prospective;
            self.last_plan = None;
        }
        let owned: Vec<_> = requested
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|id| {
                prospective == Some(*id)
                    || query.known_base_depots(*id).iter().any(|d| {
                        d.owner == frame.player_id()
                            && frame.owned().iter().any(|e| {
                                e.id == d.entity_id && e.completion == AiCompletion::Complete
                            })
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
            .map(|e| AssetSignature {
                id: e.id,
                kind: e.kind,
                position: ((e.position.0 * 16.) as i32, (e.position.1 * 16.) as i32),
                active: e.extractor_active.unwrap_or(false),
                complete: e.completion == AiCompletion::Complete,
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
        let mut result = scoring::score(self, frame, unit_budget);
        if let Some(site) = prospective.filter(|site| {
            !result
                .sectors
                .iter()
                .any(|sector| sector.base_ids.contains(site) && sector.supported)
        }) {
            let cut = geometry::expansion_cut(self, frame, site);
            if !cut.is_empty() {
                // Recover an unsupported prospective garrison without discarding rear sectors.
                self.geometry.retain(|sector| sector.bases != [site]);
                self.geometry.extend(cut);
                result = scoring::score(self, frame, unit_budget);
            }
        }
        self.last_assets = assets;
        self.last_budget = unit_budget;
        self.last_tick = frame.tick();
        self.last_plan = Some(result.clone());
        result
    }
}
