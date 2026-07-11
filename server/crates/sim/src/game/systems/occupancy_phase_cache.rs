use crate::game::entity::{EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::services::occupancy::Occupancy;

/// Exact building facts that determine an [`Occupancy`] snapshot.
///
/// Positions use their bit representation so equality cannot produce a false cache hit. Entity
/// iteration is id-ordered, making the complete vector deterministic without a probabilistic
/// fingerprint. Include every building, even one whose current blocker class is `None`, so a
/// future rule change cannot silently make the cache key incomplete.
#[derive(Debug, PartialEq, Eq)]
struct OccupancyTopology(Vec<OccupancyTopologyEntry>);

#[derive(Debug, PartialEq, Eq)]
struct OccupancyTopologyEntry {
    id: u32,
    owner: u32,
    kind: EntityKind,
    pos_x_bits: u32,
    pos_y_bits: u32,
}

impl OccupancyTopology {
    fn capture(entities: &EntityStore) -> Self {
        Self(
            entities
                .iter()
                .filter(|entity| entity.is_building())
                .map(|entity| OccupancyTopologyEntry {
                    id: entity.id,
                    owner: entity.owner,
                    kind: entity.kind,
                    pos_x_bits: entity.pos_x.to_bits(),
                    pos_y_bits: entity.pos_y.to_bits(),
                })
                .collect(),
        )
    }
}

/// Tick-local cache shared by the three named derived-state phase boundaries.
///
/// Unit movement never changes static occupancy, while construction placement and building death
/// do. Each boundary captures and compares the complete topology before cloning the immutable,
/// shared occupancy data, so a mutation rebuilds immediately without persistent invalidation
/// state.
pub(super) struct OccupancyPhaseCache<'a> {
    map: &'a Map,
    topology: Option<OccupancyTopology>,
    occupancy: Option<Occupancy<'a>>,
    #[cfg(test)]
    rebuild_count: usize,
}

impl<'a> OccupancyPhaseCache<'a> {
    pub(super) fn new(map: &'a Map) -> Self {
        Self {
            map,
            topology: None,
            occupancy: None,
            #[cfg(test)]
            rebuild_count: 0,
        }
    }

    pub(super) fn snapshot(&mut self, entities: &EntityStore) -> Occupancy<'a> {
        let topology = OccupancyTopology::capture(entities);
        if self.topology.as_ref() != Some(&topology) || self.occupancy.is_none() {
            self.occupancy = Some(Occupancy::build(self.map, entities));
            self.topology = Some(topology);
            #[cfg(test)]
            {
                self.rebuild_count += 1;
            }
        }
        self.occupancy
            .as_ref()
            .cloned()
            .unwrap_or_else(|| Occupancy::build(self.map, entities))
    }

    #[cfg(test)]
    fn rebuild_count(&self) -> usize {
        self.rebuild_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::services::occupancy::footprint_center;
    use crate::protocol::terrain;

    fn flat_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: Vec::new(),
            expansion_sites: Vec::new(),
        }
    }

    #[test]
    fn reuses_clearance_for_unchanged_and_unit_movement_phases() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, 96.0, 96.0)
            .expect("rifleman should spawn");
        let mut cache = OccupancyPhaseCache::new(&map);

        cache.snapshot(&entities);
        cache.snapshot(&entities);
        assert_eq!(cache.rebuild_count(), 1);

        entities
            .get_mut(unit)
            .expect("rifleman should remain")
            .set_position(320.0, 320.0);
        cache.snapshot(&entities);

        assert_eq!(
            cache.rebuild_count(),
            1,
            "unit-only position changes must not rebuild static clearance"
        );
    }

    #[test]
    fn rebuilds_after_construction_placement_and_building_death() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let mut cache = OccupancyPhaseCache::new(&map);

        cache.snapshot(&entities);
        let (x, y) = footprint_center(&map, EntityKind::Depot, 6, 6);
        let depot = entities
            .spawn_building(1, EntityKind::Depot, x, y, false)
            .expect("depot scaffold should spawn");
        let after_construction = cache.snapshot(&entities);

        assert!(after_construction.building_blocked_at_tile(6, 6));
        assert_eq!(cache.rebuild_count(), 2);

        entities.remove(depot);
        let after_death = cache.snapshot(&entities);

        assert!(!after_death.building_blocked_at_tile(6, 6));
        assert_eq!(
            cache.rebuild_count(),
            3,
            "building removal must invalidate the phase cache immediately"
        );
    }

    #[test]
    fn exact_key_detects_building_position_and_owner_changes() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let (old_x, old_y) = footprint_center(&map, EntityKind::TankTrap, 5, 5);
        let trap = entities
            .spawn_building(1, EntityKind::TankTrap, old_x, old_y, true)
            .expect("tank trap should spawn");
        let mut cache = OccupancyPhaseCache::new(&map);
        cache.snapshot(&entities);

        let (new_x, new_y) = footprint_center(&map, EntityKind::TankTrap, 10, 10);
        entities
            .get_mut(trap)
            .expect("tank trap should remain")
            .set_position(new_x, new_y);
        let moved = cache.snapshot(&entities);
        assert!(!moved.building_blocked_at_tile(5, 5));
        assert!(moved.building_blocked_at_tile(10, 10));

        entities
            .get_mut(trap)
            .expect("tank trap should remain")
            .owner = 2;
        cache.snapshot(&entities);
        assert_eq!(cache.rebuild_count(), 3);
    }
}
