use super::*;

pub(super) fn balanced_approach_slot(
    index: usize,
    unit_count: usize,
    approach_count: usize,
) -> (usize, usize) {
    let approach_count = approach_count.max(1);
    let base = unit_count / approach_count;
    let remainder = unit_count % approach_count;
    let mut start = 0;
    for approach in 0..approach_count {
        let count = base + usize::from(approach < remainder);
        if index < start + count {
            return (approach, index - start);
        }
        start += count;
    }
    (approach_count - 1, 0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct DefendedBuildingSite {
    pub(super) x: f32,
    pub(super) y: f32,
    half_width: f32,
    half_height: f32,
}

impl DefendedBuildingSite {
    fn from_center(kind: EntityKind, x: f32, y: f32, tile_size: f32) -> Option<Self> {
        let stats = config::building_stats(kind)?;
        Some(Self {
            x,
            y,
            half_width: stats.foot_w as f32 * tile_size * 0.5,
            half_height: stats.foot_h as f32 * tile_size * 0.5,
        })
    }

    fn support(self, direction: (f32, f32)) -> (f32, f32) {
        (
            self.x + signed_extent(direction.0, self.half_width),
            self.y + signed_extent(direction.1, self.half_height),
        )
    }

    pub(super) fn distance2_to_footprint(self, point: (f32, f32)) -> f32 {
        let dx = (point.0 - self.x).abs() - self.half_width;
        let dy = (point.1 - self.y).abs() - self.half_height;
        squared(dx.max(0.0)) + squared(dy.max(0.0))
    }
}

fn signed_extent(direction: f32, extent: f32) -> f32 {
    if direction > 0.0 {
        extent
    } else if direction < 0.0 {
        -extent
    } else {
        0.0
    }
}

pub(super) fn defended_building_sites(
    observation: &AiObservation,
    include_planned_buildings: bool,
) -> Vec<DefendedBuildingSite> {
    let tile_size = observation.map.tile_size as f32;
    let mut sites: Vec<DefendedBuildingSite> = observation
        .owned
        .iter()
        .filter(|entity| entity.kind.is_building() && entity.hp > 0)
        .filter_map(|entity| {
            DefendedBuildingSite::from_center(entity.kind, entity.x, entity.y, tile_size)
        })
        .collect();
    if !include_planned_buildings {
        return sites;
    }
    for pending in &observation.pending_builds {
        let Some((x, y)) = building_center(
            (pending.tile_x, pending.tile_y),
            pending.kind,
            observation.map.tile_size,
        ) else {
            continue;
        };
        if sites
            .iter()
            .any(|site| dist2(site.x, site.y, x, y) <= squared(tile_size * 0.25))
        {
            continue;
        }
        if let Some(site) = DefendedBuildingSite::from_center(pending.kind, x, y, tile_size) {
            sites.push(site);
        }
    }
    sites
}

pub(super) fn defended_envelope_center(sites: &[DefendedBuildingSite]) -> Option<(f32, f32)> {
    (!sites.is_empty()).then(|| {
        let sum = sites
            .iter()
            .fold((0.0, 0.0), |sum, site| (sum.0 + site.x, sum.1 + site.y));
        (sum.0 / sites.len() as f32, sum.1 / sites.len() as f32)
    })
}

pub(super) fn defended_envelope_support(
    sites: &[DefendedBuildingSite],
    fallback: (f32, f32),
    direction: (f32, f32),
) -> (f32, f32) {
    sites
        .iter()
        .map(|site| site.support(direction))
        .max_by(|left, right| {
            (left.0 * direction.0 + left.1 * direction.1)
                .total_cmp(&(right.0 * direction.0 + right.1 * direction.1))
        })
        .unwrap_or(fallback)
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::ai_core::decision) struct LocalDefenseContact {
    pub(in crate::ai_core::decision) target_ids: Vec<u32>,
    pub(in crate::ai_core::decision) centroid: (f32, f32),
    pub(in crate::ai_core::decision) intercept: (f32, f32),
    pub(in crate::ai_core::decision) threat_value: u32,
}

/// Select the strongest currently visible local sector. Concentrating the response on one sector
/// avoids averaging a flank raid back toward the base center and lets a weak edge pull reserves.
pub(in crate::ai_core::decision) fn local_defense_contact(
    observation: &AiObservation,
) -> Option<LocalDefenseContact> {
    let geometry = LocalDefenseGeometry::from_observation_with_plans(observation, true);
    let center = geometry.envelope_center();
    let mut sectors: BTreeMap<u8, Vec<&AiEntitySummary>> = BTreeMap::new();
    for enemy in observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind.is_unit() || enemy.kind.is_building())
        .filter(|enemy| geometry.contains(enemy))
    {
        sectors
            .entry(defense_sector(center, (enemy.x, enemy.y)))
            .or_default()
            .push(enemy);
    }
    let (_, enemies) =
        sectors
            .into_iter()
            .max_by(|(left_sector, left), (right_sector, right)| {
                sector_threat_value(left)
                    .cmp(&sector_threat_value(right))
                    .then_with(|| right_sector.cmp(left_sector))
            })?;
    let threat_value = sector_threat_value(&enemies);
    let centroid = enemies
        .iter()
        .fold((0.0, 0.0), |sum, enemy| (sum.0 + enemy.x, sum.1 + enemy.y));
    let centroid = (
        centroid.0 / enemies.len() as f32,
        centroid.1 / enemies.len() as f32,
    );
    let intercept = defensive_intercept_point(observation, &geometry.buildings, center, centroid);
    let mut target_ids = enemies.iter().map(|enemy| enemy.id).collect::<Vec<_>>();
    target_ids.sort_unstable();
    Some(LocalDefenseContact {
        target_ids,
        centroid,
        intercept,
        threat_value,
    })
}

fn defensive_intercept_point(
    observation: &AiObservation,
    sites: &[DefendedBuildingSite],
    fallback: (f32, f32),
    contact: (f32, f32),
) -> (f32, f32) {
    let site = sites.iter().min_by(|left, right| {
        left.distance2_to_footprint(contact)
            .total_cmp(&right.distance2_to_footprint(contact))
    });
    let anchor = site.map_or(fallback, |site| (site.x, site.y));
    let Some(direction) = normalized_direction(anchor, contact) else {
        return anchor;
    };
    let support = site.map_or(anchor, |site| site.support(direction));
    let contact_ahead =
        (contact.0 - support.0) * direction.0 + (contact.1 - support.1) * direction.1;
    if contact_ahead <= 0.0 {
        return clamp_to_map(contact, observation.map);
    }
    let contact_distance = dist2(support.0, support.1, contact.0, contact.1).sqrt();
    let forward_distance = (1.5 * observation.map.tile_size as f32).min(contact_distance * 0.5);
    clamp_to_map(
        (
            support.0 + direction.0 * forward_distance,
            support.1 + direction.1 * forward_distance,
        ),
        observation.map,
    )
}

fn defense_sector(center: (f32, f32), point: (f32, f32)) -> u8 {
    let angle = (point.1 - center.1).atan2(point.0 - center.0);
    (((angle + std::f32::consts::PI) * 8.0 / std::f32::consts::TAU).floor() as i32).rem_euclid(8)
        as u8
}

fn sector_threat_value(enemies: &[&AiEntitySummary]) -> u32 {
    enemies
        .iter()
        .map(|enemy| {
            if enemy.kind.is_unit() {
                unit_value(enemy.kind).max(1)
            } else {
                1
            }
        })
        .sum()
}
