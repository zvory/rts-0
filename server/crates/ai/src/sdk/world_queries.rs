use std::collections::{BTreeMap, BTreeSet};

use rts_rules::EntityKind;

use super::{AiEntity, AiFrame, AiRememberedContact, AiResource, AiResourceAmount, AiTerrain};

const POINT_IN_RECT_EPS_PX: f32 = 0.001;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AiTile {
    x: u32,
    y: u32,
}

impl AiTile {
    pub fn x(self) -> u32 {
        self.x
    }

    pub fn y(self) -> u32 {
        self.y
    }

    pub fn as_tuple(self) -> (u32, u32) {
        (self.x, self.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AiWorldPoint {
    x: f32,
    y: f32,
}

impl AiWorldPoint {
    pub fn x(self) -> f32 {
        self.x
    }

    pub fn y(self) -> f32 {
        self.y
    }

    pub fn as_tuple(self) -> (f32, f32) {
        (self.x, self.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnownResourceState {
    KnownExhausted,
    KnownConflict,
    NoKnownConflict,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnownBuildSiteBlocker {
    ControllerExclusion,
    KnownTerrain,
    KnownResource,
    CurrentBuilding,
    CurrentUnit,
    ProductionExit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnownBuildSite {
    Invalid,
    KnownBlocked(KnownBuildSiteBlocker),
    /// No conflict is present in the frame's known world. This is not authoritative legality,
    /// placeability, command acceptance, or a promise that hidden state is clear.
    NoKnownConflict,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KnownBuildSiteExclusions {
    sites: BTreeSet<(EntityKind, AiTile)>,
}

impl KnownBuildSiteExclusions {
    pub fn exclude(&mut self, building: EntityKind, tile: AiTile) -> bool {
        self.sites.insert((building, tile))
    }

    pub fn contains(&self, building: EntityKind, tile: AiTile) -> bool {
        self.sites.contains(&(building, tile))
    }
}

/// Deterministic indexes and uncertainty-honest queries over one fog-filtered [`AiFrame`].
///
/// The query layer has no simulation access. Current contacts, remembered contacts, public static
/// resources, and controller-owned exclusions remain distinct inputs.
pub struct WorldQueries<'a> {
    frame: &'a AiFrame,
    owned_by_id: BTreeMap<u32, usize>,
    ally_by_id: BTreeMap<u32, usize>,
    enemy_by_id: BTreeMap<u32, usize>,
    remembered_by_id: BTreeMap<u32, usize>,
    resource_by_id: BTreeMap<u32, usize>,
}

impl<'a> WorldQueries<'a> {
    pub fn new(frame: &'a AiFrame) -> Self {
        Self {
            frame,
            owned_by_id: id_index(frame.owned(), |entity| entity.id),
            ally_by_id: id_index(frame.visible_allies(), |entity| entity.id),
            enemy_by_id: id_index(frame.visible_enemies(), |entity| entity.id),
            remembered_by_id: id_index(frame.remembered_contacts(), |contact| contact.id),
            resource_by_id: id_index(frame.resources(), |resource| resource.id),
        }
    }

    pub fn owned(&self) -> &'a [AiEntity] {
        self.frame.owned()
    }

    pub fn visible_allies(&self) -> &'a [AiEntity] {
        self.frame.visible_allies()
    }

    pub fn visible_enemies(&self) -> &'a [AiEntity] {
        self.frame.visible_enemies()
    }

    pub fn remembered_contacts(&self) -> &'a [AiRememberedContact] {
        self.frame.remembered_contacts()
    }

    pub fn resources(&self) -> &'a [AiResource] {
        self.frame.resources()
    }

    pub fn owned_entity(&self, id: u32) -> Option<&'a AiEntity> {
        self.owned_by_id.get(&id).map(|index| &self.owned()[*index])
    }

    pub fn visible_ally(&self, id: u32) -> Option<&'a AiEntity> {
        self.ally_by_id
            .get(&id)
            .map(|index| &self.visible_allies()[*index])
    }

    pub fn visible_enemy(&self, id: u32) -> Option<&'a AiEntity> {
        self.enemy_by_id
            .get(&id)
            .map(|index| &self.visible_enemies()[*index])
    }

