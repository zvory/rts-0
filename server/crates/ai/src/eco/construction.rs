use super::{complete, EcoStrategy};
use crate::sdk::KnownBuildSite;
use crate::sdk::{
    AiActions, AiCompletion, AiEntityState, AiFrame, AiRulebook, EntityKind, WorldQueries,
};
use rts_rules::math::GameplayMath as _;
use std::collections::BTreeSet;
impl EcoStrategy {
    /// Defense and construction share one actual intended footprint, including retry exclusions.
    pub(super) fn refresh_depot_reservation(&mut self, frame: &AiFrame, rules: AiRulebook) {
        let Some(frontier) = self.defense.frontier() else {
            return;
        };
        let q = WorldQueries::new(frame);
        let size = frame.map().tile_size as f32;
        let nearest_site = |point| {
            frame
                .base_sites()
                .iter()
                .min_by(|a, b| dist2(a.position, point).total_cmp(&dist2(b.position, point)))
                .map(|s| s.id)
        };
        let tile = frame
            .owned()
            .iter()
            .find(|e| {
                e.kind == EntityKind::ResourceDepot && nearest_site(e.position) == Some(frontier)
            })
            .and_then(|e| {
                let foot = rules.entity(e.kind)?.footprint?;
                q.tile(
                    (e.position.0 / size - foot.width_tiles as f32 / 2.)
                        .round()
                        .max(0.) as u32,
                    (e.position.1 / size - foot.height_tiles as f32 / 2.)
                        .round()
                        .max(0.) as u32,
                )
            })
            .or_else(|| {
                frame
                    .submitted_builds()
                    .iter()
                    .find(|b| b.kind == EntityKind::ResourceDepot)
                    .and_then(|b| q.tile(b.tile_x, b.tile_y))
            })
            .or_else(|| {
                self.last_build
                    .filter(|(kind, _, _)| *kind == EntityKind::ResourceDepot)
                    .and_then(|(_, x, y)| q.tile(x, y))
            })
            .or_else(|| q.base_depot_site(frontier, &self.exclusions));
        if let Some(tile) = tile {
            self.defense.reserve_depot(tile);
        }
    }

