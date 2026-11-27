use super::*;
use crate::sdk::KnownBuildSite;
use rts_rules::math::GameplayMath as _;
impl EcoStrategy {
    /// Returns true when this decision has reserved money for a not-yet-paid travel order.
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
        if worker.state == AiEntityState::Build
            && frame.owned().iter().any(|e| {
                rules.can_build(worker.kind, e.kind)
                    && matches!(
                        e.completion,
                        AiCompletion::UnderConstruction {
                            active: Some(true),
                            ..
                        }
                    )
            })
        {
            return false;
        }
        let q = WorldQueries::new(frame);
        let size = frame.map().tile_size as f32;
        if let Some(building) = frame.owned().iter().find(|e| {
            rules.can_build(worker.kind, e.kind)
                && matches!(
                    e.completion,
                    AiCompletion::UnderConstruction {
                        active: Some(false),
                        ..
                    }
                )
        }) {
            if let Some(foot) = rules.entity(building.kind).and_then(|r| r.footprint) {
                let x = (building.position.0 / size - foot.width_tiles as f32 / 2. + 0.5).max(0.)
                    as u32;
                let y = (building.position.1 / size - foot.height_tiles as f32 / 2. + 0.5).max(0.)
                    as u32;
                let _ = actions.resume_build(&[worker.id], building.kind, x, y);
            }
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
        let Some(cost) = rules.cost(kind) else {
            return false;
        };
        let budget = actions.remaining_budget();
        if budget.steel < cost.steel || budget.oil < cost.oil {
            return false;
        }
        let Some(foot) = rules.entity(kind).and_then(|r| r.footprint) else {
            return false;
        };
        let mut anchors = vec![point];
        if kind != EntityKind::ResourceDepot {
            anchors.extend(
                frame
                    .owned()
                    .iter()
                    .filter(|e| e.kind == EntityKind::ResourceDepot && complete(e))
                    .map(|e| e.position)
                    .filter(|p| *p != point),
            );
        }
        let mut seen = BTreeSet::new();
        let mut candidates = Vec::new();
        for anchor in anchors {
            let cx = (anchor.0 / size) as i32;
            let cy = (anchor.1 / size) as i32;
            for dy in -12i32..=12 {
                for dx in -12i32..=12 {
                    let x = cx + dx;
                    let y = cy + dy;
                    if x < 0 || y < 0 {
                        continue;
                    }
                    if !seen.insert((x, y)) {
                        continue;
                    }
                    let Some(tile) = q.tile(x as u32, y as u32) else {
                        continue;
                    };
                    let center = (
                        (x as f32 + foot.width_tiles as f32 / 2.) * size,
                        (y as f32 + foot.height_tiles as f32 / 2.) * size,
                    );
                    // Match the SDK's twelve-tile association bound as well as nearest landmark.
                    if kind == EntityKind::ResourceDepot
                        && dist2(center, point) > (12. * size).det_powi(2)
                    {
                        continue;
                    }
                    // Keep a Depot associated with its selected SDK landmark, not a nearby base.
                    if kind == EntityKind::ResourceDepot
                        && frame
                            .base_sites()
                            .iter()
                            .min_by(|a, b| {
                                dist2(a.position, center).total_cmp(&dist2(b.position, center))
                            })
                            .is_some_and(|s| s.position != point)
                    {
                        continue;
                    }
                    if q.known_build_site(kind, tile, &self.exclusions)
                        == KnownBuildSite::NoKnownConflict
                    {
                        candidates.push((
                            dist2(center, anchor) + dist2(center, worker.position) * 0.05,
                            tile,
                        ));
                    }
                }
            }
        }
        candidates.sort_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then_with(|| a.1.as_tuple().cmp(&b.1.as_tuple()))
        });
        if let Some((_, tile)) = candidates.first() {
            if actions
                .paid_build(&[worker.id], kind, tile.x(), tile.y())
                .is_ok()
            {
                self.last_build = Some((kind, tile.x(), tile.y()));
                return true;
            }
        }
        false
    }
}
pub(super) fn dist2(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 - b.0) * (a.0 - b.0) + (a.1 - b.1) * (a.1 - b.1)
}