    pub fn remembered_contact(&self, id: u32) -> Option<&'a AiRememberedContact> {
        self.remembered_by_id
            .get(&id)
            .map(|index| &self.remembered_contacts()[*index])
    }

    pub fn resource(&self, id: u32) -> Option<&'a AiResource> {
        self.resource_by_id
            .get(&id)
            .map(|index| &self.resources()[*index])
    }

    pub fn known_resources(&self, kind: EntityKind) -> impl Iterator<Item = &'a AiResource> + '_ {
        self.resources().iter().filter(move |resource| {
            resource.kind == kind && resource.remaining != AiResourceAmount::Known(0)
        })
    }

    pub fn nearest_visible_enemy(&self, origin: AiWorldPoint) -> Option<&'a AiEntity> {
        nearest_by_position(self.visible_enemies(), origin, |entity| {
            (entity.id, entity.position)
        })
    }

    pub fn nearest_remembered_contact(
        &self,
        origin: AiWorldPoint,
    ) -> Option<&'a AiRememberedContact> {
        nearest_by_position(self.remembered_contacts(), origin, |contact| {
            (contact.id, contact.position)
        })
    }

    pub fn nearest_known_resource(
        &self,
        origin: AiWorldPoint,
        kind: EntityKind,
    ) -> Option<&'a AiResource> {
        self.known_resources(kind).min_by(|left, right| {
            distance2(origin, left.position)
                .total_cmp(&distance2(origin, right.position))
                .then_with(|| left.id.cmp(&right.id))
        })
    }

    pub fn known_resource_state(&self, id: u32) -> Option<KnownResourceState> {
        let resource = self.resource(id)?;
        if resource.remaining == AiResourceAmount::Known(0) {
            return Some(KnownResourceState::KnownExhausted);
        }
        let conflict = self
            .current_entities()
            .any(|entity| entity.latched_resource == Some(id))
            || self.current_entities().any(|entity| {
                entity.kind.extracted_resource_kind() == Some(resource.kind)
                    && point_overlaps_building(
                        resource.position,
                        entity,
                        self.frame.map().tile_size,
                    )
            });
        Some(if conflict {
            KnownResourceState::KnownConflict
        } else {
            KnownResourceState::NoKnownConflict
        })
    }

    pub fn tile(&self, x: u32, y: u32) -> Option<AiTile> {
        (x < self.frame.map().width && y < self.frame.map().height).then_some(AiTile { x, y })
    }

    pub fn world_point(&self, x: f32, y: f32) -> Option<AiWorldPoint> {
        let map = self.frame.map();
        let width = map.width.checked_mul(map.tile_size)? as f32;
        let height = map.height.checked_mul(map.tile_size)? as f32;
        (x.is_finite() && y.is_finite() && x >= 0.0 && y >= 0.0 && x < width && y < height)
            .then_some(AiWorldPoint { x, y })
    }

    pub fn world_to_tile(&self, point: AiWorldPoint) -> Option<AiTile> {
        let tile_size = self.frame.map().tile_size;
        if tile_size == 0 {
            return None;
        }
        self.tile(
            (point.x / tile_size as f32).floor() as u32,
            (point.y / tile_size as f32).floor() as u32,
        )
    }

    pub fn tile_center(&self, tile: AiTile) -> Option<AiWorldPoint> {
        let tile_size = self.frame.map().tile_size as f32;
        self.world_point(
            (tile.x as f32 + 0.5) * tile_size,
            (tile.y as f32 + 0.5) * tile_size,
        )
    }

    pub fn known_build_site(
        &self,
        building: EntityKind,
        tile: AiTile,
        exclusions: &KnownBuildSiteExclusions,
    ) -> KnownBuildSite {
        self.known_build_site_with_production_policy(building, tile, exclusions, true)
    }

    /// Legacy profiles historically used the global producer list for the exit-tile check. Keep
    /// that policy isolated while sharing every other known-world placement operation.
    pub(crate) fn known_build_site_compatibility(
        &self,
        building: EntityKind,
        tile: AiTile,
        exclusions: &KnownBuildSiteExclusions,
    ) -> KnownBuildSite {
        self.known_build_site_with_production_policy(building, tile, exclusions, false)
    }

    fn known_build_site_with_production_policy(
        &self,
        building: EntityKind,
        tile: AiTile,
        exclusions: &KnownBuildSiteExclusions,
        faction_aware_production: bool,
    ) -> KnownBuildSite {
        let Some(stats) = rts_rules::balance::building_stats(building) else {
            return KnownBuildSite::Invalid;
        };
        if exclusions.contains(building, tile) {
            return KnownBuildSite::KnownBlocked(KnownBuildSiteBlocker::ControllerExclusion);
        }

        let Some(right) = tile.x.checked_add(stats.foot_w) else {
            return KnownBuildSite::Invalid;
        };
        let Some(bottom) = tile.y.checked_add(stats.foot_h) else {
            return KnownBuildSite::Invalid;
        };
        if right > self.frame.map().width || bottom > self.frame.map().height {
            return KnownBuildSite::Invalid;
        }

        for y in tile.y..bottom {
            for x in tile.x..right {
                let Some(index) = tile_index(self.frame, x, y) else {
                    return KnownBuildSite::Invalid;
                };
                if !is_passable(self.frame.map().terrain[index]) {
                    return KnownBuildSite::KnownBlocked(KnownBuildSiteBlocker::KnownTerrain);
                }
                if self.frame.map().no_building_tiles.contains(&(x, y)) {
                    return KnownBuildSite::KnownBlocked(KnownBuildSiteBlocker::KnownTerrain);
                }
                if self.resource_at_tile(x, y) {
                    return KnownBuildSite::KnownBlocked(KnownBuildSiteBlocker::KnownResource);
                }
            }
        }

        if self.current_buildings().any(|existing| {
            let Some(existing_tile) = self.building_top_left(existing) else {
                return false;
            };
            !crate::ai_shared::footprints_respect_clearance(
                building,
                tile.x,
                tile.y,
                existing.kind,
                existing_tile.x,
                existing_tile.y,
            )
        }) {
            return KnownBuildSite::KnownBlocked(KnownBuildSiteBlocker::CurrentBuilding);
        }

        let tile_size = self.frame.map().tile_size as f32;
        let footprint = (
            tile.x as f32 * tile_size,
            tile.y as f32 * tile_size,
            right as f32 * tile_size,
            bottom as f32 * tile_size,
        );
        if self.owned().iter().any(|entity| {
            entity.kind.is_unit()
                && unit_circle_touches_rect(
                    entity.position,
                    rts_rules::balance::unit_placement_radius(entity.kind),
                    footprint,
                )
        }) {
            return KnownBuildSite::KnownBlocked(KnownBuildSiteBlocker::CurrentUnit);
        }

        let has_production_exit = if faction_aware_production {
            !rts_rules::economy::trainable_units_for_faction(self.frame.faction_id(), building)
                .is_empty()
        } else {
            !rts_rules::economy::trainable_units(building).is_empty()
        };
        if has_production_exit {
            let spawn_x = tile.x + stats.foot_w / 2;
            let Some(spawn_y) = tile.y.checked_add(stats.foot_h) else {
                return KnownBuildSite::Invalid;
            };
            let Some(index) = tile_index(self.frame, spawn_x, spawn_y) else {
                return KnownBuildSite::Invalid;
            };
            if !is_passable(self.frame.map().terrain[index])
                || self.resource_at_tile(spawn_x, spawn_y)
                || self
                    .current_buildings()
                    .any(|existing| self.tile_within_building_clearance(spawn_x, spawn_y, existing))
            {
                return KnownBuildSite::KnownBlocked(KnownBuildSiteBlocker::ProductionExit);
            }
        }

        KnownBuildSite::NoKnownConflict
    }

    /// Runs the established outward ring traversal and tie-breaking over known-world answers.
    pub fn find_known_build_site_near(
        &self,
        start: AiTile,
        building: EntityKind,
        exclusions: &KnownBuildSiteExclusions,
    ) -> Option<AiTile> {
        let skipped = exclusions
            .sites
            .iter()
            .filter(|(kind, _)| *kind == building)
            .map(|(_, tile)| tile.as_tuple())
            .collect();
        crate::ai_shared::find_build_spot_near_start(
            self.frame.map().width,
            self.frame.map().height,
            start.as_tuple(),
            building,
            &skipped,
            |x, y| {
                self.tile(x, y).is_some_and(|tile| {
                    self.known_build_site(building, tile, exclusions)
                        == KnownBuildSite::NoKnownConflict
                })
            },
        )
        .and_then(|(x, y)| self.tile(x, y))
    }

    fn current_entities(&self) -> impl Iterator<Item = &'a AiEntity> + '_ {
        self.owned()
            .iter()
            .chain(self.visible_allies())
            .chain(self.visible_enemies())
    }

    fn current_buildings(&self) -> impl Iterator<Item = &'a AiEntity> + '_ {
        self.current_entities()
            .filter(|entity| entity.kind.is_building())
    }

    fn resource_at_tile(&self, x: u32, y: u32) -> bool {
        self.resources().iter().any(|resource| {
            self.world_point(resource.position.0, resource.position.1)
                .and_then(|point| self.world_to_tile(point))
                .is_some_and(|tile| tile.x == x && tile.y == y)
        })
    }

    fn building_top_left(&self, entity: &AiEntity) -> Option<AiTile> {
        let stats = rts_rules::balance::building_stats(entity.kind)?;
        let center = self.world_point(entity.position.0, entity.position.1)?;
        let center_tile = self.world_to_tile(center)?;
        self.tile(
            center_tile.x.saturating_sub(stats.foot_w / 2),
            center_tile.y.saturating_sub(stats.foot_h / 2),
        )
    }

    fn tile_within_building_clearance(&self, x: u32, y: u32, entity: &AiEntity) -> bool {
        let Some(stats) = rts_rules::balance::building_stats(entity.kind) else {
            return false;
        };
        let Some(top_left) = self.building_top_left(entity) else {
            return false;
        };
        let clearance = crate::ai_shared::building_clearance_tiles(entity.kind);
        let left = top_left.x as i64 - clearance as i64;
        let top = top_left.y as i64 - clearance as i64;
        let right = top_left.x as i64 + stats.foot_w as i64 - 1 + clearance as i64;
        let bottom = top_left.y as i64 + stats.foot_h as i64 - 1 + clearance as i64;
        (x as i64) >= left && (x as i64) <= right && (y as i64) >= top && (y as i64) <= bottom
    }
}

