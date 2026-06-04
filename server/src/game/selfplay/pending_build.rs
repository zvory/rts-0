use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::player_view::{building_footprint_tiles, is_kind, PlayerView};
use crate::config;
use crate::game::ai_core::observation::AiBuildIntent;
use crate::game::command::SimCommand as Command;
use crate::game::entity::EntityKind;
use crate::protocol::{states, EntityView};

const FAILED_SPOTS_CAP: usize = 16;
/// Force a pending build to be treated as failed after this many ticks without worker movement so
/// stale commands do not suppress future build attempts forever if a worker gets stuck.
pub(super) const PENDING_BUILD_STALE_TICKS: u32 = 300;
const PENDING_BUILD_PROGRESS_EPS_PX: f32 = 4.0;

#[derive(Clone, Copy)]
struct PendingBuild {
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
    last_x: Option<f32>,
    last_y: Option<f32>,
    last_progress_tick: u32,
}

impl PendingBuild {
    fn observe_worker(&mut self, worker: &EntityView, tick: u32) {
        let Some(last_x) = self.last_x else {
            self.last_x = Some(worker.x);
            self.last_y = Some(worker.y);
            self.last_progress_tick = tick;
            return;
        };
        let last_y = self.last_y.unwrap_or(worker.y);
        let dx = worker.x - last_x;
        let dy = worker.y - last_y;
        if dx * dx + dy * dy >= PENDING_BUILD_PROGRESS_EPS_PX * PENDING_BUILD_PROGRESS_EPS_PX {
            self.last_x = Some(worker.x);
            self.last_y = Some(worker.y);
            self.last_progress_tick = tick;
        }
    }

    fn stale_at(self, tick: u32) -> bool {
        tick.saturating_sub(self.last_progress_tick) >= PENDING_BUILD_STALE_TICKS
    }
}

#[derive(Default)]
pub(super) struct PendingBuildTracker {
    pending: BTreeMap<u32, PendingBuild>,
    failed_spots: HashMap<EntityKind, BTreeSet<(u32, u32)>>,
}

impl PendingBuildTracker {
    pub(super) fn observe(&mut self, view: PlayerView<'_>) {
        let own: Vec<&EntityView> = view
            .snapshot
            .entities
            .iter()
            .filter(|e| e.owner == view.player_id)
            .collect();
        let workers: Vec<&EntityView> = own
            .iter()
            .copied()
            .filter(|e| is_kind(e, EntityKind::Worker))
            .collect();
        let mut dropped = Vec::new();
        self.pending.retain(|worker_id, pending| {
            let worker = workers
                .iter()
                .copied()
                .find(|w| w.id == *worker_id && w.state == states::BUILD);
            let keep = worker
                .map(|worker| {
                    pending.observe_worker(worker, view.tick);
                    !pending.stale_at(view.tick)
                })
                .unwrap_or(false);
            if !keep {
                dropped.push(*pending);
            }
            keep
        });
        for pending in dropped {
            let succeeded = own.iter().any(|e| {
                is_kind(e, pending.kind)
                    && building_footprint_tiles(&view.start.map, e)
                        .contains(&(pending.tile_x, pending.tile_y))
            });
            if succeeded {
                self.failed_spots.remove(&pending.kind);
            } else {
                let set = self.failed_spots.entry(pending.kind).or_default();
                set.insert((pending.tile_x, pending.tile_y));
                if set.len() > FAILED_SPOTS_CAP {
                    set.clear();
                }
            }
        }
    }

    pub(super) fn intents(&self) -> Vec<AiBuildIntent> {
        self.pending
            .iter()
            .map(|(worker_id, pending)| {
                AiBuildIntent::to_site(*worker_id, pending.kind, pending.tile_x, pending.tile_y)
            })
            .collect()
    }

    pub(super) fn record_commands(&mut self, tick: u32, commands: &[Command]) {
        for command in commands {
            let Command::Build {
                worker,
                building,
                tile_x,
                tile_y,
            } = command
            else {
                continue;
            };
            if config::building_stats(*building).is_none() {
                continue;
            }
            self.pending.insert(
                *worker,
                PendingBuild {
                    kind: *building,
                    tile_x: *tile_x,
                    tile_y: *tile_y,
                    last_x: None,
                    last_y: None,
                    last_progress_tick: tick,
                },
            );
        }
    }

    pub(super) fn failed(&self, kind: EntityKind, tile_x: u32, tile_y: u32) -> bool {
        self.failed_spots
            .get(&kind)
            .map(|spots| spots.contains(&(tile_x, tile_y)))
            .unwrap_or(false)
    }
}
