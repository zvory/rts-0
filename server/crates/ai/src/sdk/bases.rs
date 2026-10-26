//! Static base geography and fog-respecting evidence, independent of strategy policy.
use super::{AiFrame, EntityKind, WorldQueries};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AiBaseSiteId {
    Start(u32),
    ResourceCluster(u32),
}

/// A public geographic landmark, not a legal Depot footprint or proof of occupation.
#[derive(Clone, Debug, PartialEq)]
pub struct AiBaseSite {
    pub id: AiBaseSiteId,
    /// World pixels. Starting sites use the center of the public start tile.
    pub position: (f32, f32),
    pub resource_ids: Vec<u32>,
    /// Static ground path lengths in world pixels, keyed by original-main player ID.
    /// Missing entries mean no ground route. Ignores units and buildings; terrain paths use
    /// cardinal/diagonal tile costs of 1/1.4 and never cut blocked corners.
    pub ground_distances_from_starts: std::collections::BTreeMap<u32, f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AiRankedBase<'a> {
    pub site: &'a AiBaseSite,
    /// One-based geography rank: original main is always 1, regardless of its survival.
    pub rank: usize,
    /// Static ground path length in world pixels; `None` means unreachable by ground.
    pub distance_from_main: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiBaseEvidence {
    Owned,
    Visible,
    Remembered { observed_tick: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AiBaseDepot {
    pub entity_id: u32,
    pub owner: u32,
    pub evidence: AiBaseEvidence,
}

impl WorldQueries<'_> {
    /// All public sites ordered relative to `player_id`, including other players' starting sites.
    /// Stable site IDs break equal-distance ties. Resource depletion and occupancy never reorder
    /// sites. Reachable sites precede unreachable sites, which are ordered by stable ID and
    /// report no distance. Unknown players return no sites. Rank is not construction order.
    pub fn ranked_bases(&self, player_id: u32) -> Vec<AiRankedBase<'_>> {
        let Some(main) = self
            .frame
            .base_sites()
            .iter()
            .find(|site| site.id == AiBaseSiteId::Start(player_id))
        else {
            return Vec::new();
        };
        let mut sites: Vec<_> = self.frame.base_sites().iter().collect();
        sites.sort_by(|a, b| {
            (a.id != main.id)
                .cmp(&(b.id != main.id))
                .then_with(|| {
                    match (
                        a.ground_distances_from_starts.get(&player_id),
                        b.ground_distances_from_starts.get(&player_id),
                    ) {
                        (Some(a), Some(b)) => a.total_cmp(b),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    }
                })
                .then_with(|| a.id.cmp(&b.id))
        });
        sites
            .into_iter()
            .enumerate()
            .map(|(index, site)| AiRankedBase {
                site,
                rank: index + 1,
                distance_from_main: site.ground_distances_from_starts.get(&player_id).copied(),
            })
            .collect()
    }

    pub fn base(&self, player_id: u32, rank: usize) -> Option<AiRankedBase<'_>> {
        rank.checked_sub(1)
            .and_then(|index| self.ranked_bases(player_id).get(index).copied())
    }

    /// Known Depots associated with a site's nearest landmark, within twelve tiles.
    /// An empty result means no known Depot, never confirmed vacancy. Remembered evidence is
    /// stale; visible and owned records win over remembered copies of the same entity.
    pub fn known_base_depots(&self, site_id: AiBaseSiteId) -> Vec<AiBaseDepot> {
        let mut depots = std::collections::BTreeMap::new();
        for contact in self
            .remembered_contacts()
            .iter()
            .filter(|e| e.kind == EntityKind::ResourceDepot)
        {
            if associated_site(self.frame, contact.position) == Some(site_id) {
                depots.insert(
                    contact.id,
                    AiBaseDepot {
                        entity_id: contact.id,
                        owner: contact.owner,
                        evidence: AiBaseEvidence::Remembered {
                            observed_tick: contact.observed_tick,
                        },
                    },
                );
            }
        }
        for (entities, evidence) in [
            (self.visible_allies(), AiBaseEvidence::Visible),
            (self.visible_enemies(), AiBaseEvidence::Visible),
            (self.owned(), AiBaseEvidence::Owned),
        ] {
            for entity in entities
                .iter()
                .filter(|e| e.kind == EntityKind::ResourceDepot)
            {
                // Remove stale association even when the current entity belongs to another site.
                depots.remove(&entity.id);
                if associated_site(self.frame, entity.position) == Some(site_id) {
                    depots.insert(
                        entity.id,
                        AiBaseDepot {
                            entity_id: entity.id,
                            owner: entity.owner,
                            evidence,
                        },
                    );
                }
            }
        }
        depots.into_values().collect()
    }
}

fn associated_site(frame: &AiFrame, position: (f32, f32)) -> Option<AiBaseSiteId> {
    let radius = frame.map().tile_size as f32 * 12.0;
    frame
        .base_sites()
        .iter()
        .filter(|site| distance2(site.position, position) <= radius * radius)
        .min_by(|a, b| {
            distance2(a.position, position)
                .total_cmp(&distance2(b.position, position))
                .then_with(|| a.id.cmp(&b.id))
        })
        .map(|site| site.id)
}

fn distance2(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)
}