fn id_index<T>(items: &[T], id: impl Fn(&T) -> u32) -> BTreeMap<u32, usize> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| (id(item), index))
        .collect()
}

fn nearest_by_position<T>(
    items: &[T],
    origin: AiWorldPoint,
    identity_and_position: impl Fn(&T) -> (u32, (f32, f32)),
) -> Option<&T> {
    items.iter().min_by(|left, right| {
        let (left_id, left_position) = identity_and_position(left);
        let (right_id, right_position) = identity_and_position(right);
        distance2(origin, left_position)
            .total_cmp(&distance2(origin, right_position))
            .then_with(|| left_id.cmp(&right_id))
    })
}

fn distance2(origin: AiWorldPoint, position: (f32, f32)) -> f32 {
    let dx = position.0 - origin.x;
    let dy = position.1 - origin.y;
    dx * dx + dy * dy
}

fn tile_index(frame: &AiFrame, x: u32, y: u32) -> Option<usize> {
    if x >= frame.map().width || y >= frame.map().height {
        return None;
    }
    y.checked_mul(frame.map().width)
        .and_then(|row| row.checked_add(x))
        .and_then(|index| usize::try_from(index).ok())
        .filter(|index| *index < frame.map().terrain.len())
}

fn is_passable(terrain: AiTerrain) -> bool {
    !matches!(
        terrain,
        AiTerrain::Rock | AiTerrain::Water | AiTerrain::Unknown(_)
    )
}

