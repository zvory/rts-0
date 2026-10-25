//! Forward construction rush authored only against the fog-filtered strategy SDK.
use std::collections::{BTreeMap, BTreeSet};

use crate::sdk::{
    AiActions, AiCompletion, AiEntity, AiEntityState, AiFrame, AiStrategy, EntityKind,
    KnownBuildSite, KnownBuildSiteExclusions, UnitGroup, UpgradeKind, WorldQueries,
};
use rts_sim::game::entity::RallyKind;

mod staging;

pub(crate) const RUSH_AI_ID: &str = "rush_ai";
const PANZERFAUST_RESEARCH_TICK: u32 = rts_rules::balance::TICK_HZ * 120;
const ENGINEER_MIN_DISTANCE_TILES: f32 = 30.0;
// Leave room for travel between decisions and the approach to a construction footprint.
const ENGINEER_STANDOFF_TILES: f32 = ENGINEER_MIN_DISTANCE_TILES + 2.0;

#[derive(Default)]
pub(crate) struct RushStrategy {
    configured: BTreeSet<u32>,
    waves: staging::RushWaves,
    barracks_repeats: BTreeMap<u32, BTreeSet<EntityKind>>,
    research_queued: BTreeSet<(u32, UpgradeKind)>,
    riflemen_retired: bool,
    exclusions: KnownBuildSiteExclusions,
    last_build: Option<(EntityKind, u32, u32)>,
}

