//! Jeff's AI V1.1 finishes what it left unfinished, and keeps a spare Engineer while Steel piles up.
//!
//! - **Resume** (`resume`): an unfinished Factory, Engineering Complex, Barracks, Training Centre,
//!   Steelworks or Depot that no Engineer has worked on for `UNATTENDED_TICKS`, with no enemy fighter
//!   within `SAFE_TILES`, gets a free Engineer sent back to it. Only Resource Depots were resumed:
//!   on Crossroads vs Rush (P2, seeds 0/1/4) a Rifleman killed the Factory's builder at 2:51. The
//!   unfinished Factory still counted as built, so none was placed again, no Engineering Complex or
//!   Tank ever came, and Jeff died at 11:41 with 4,800 Steel banked and its last Engineer idle at
//!   the natural.
//! - **Spare Engineer** (`spareworker`): with one Engineer and `SPARE_STEEL` banked after
//!   `SPARE_FROM_TICK`, a second one is trained, so one lost builder does not stop every build.

use super::*;
use crate::ai_core::decision::geometry::{dist2, footprint_top_left_for_center, squared};

/// How long a site must stand with no Engineer on it before one is sent back.
const UNATTENDED_TICKS: u32 = config::TICK_HZ * 3;
/// No Engineer goes back while an enemy fighter stands this close to the site.
const SAFE_TILES: f32 = 10.0;
const SPARE_STEEL: u32 = 500;
const SPARE_FROM_TICK: u32 = config::TICK_HZ * 150;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct ResumeMemory {
    /// Unfinished site -> the tick it was first seen with no Engineer on it.
    unattended: BTreeMap<u32, u32>,
    /// The last site an Engineer was sent back to, and when.
    pub(in crate::ai_core::decision) sent: Option<(u32, EntityKind, u32)>,
}

fn resumable(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Factory
            | EntityKind::EngineeringComplex
            | EntityKind::Barracks
            | EntityKind::TrainingCentre
            | EntityKind::Steelworks
            | EntityKind::Depot
    )
}

/// The order sites are resumed in: tech first.
fn rank(kind: EntityKind) -> u8 {
    match kind {
        EntityKind::Factory => 0,
        EntityKind::EngineeringComplex => 1,
        EntityKind::TrainingCentre => 2,
        EntityKind::Barracks => 3,
        _ => 4,
    }
}

fn site_tile(observation: &AiObservation, site: &AiEntitySummary) -> Option<(u32, u32)> {
    let ts = observation.map.tile_size as f32;
    if ts <= 0.0 || !site.x.is_finite() || !site.y.is_finite() || site.x < 0.0 || site.y < 0.0 {
        return None;
    }
    let center = ((site.x / ts).floor() as u32, (site.y / ts).floor() as u32);
    let (tile_x, tile_y) = footprint_top_left_for_center(center, site.kind)?;
    let stats = config::building_stats(site.kind)?;
    (tile_x <= observation.map.width.saturating_sub(stats.foot_w)
        && tile_y <= observation.map.height.saturating_sub(stats.foot_h))
    .then_some((tile_x, tile_y))
}

fn attended(observation: &AiObservation, site: &AiEntitySummary, tile: (u32, u32)) -> bool {
    observation.owned.iter().any(|unit| {
        unit.kind == EntityKind::Worker
            && unit.state == AiEntityState::Build
            && unit.target_id == Some(site.id)
    }) || observation.pending_builds.iter().any(|intent| {
        intent.kind == site.kind && intent.tile_x == tile.0 && intent.tile_y == tile.1
    })
}

fn threatened(observation: &AiObservation, site: &AiEntitySummary) -> bool {
    let reach = squared(SAFE_TILES * observation.map.tile_size as f32);
    observation.visible_enemies.iter().any(|enemy| {
        enemy.hp > 0
            && !enemy.kind.is_building()
            && enemy.kind != EntityKind::Worker
            && dist2(enemy.x, enemy.y, site.x, site.y) <= reach
    })
}

/// Sends a free Engineer back to the first unfinished site due (see the module docs); the kind sent
/// back to, if any.
pub(in crate::ai_core::decision) fn resume(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    profile_id: &str,
    builder_pools: &[&[u32]],
) -> Option<EntityKind> {
    if !is_jeff_v1_1(profile_id) || super::off("resume") {
        return None;
    }
    let tick = observation.tick;
    let mut sites: Vec<(&AiEntitySummary, (u32, u32))> = Vec::new();
    let mut unattended = BTreeMap::new();
    for site in observation
        .owned
        .iter()
        .filter(|site| resumable(site.kind) && !site.is_complete && site.hp > 0)
    {
        let Some(tile) = site_tile(observation, site) else {
            continue;
        };
        if attended(observation, site, tile) {
            continue;
        }
        let since = memory
            .jeff_v1_1
            .resume
            .unattended
            .get(&site.id)
            .copied()
            .unwrap_or(tick);
        unattended.insert(site.id, since);
        if tick.saturating_sub(since) >= UNATTENDED_TICKS && !threatened(observation, site) {
            sites.push((site, tile));
        }
    }
    memory.jeff_v1_1.resume.unattended = unattended;
    sites.sort_by_key(|(site, _)| (rank(site.kind), site.id));
    let (site, (tile_x, tile_y)) = sites.first().copied()?;
    actions::try_resume_construction_at(actions, builder_pools, site.kind, tile_x, tile_y)?;
    memory.jeff_v1_1.resume.unattended.remove(&site.id);
    memory.jeff_v1_1.resume.sent = Some((site.id, site.kind, tick));
    Some(site.kind)
}

/// The Engineers V1.1 wants: two while it has one and Steel piles up (`spareworker`).
pub(in crate::ai_core::decision) fn spare_engineer(
    profile_id: &str,
    observation: &AiObservation,
    workers: usize,
    target: usize,
) -> usize {
    if !is_jeff_v1_1(profile_id)
        || super::off("spareworker")
        || observation.tick < SPARE_FROM_TICK
        || workers != 1
        || observation.economy.steel < SPARE_STEEL
    {
        return target;
    }
    target.max(2)
}

pub(in crate::ai_core::decision) fn trace_line(memory: &AiDecisionMemory) -> Option<String> {
    let state = &memory.jeff_v1_1.resume;
    if state.unattended.is_empty() && state.sent.is_none() {
        return None;
    }
    Some(format!(
        "v1_1_resume unattended={:?} sent={:?}",
        state.unattended, state.sent
    ))
}