    /// Returns true after submitting the next manual build order.
    pub(super) fn construct(
        &mut self,
        frame: &AiFrame,
        actions: &mut AiActions,
        rules: AiRulebook,
        next: Option<(EntityKind, (f32, f32))>,
    ) -> bool {
        let Some(worker) = frame.owned().iter().find(|e| e.kind == EntityKind::Worker) else {
            return false;
        };
        let q = WorldQueries::new(frame);
        let size = frame.map().tile_size as f32;
        // Rush leaves an Engineer's active construction/travel order alone. In particular,
        // a paid scaffold does not need another Build command every decision.
        if (worker.state == AiEntityState::Build
            && frame.owned().iter().any(|e| {
                matches!(
                    e.kind,
                    EntityKind::Barracks
                        | EntityKind::TrainingCentre
                        | EntityKind::ResourceDepot
                        | EntityKind::EngineeringComplex
                        | EntityKind::Factory
                ) && matches!(
                    e.completion,
                    AiCompletion::UnderConstruction {
                        active: Some(true),
                        ..
                    }
                )
            }))
            || frame
                .submitted_builds()
                .iter()
                .any(|b| b.worker_id == worker.id)
        {
            return false;
        }
        // Copy Jeff's bounded, attended-site recovery pattern, using Eco's own roster.
        // Pump Jacks can also be built manually, so can_build alone cannot distinguish
        // an Engineer scaffold from a permanent Depot extractor job.
        let mut unattended = std::collections::BTreeMap::new();
        let mut abandoned = Vec::new();
        for building in frame.owned().iter().filter(|e| {
            matches!(
                e.kind,
                EntityKind::Barracks
                    | EntityKind::TrainingCentre
                    | EntityKind::ResourceDepot
                    | EntityKind::EngineeringComplex
                    | EntityKind::Factory
            ) && matches!(
                e.completion,
                AiCompletion::UnderConstruction {
                    active: Some(false),
                    ..
                }
            )
        }) {
            let since = self
                .unattended
                .get(&building.id)
                .copied()
                .unwrap_or(frame.tick());
            unattended.insert(building.id, since);
            let threatened = frame.visible_enemies().iter().any(|e| {
                e.health.current > 0
                    && !e.kind.is_building()
                    && e.kind != EntityKind::Worker
                    && dist2(e.position, building.position) <= (10. * size).det_powi(2)
            });
            if frame.tick().saturating_sub(since) >= 90 && !threatened {
                abandoned.push(building);
            }
        }
        self.unattended = unattended;
        abandoned.sort_by_key(|e| {
            (
                match e.kind {
                    EntityKind::TrainingCentre => 0,
                    EntityKind::Barracks => 1,
                    _ => 2,
                },
                e.id,
            )
        });
        if let Some(building) = abandoned.first() {
            if building.kind == EntityKind::ResourceDepot {
                let site = frame.base_sites().iter().min_by(|a, b| {
                    dist2(a.position, building.position)
                        .total_cmp(&dist2(b.position, building.position))
                });
                if !site.is_some_and(|site| self.defense.ready_for_builder(frame, site.id)) {
                    return false;
                }
            }
            if let Some(foot) = rules.entity(building.kind).and_then(|r| r.footprint) {
                let x = (building.position.0 / size - foot.width_tiles as f32 / 2.)
                    .round()
                    .max(0.) as u32;
                let y = (building.position.1 / size - foot.height_tiles as f32 / 2.)
                    .round()
                    .max(0.) as u32;
                if actions
                    .resume_build(&[worker.id], building.kind, x, y)
                    .is_ok()
                {
                    self.unattended.remove(&building.id);
                }
            }
            return false;
        }
        // Do not duplicate an existing paid scaffold while its recovery grace runs.
        if !self.unattended.is_empty() {
            return false;
        }
        if let Some((kind, x, y)) = self.last_build.take() {
            // A rejected or interrupted travel order must not monopolize one invalid site.
            let exists = frame.owned().iter().any(|e| {
                e.kind == kind
                    && rules
                        .entity(kind)
                        .and_then(|r| r.footprint)
                        .is_some_and(|f| {
                            (e.position.0 / size - (x as f32 + f.width_tiles as f32 / 2.)).abs()
                                < 0.1
                                && (e.position.1 / size - (y as f32 + f.height_tiles as f32 / 2.))
                                    .abs()
                                    < 0.1
                        })
            });
            if !exists {
                if let Some(tile) = q.tile(x, y) {
                    self.exclusions.exclude(kind, tile);
                    self.failed_sites += 1;
                    // Replan defensive posts around the next footprint before dispatching.
                    if kind == EntityKind::ResourceDepot {
                        return false;
                    }
                }
            }
            if self.failed_sites >= 64 {
                self.exclusions = Default::default();
                self.failed_sites = 0;
            }
        }
        let Some((kind, point)) = next else {
            return false;
        };
        let site = if kind == EntityKind::ResourceDepot {
            // The SDK owns the same resource-centered geometry as Pressure. General
            // outward building rings do not preserve a Depot's Steel/Oil coverage.
            frame
                .base_sites()
                .iter()
                .find(|site| site.position == point)
                .filter(|site| self.defense.frontier() == Some(site.id))
                .and_then(|_| self.defense.depot_tile())
                .filter(|tile| {
                    let allowed = q.planned_build_site_with_access(kind, *tile, &self.exclusions)
                        == KnownBuildSite::NoKnownConflict;
                    if !allowed {
                        // Resource geometry is recommended independently of this stricter
                        // layout policy. Retry another covered footprint rather than wait
                        // forever on a recommendation rejected by access preservation.
                        self.exclusions.exclude(kind, *tile);
                    }
                    allowed
                })
                .map(|tile| tile.as_tuple())
        } else {
            let mut anchors = vec![point];
            anchors.extend(
                frame
                    .owned()
                    .iter()
                    .filter(|e| e.kind == EntityKind::ResourceDepot && complete(e))
                    .map(|e| e.position)
                    .filter(|p| *p != point),
            );
            let skip = BTreeSet::new();
            anchors.into_iter().find_map(|anchor| {
                crate::ai_shared::find_build_spot_near_start_with(
                    frame.map().width,
                    frame.map().height,
                    ((anchor.0 / size) as u32, (anchor.1 / size) as u32),
                    kind,
                    crate::ai_shared::BuildSearch::default(),
                    &skip,
                    |x, y| {
                        q.tile(x, y).is_some_and(|tile| {
                            q.known_build_site_with_access(kind, tile, &self.exclusions)
                                == KnownBuildSite::NoKnownConflict
                        })
                    },
                )
            })
        };
        if let Some((x, y)) = site {
            if actions.queue_build(&[worker.id], kind, x, y).is_ok() {
                self.last_build = Some((kind, x, y));
                return true;
            }
        }
        false
    }
}
pub(super) fn dist2(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 - b.0) * (a.0 - b.0) + (a.1 - b.1) * (a.1 - b.1)
}