impl AiStrategy for RushStrategy {
    fn step(&mut self, frame: &AiFrame, actions: &mut AiActions) {
        let Some(enemy) = frame
            .players()
            .iter()
            .find(|p| p.team_id != frame.team_id() && p.is_alive)
        else {
            return;
        };
        let size = frame.map().tile_size as f32;
        let target = (
            (enemy.start_tile.0 as f32 + 0.5) * size,
            (enemy.start_tile.1 as f32 + 0.5) * size,
        );
        let queries = WorldQueries::new(frame);

        let rally = self.waves.rally(frame, enemy.id, target);
        self.riflemen_retired |= frame.owned().iter().any(|entity| {
            entity.kind == EntityKind::TrainingCentre && entity.completion == AiCompletion::Complete
        });
        let panzerfausts_ready = frame
            .completed_upgrades()
            .contains(&UpgradeKind::Panzerfausts);
        for entity in frame.owned() {
            if entity.kind == EntityKind::ResourceDepot && !self.configured.contains(&entity.id) {
                let steel =
                    actions.set_production_repeat(&[entity.id], EntityKind::SteelMine, true);
                let oil = actions.set_production_repeat(&[entity.id], EntityKind::PumpJack, true);
                if steel.is_ok() && oil.is_ok() {
                    self.configured.insert(entity.id);
                }
            }
            if entity.kind == EntityKind::Barracks && entity.completion == AiCompletion::Complete {
                if let Some(rally) = rally.filter(|_| !self.configured.contains(&entity.id)) {
                    if actions
                        .set_rally(entity.id, rally.0, rally.1, RallyKind::Move, false)
                        .is_ok()
                    {
                        self.configured.insert(entity.id);
                    }
                }
                let repeats = self.barracks_repeats.entry(entity.id).or_default();
                for (kind, enabled) in [
                    (EntityKind::Rifleman, !self.riflemen_retired),
                    (EntityKind::MachineGunner, true),
                    (EntityKind::Panzerfaust, panzerfausts_ready),
                ] {
                    if repeats.contains(&kind) != enabled
                        && actions
                            .set_production_repeat(&[entity.id], kind, enabled)
                            .is_ok()
                    {
                        if enabled {
                            repeats.insert(kind);
                        } else {
                            repeats.remove(&kind);
                        }
                    }
                }
            }
            if entity.kind == EntityKind::TrainingCentre {
                for upgrade in [
                    UpgradeKind::Methamphetamines,
                    UpgradeKind::Entrenchment,
                    UpgradeKind::Panzerfausts,
                ] {
                    if upgrade == UpgradeKind::Panzerfausts
                        && frame.tick() < PANZERFAUST_RESEARCH_TICK
                    {
                        break;
                    }
                    if !frame.completed_upgrades().contains(&upgrade)
                        && !self.research_queued.contains(&(entity.id, upgrade))
                    {
                        // Queue in order, including unpaid research on the scaffold. One new item
                        // per decision preserves the SDK's producer reservation rule.
                        if actions.queue_research(&[entity.id], upgrade).is_ok() {
                            self.research_queued.insert((entity.id, upgrade));
                        }
                        break;
                    }
                }
            }
        }

        self.waves.step(frame, target, actions);

        let Some(worker) = frame.owned().iter().find(|e| e.kind == EntityKind::Worker) else {
            return;
        };
        let distance = (worker.position.0 - target.0).hypot(worker.position.1 - target.1);
        if distance < (ENGINEER_MIN_DISTANCE_TILES + 1.0) * size {
            // Correct a pathing detour before the next decision could enter the forbidden radius.
            if let Ok(group) = UnitGroup::new([worker.id]) {
                if let Some(point) = standoff_destination(frame, worker.position, target) {
                    let _ = actions.move_group(&group, point.0, point.1, false);
                } else {
                    let _ = actions.hold_position(&group, false);
                }
            }
            return;
        }
        if worker.state == AiEntityState::Build
            || frame
                .submitted_builds()
                .iter()
                .any(|b| b.worker_id == worker.id)
        {
            return;
        }
        if let Some(building) = frame.owned().iter().find(|e| {
            matches!(e.kind, EntityKind::Barracks | EntityKind::TrainingCentre)
                && matches!(e.completion, AiCompletion::UnderConstruction { .. })
        }) {
            if let Some(stats) = rts_rules::balance::building_stats(building.kind) {
                let x = (building.position.0 / size - stats.foot_w as f32 / 2.0)
                    .round()
                    .max(0.0) as u32;
                let y = (building.position.1 / size - stats.foot_h as f32 / 2.0)
                    .round()
                    .max(0.0) as u32;
                if safe_construction_site(worker.position, building.kind, x, y, target, size) {
                    let _ = actions.resume_build(&[worker.id], building.kind, x, y);
                }
            }
            return;
        }
        // If a submitted site failed validation, don't keep picking that same footprint.
        if let Some((kind, x, y)) = self.last_build.take() {
            if let Some(tile) = queries.tile(x, y) {
                self.exclusions.exclude(kind, tile);
            }
        }
        let barracks = frame
            .owned()
            .iter()
            .filter(|e| e.kind == EntityKind::Barracks)
            .count();
        let training = frame
            .owned()
            .iter()
            .any(|e| e.kind == EntityKind::TrainingCentre);
        let next = if barracks == 0 {
            Some(EntityKind::Barracks)
        } else if !training {
            Some(EntityKind::TrainingCentre)
        } else if barracks < 3 {
            Some(EntityKind::Barracks)
        } else {
            None
        };
        if let Some(kind) = next {
            let (steel, oil) = rts_rules::economy::cost(kind);
            if actions.remaining_budget().steel >= steel && actions.remaining_budget().oil >= oil {
                if let Some((x, y)) =
                    forward_site(&queries, worker, kind, target, size, &self.exclusions)
                {
                    if actions.paid_build(&[worker.id], kind, x, y).is_ok() {
                        self.last_build = Some((kind, x, y));
                        return;
                    }
                }
            }
        }
        if worker.state != AiEntityState::Move && distance > ENGINEER_STANDOFF_TILES * size + 8.0 {
            if let (Ok(group), Some(point)) = (
                UnitGroup::new([worker.id]),
                standoff_destination(frame, worker.position, target),
            ) {
                let _ = actions.move_group(&group, point.0, point.1, false);
            }
        }
    }
}