fn point_overlaps_building(point: (f32, f32), building: &AiEntity, tile_size: u32) -> bool {
    let Some(stats) = rts_rules::balance::building_stats(building.kind) else {
        return false;
    };
    let tile_size = tile_size as f32;
    let half_w = stats.foot_w as f32 * tile_size * 0.5;
    let half_h = stats.foot_h as f32 * tile_size * 0.5;
    point.0 >= building.position.0 - half_w - POINT_IN_RECT_EPS_PX
        && point.0 <= building.position.0 + half_w + POINT_IN_RECT_EPS_PX
        && point.1 >= building.position.1 - half_h - POINT_IN_RECT_EPS_PX
        && point.1 <= building.position.1 + half_h + POINT_IN_RECT_EPS_PX
}

pub(crate) fn unit_circle_touches_rect(
    position: (f32, f32),
    radius: f32,
    rect: (f32, f32, f32, f32),
) -> bool {
    let nearest_x = position.0.clamp(rect.0, rect.2);
    let nearest_y = position.1.clamp(rect.1, rect.3);
    let dx = position.0 - nearest_x;
    let dy = position.1 - nearest_y;
    dx * dx + dy * dy <= radius * radius
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use crate::selfplay::player_view::{
        footprint_placeable_from_snapshot, occupied_tiles_from_snapshot,
    };
    use rts_sim::game::{Game, PlayerInit};
    use rts_sim::protocol::{self, kinds, states, ResourceNode};

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
    fn coordinate_helpers_reject_non_finite_and_out_of_bounds_values() {
        let game = Game::new_without_ai_controllers(&players(), 51);
        let frame = AiFrame::from_host(
            &game.start_payload(),
            &game.snapshot_for(1),
            1,
            [],
            Some(&[1, 2]),
        )
        .unwrap();
        let queries = WorldQueries::new(&frame);
        assert!(queries.world_point(f32::NAN, 0.0).is_none());
        assert!(queries.world_point(f32::INFINITY, 0.0).is_none());
        assert!(queries.tile(frame.map().width, 0).is_none());
        let tile = queries.tile(1, 2).unwrap();
        assert_eq!(
            queries.world_to_tile(queries.tile_center(tile).unwrap()),
            Some(tile)
        );
    }

    #[test]
    fn resource_conflicts_use_the_frame_tile_size() {
        let game = Game::new_without_ai_controllers(&players(), 52);
        let mut start = game.start_payload();
        let mut snapshot = game.snapshot_for(1);
        start.map.tile_size = rts_rules::balance::TILE_SIZE * 2;

        let pump_x = 128.0;
        let pump_y = 128.0;
        let resource_id = 999_999;
        let resource_position = (pump_x + 24.0, pump_y);
        start.map.resources.push(ResourceNode {
            id: resource_id,
            kind: protocol::kinds::OIL.to_string(),
            x: resource_position.0,
            y: resource_position.1,
        });
        snapshot.entities.push(protocol::EntityView::new(
            999_998,
            1,
            protocol::kind_to_wire(EntityKind::PumpJack),
            pump_x,
            pump_y,
            50,
            50,
            states::IDLE,
        ));

        let frame = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1, 2])).unwrap();
        let queries = WorldQueries::new(&frame);
        let pump = queries
            .owned()
            .iter()
            .find(|entity| entity.id == 999_998)
            .unwrap();
        assert!(!point_overlaps_building(
            resource_position,
            pump,
            rts_rules::balance::TILE_SIZE,
        ));
        assert_eq!(
            queries.known_resource_state(resource_id),
            Some(KnownResourceState::KnownConflict)
        );
    }

    #[test]
    fn steel_mines_conflict_with_their_known_resource_patch() {
        let game = Game::new_without_ai_controllers(&players(), 54);
        let mut start = game.start_payload();
        let mut snapshot = game.snapshot_for(1);
        let resource_id = 999_997;
        let position = (160.0, 160.0);
        start.map.resources.push(ResourceNode {
            id: resource_id,
            kind: protocol::kinds::STEEL.to_string(),
            x: position.0,
            y: position.1,
        });
        snapshot.entities.push(protocol::EntityView::new(
            999_996,
            1,
            protocol::kind_to_wire(EntityKind::SteelMine),
            position.0,
            position.1,
            50,
            50,
            states::IDLE,
        ));

        let frame = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1, 2])).unwrap();
        assert_eq!(
            WorldQueries::new(&frame).known_resource_state(resource_id),
            Some(KnownResourceState::KnownConflict)
        );
    }

    #[test]
    fn extractors_do_not_conflict_with_a_different_resource_kind() {
        let game = Game::new_without_ai_controllers(&players(), 55);
        let mut start = game.start_payload();
        let mut snapshot = game.snapshot_for(1);
        let resource_id = 999_995;
        let position = (160.0, 160.0);
        start.map.resources.push(ResourceNode {
            id: resource_id,
            kind: protocol::kinds::STEEL.to_string(),
            x: position.0,
            y: position.1,
        });
        snapshot.entities.push(protocol::EntityView::new(
            999_994,
            1,
            protocol::kind_to_wire(EntityKind::PumpJack),
            position.0,
            position.1,
            50,
            50,
            states::IDLE,
        ));

        let frame = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1, 2])).unwrap();
        assert_eq!(
            WorldQueries::new(&frame).known_resource_state(resource_id),
            Some(KnownResourceState::NoKnownConflict)
        );
    }

    #[test]
    fn hidden_snapshot_variants_produce_identical_queries() {
        let game = Game::new_without_ai_controllers(&players(), 53);
        let start = game.start_payload();
        let fogged = game.snapshot_for(1);
        let mut hidden_variant = fogged.clone();
        let mut hidden = game
            .snapshot_full_for(1)
            .entities
            .into_iter()
            .find(|entity| {
                entity.owner == 2 && !fogged.entities.iter().any(|seen| seen.id == entity.id)
            })
            .unwrap();
        hidden.vision_only = true;
        hidden_variant.entities.push(hidden);

        let first = AiFrame::from_host(&start, &fogged, 1, [], Some(&[1, 2])).unwrap();
        let second = AiFrame::from_host(&start, &hidden_variant, 1, [], Some(&[1, 2])).unwrap();
        assert_eq!(first, second);
        let first_queries = WorldQueries::new(&first);
        let second_queries = WorldQueries::new(&second);
        assert_eq!(
            first_queries.visible_enemies(),
            second_queries.visible_enemies()
        );
        assert_eq!(first_queries.resources(), second_queries.resources());
        let start_tile = first_queries.tile(8, 8).unwrap();
        assert_eq!(
            first_queries.known_build_site(
                EntityKind::Depot,
                start_tile,
                &KnownBuildSiteExclusions::default()
            ),
            second_queries.known_build_site(
                EntityKind::Depot,
                start_tile,
                &KnownBuildSiteExclusions::default()
            )
        );
    }

    #[test]
    fn known_placement_matches_legacy_grid_and_ring_order() {
        for seed in [59, 61, 67] {
            let game = Game::new_without_ai_controllers(&players(), seed);
            let start = game.start_payload();
            let snapshot = game.snapshot_for(1);
            let frame = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1, 2])).unwrap();
            let queries = WorldQueries::new(&frame);
            let occupied = occupied_tiles_from_snapshot(&start.map, &snapshot);
            let exclusions = KnownBuildSiteExclusions::default();

            for building in [EntityKind::Depot, EntityKind::Barracks, EntityKind::Factory] {
                for y in 0..start.map.height {
                    for x in 0..start.map.width {
                        let old = footprint_placeable_from_snapshot(
                            &start.map, &snapshot, 1, building, x, y, &occupied,
                        );
                        let new = queries.tile(x, y).is_some_and(|tile| {
                            queries.known_build_site(building, tile, &exclusions)
                                == KnownBuildSite::NoKnownConflict
                        });
                        assert_eq!(old, new, "seed={seed} building={building} tile=({x},{y})");
                    }
                }
            }

            let own_start = frame
                .players()
                .iter()
                .find(|player| player.id == frame.player_id())
                .unwrap()
                .start_tile;
            let start_tile = queries.tile(own_start.0, own_start.1).unwrap();
            let old = crate::ai_shared::find_build_spot_near_start(
                start.map.width,
                start.map.height,
                own_start,
                EntityKind::Depot,
                &BTreeSet::new(),
                |x, y| {
                    footprint_placeable_from_snapshot(
                        &start.map,
                        &snapshot,
                        1,
                        EntityKind::Depot,
                        x,
                        y,
                        &occupied,
                    )
                },
            );
            assert_eq!(
                queries
                    .find_known_build_site_near(
                        start_tile,
                        EntityKind::Depot,
                        &KnownBuildSiteExclusions::default(),
                    )
                    .map(AiTile::as_tuple),
                old,
                "seed={seed} should preserve ring traversal and tie-breaking"
            );

            if let Some(first) = old {
                let old_skips = BTreeSet::from([first]);
                let old_second = crate::ai_shared::find_build_spot_near_start(
                    start.map.width,
                    start.map.height,
                    own_start,
                    EntityKind::Depot,
                    &old_skips,
                    |x, y| {
                        footprint_placeable_from_snapshot(
                            &start.map,
                            &snapshot,
                            1,
                            EntityKind::Depot,
                            x,
                            y,
                            &occupied,
                        )
                    },
                );
                let mut query_exclusions = KnownBuildSiteExclusions::default();
                query_exclusions
                    .exclude(EntityKind::Depot, queries.tile(first.0, first.1).unwrap());
                assert_eq!(
                    queries
                        .find_known_build_site_near(
                            start_tile,
                            EntityKind::Depot,
                            &query_exclusions,
                        )
                        .map(AiTile::as_tuple),
                    old_second,
                    "seed={seed} should preserve failed-site exclusion order"
                );
            }
        }
    }

    #[test]
    fn known_placement_rejects_no_building_tiles() {
        let game = Game::new_without_ai_controllers(&players(), 71);
        let mut start = game.start_payload();
        let snapshot = game.snapshot_for(1);
        let initial = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1, 2])).unwrap();
        let initial_queries = WorldQueries::new(&initial);
        let exclusions = KnownBuildSiteExclusions::default();
        let clear = (0..start.map.height)
            .flat_map(|y| (0..start.map.width).map(move |x| (x, y)))
            .find(|&(x, y)| {
                initial_queries.tile(x, y).is_some_and(|tile| {
                    initial_queries.known_build_site(EntityKind::TankTrap, tile, &exclusions)
                        == KnownBuildSite::NoKnownConflict
                })
            })
            .expect("flat map should have a clear Tank Trap site");
        start.map.no_building_tiles.push(protocol::MapTile {
            x: clear.0,
            y: clear.1,
        });

        let frame = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1, 2])).unwrap();
        let queries = WorldQueries::new(&frame);
        assert_eq!(
            queries.known_build_site(
                EntityKind::TankTrap,
                queries.tile(clear.0, clear.1).unwrap(),
                &exclusions,
            ),
            KnownBuildSite::KnownBlocked(KnownBuildSiteBlocker::KnownTerrain),
        );
    }

    #[test]
    fn known_placement_rejects_owned_unit_touching_footprint() {
        let game = Game::new_without_ai_controllers(&players(), 73);
        let start = game.start_payload();
        let mut snapshot = game.snapshot_for(1);
        let initial = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1, 2])).unwrap();
        let initial_queries = WorldQueries::new(&initial);
        let exclusions = KnownBuildSiteExclusions::default();
        let clear = (0..start.map.height)
            .flat_map(|y| (0..start.map.width).map(move |x| (x, y)))
            .find(|&(x, y)| {
                initial_queries.tile(x, y).is_some_and(|tile| {
                    initial_queries.known_build_site(EntityKind::TankTrap, tile, &exclusions)
                        == KnownBuildSite::NoKnownConflict
                })
            })
            .expect("map should have a clear Tank Trap site");
        let tile_size = start.map.tile_size as f32;
        let worker = snapshot
            .entities
            .iter_mut()
            .find(|entity| entity.owner == 1 && entity.kind == kinds::WORKER)
            .expect("player should have a worker");
        worker.x = clear.0 as f32 * tile_size + tile_size * 0.5;
        worker.y = clear.1 as f32 * tile_size + tile_size * 0.5;

        let frame = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1, 2])).unwrap();
        let queries = WorldQueries::new(&frame);
        let tile = queries.tile(clear.0, clear.1).unwrap();
        assert_eq!(
            queries.known_build_site(EntityKind::TankTrap, tile, &exclusions),
            KnownBuildSite::KnownBlocked(KnownBuildSiteBlocker::CurrentUnit)
        );
    }

    #[test]
    fn unit_touching_footprint_edge_counts_as_blocked() {
        assert!(unit_circle_touches_rect(
            (110.0, 105.0),
            10.0,
            (100.0, 100.0, 105.0, 110.0),
        ));
        assert!(!unit_circle_touches_rect(
            (110.01, 105.0),
            5.0,
            (100.0, 100.0, 105.0, 110.0),
        ));
    }
}