fn forward_site(
    queries: &WorldQueries<'_>,
    worker: &AiEntity,
    kind: EntityKind,
    target: (f32, f32),
    size: f32,
    exclusions: &KnownBuildSiteExclusions,
) -> Option<(u32, u32)> {
    let stats = rts_rules::balance::building_stats(kind)?;
    let dx = target.0 - worker.position.0;
    let dy = target.1 - worker.position.1;
    let length = dx.hypot(dy).max(1.0);
    let bearing = (dx / length, dy / length);
    let tile_x = (worker.position.0 / size) as i32;
    let tile_y = (worker.position.1 / size) as i32;
    let mut candidates = Vec::new();
    for y in tile_y - 7..=tile_y + 7 {
        for x in tile_x - 7..=tile_x + 7 {
            let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
                continue;
            };
            let center = (
                (x as f32 + stats.foot_w as f32 / 2.0) * size,
                (y as f32 + stats.foot_h as f32 / 2.0) * size,
            );
            let offset = (center.0 - worker.position.0, center.1 - worker.position.1);
            let forward = offset.0 * bearing.0 + offset.1 * bearing.1;
            // At the standoff line, finish the build order beside or behind the Engineer.
            if forward < size && length > (ENGINEER_STANDOFF_TILES + 8.0) * size {
                continue;
            }
            if !safe_construction_site(worker.position, kind, x, y, target, size) {
                continue;
            }
            let desired = (bearing.0 * size * 4.0, bearing.1 * size * 4.0);
            let score = (offset.0 - desired.0).powi(2) + (offset.1 - desired.1).powi(2);
            candidates.push((score, x, y));
        }
    }
    candidates.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| (a.1, a.2).cmp(&(b.1, b.2)))
    });
    candidates.into_iter().find_map(|(_, x, y)| {
        let tile = queries.tile(x, y)?;
        (queries.known_build_site(kind, tile, exclusions) == KnownBuildSite::NoKnownConflict)
            .then_some((x, y))
    })
}

fn standoff_destination(
    frame: &AiFrame,
    position: (f32, f32),
    target: (f32, f32),
) -> Option<(f32, f32)> {
    let size = frame.map().tile_size as f32;
    let angle = (position.1 - target.1).atan2(position.0 - target.0);
    let radius = ENGINEER_STANDOFF_TILES * size;
    let queries = WorldQueries::new(frame);
    // Prefer the current bearing, falling back along the same standoff circle if its tile is blocked.
    for offset in 0..=32 {
        let step = ((offset + 1) / 2) as f32 * std::f32::consts::PI / 16.0;
        let direction = angle + if offset % 2 == 0 { step } else { -step };
        let point = (
            target.0 + direction.cos() * radius,
            target.1 + direction.sin() * radius,
        );
        let Some(world) = queries.world_point(point.0, point.1) else {
            continue;
        };
        let Some(tile) = queries.world_to_tile(world) else {
            continue;
        };
        let index = tile.y() as usize * frame.map().width as usize + tile.x() as usize;
        if frame.map().terrain.get(index).is_some_and(|terrain| {
            !matches!(
                terrain,
                crate::sdk::AiTerrain::Rock
                    | crate::sdk::AiTerrain::Water
                    | crate::sdk::AiTerrain::Unknown(_)
            )
        }) {
            return Some(point);
        }
    }
    None
}

fn safe_construction_site(
    position: (f32, f32),
    kind: EntityKind,
    x: u32,
    y: u32,
    target: (f32, f32),
    size: f32,
) -> bool {
    let Some(stats) = rts_rules::balance::building_stats(kind) else {
        return false;
    };
    let left = x as f32 * size;
    let top = y as f32 * size;
    let right = left + stats.foot_w as f32 * size;
    let bottom = top + stats.foot_h as f32 * size;
    let closest = (target.0.clamp(left, right), target.1.clamp(top, bottom));
    let radius = ENGINEER_STANDOFF_TILES * size;
    if (closest.0 - target.0).hypot(closest.1 - target.1) < radius {
        return false;
    }
    // A segment between two safe endpoints can still cut across the enemy's exclusion circle.
    let center = ((left + right) / 2.0, (top + bottom) / 2.0);
    let segment = (center.0 - position.0, center.1 - position.1);
    let length2 = segment.0 * segment.0 + segment.1 * segment.1;
    let along = if length2 > 0.0 {
        (((target.0 - position.0) * segment.0 + (target.1 - position.1) * segment.1) / length2)
            .clamp(0.0, 1.0)
    } else {
        0.0
    };
    (position.0 + segment.0 * along - target.0).hypot(position.1 + segment.1 * along - target.1)
        >= (ENGINEER_MIN_DISTANCE_TILES + 1.0) * size
}
