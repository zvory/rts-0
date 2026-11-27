//! Economic infantry defense, authored exclusively against the public, fog-filtered SDK.
use crate::sdk::{
    AiActions, AiBaseSiteId, AiCompletion, AiEntity, AiFrame, AiRulebook, AiStrategy,
    DefensePlanner, EntityKind, KnownBuildSiteExclusions, UpgradeKind, WorldQueries,
};
use rts_rules::math::GameplayMath as _;
use std::collections::BTreeSet;
mod construction;
mod tactics;
#[cfg(test)]
mod tests;

pub(crate) const ECO_AI_ID: &str = "eco_ai";

pub(crate) struct EcoStrategy {
    configured: BTreeSet<u32>,
    exclusions: KnownBuildSiteExclusions,
    last_build: Option<(EntityKind, u32, u32)>,
    failed_sites: usize,
    travel_progress: Option<(u32, f32)>,
    overflow_barracks: usize,
    planner: DefensePlanner,
    defense: tactics::DefenseAssignments,
}
impl Default for EcoStrategy {
    fn default() -> Self {
        Self {
            configured: BTreeSet::new(),
            exclusions: Default::default(),
            last_build: None,
            failed_sites: 0,
            travel_progress: None,
            overflow_barracks: 4,
            planner: DefensePlanner::with_firing_range_tiles(4.8),
            defense: Default::default(),
        }
    }
}
fn complete(e: &AiEntity) -> bool {
    e.completion == AiCompletion::Complete
}
fn idle_producer(e: &AiEntity) -> bool {
    complete(e) && e.production.is_none_or(|p| p.queue_len == Some(0))
}
fn count(frame: &AiFrame, kind: EntityKind) -> usize {
    frame.owned().iter().filter(|e| e.kind == kind).count()
}
fn barracks_target(bases: usize) -> usize {
    match bases {
        0 | 1 => 1,
        2 => 2,
        _ => 4,
    }
}

/// Completed own Depots only: geographic landmarks and scaffolds do not unlock stages.
fn owned_sites(frame: &AiFrame) -> Vec<AiBaseSiteId> {
    let q = WorldQueries::new(frame);
    q.ranked_bases(frame.player_id())
        .into_iter()
        .filter(|b| {
            q.known_base_depots(b.site.id).iter().any(|d| {
                d.owner == frame.player_id()
                    && frame
                        .owned()
                        .iter()
                        .any(|e| e.id == d.entity_id && complete(e))
            })
        })
        .map(|b| b.site.id)
        .collect()
}
fn expansion(frame: &AiFrame) -> Option<(AiBaseSiteId, (f32, f32))> {
    let q = WorldQueries::new(frame);
    q.ranked_bases(frame.player_id())
        .into_iter()
        .find(|b| {
            b.distance_from_main.is_some()
                && !matches!(b.site.id,AiBaseSiteId::Start(id) if id!=frame.player_id())
                && q.known_base_depots(b.site.id).is_empty()
        })
        .map(|b| (b.site.id, b.site.position))
}
fn desired_research(frame: &AiFrame, bases: usize) -> Option<UpgradeKind> {
    [
        UpgradeKind::Entrenchment,
        UpgradeKind::Panzerfausts,
        UpgradeKind::Methamphetamines,
    ]
    .into_iter()
    .filter(|u| *u == UpgradeKind::Entrenchment || bases >= 2)
    .find(|u| !frame.completed_upgrades().contains(u))
}
fn infantry_counts(frame: &AiFrame) -> (usize, usize) {
    let mut mg = count(frame, EntityKind::MachineGunner);
    let mut pf = count(frame, EntityKind::Panzerfaust);
    // Only current_kind is observable. Eco never creates a hidden second queue item.
    for e in frame
        .owned()
        .iter()
        .filter(|e| e.kind == EntityKind::Barracks)
    {
        match e.production.and_then(|p| p.current_kind) {
            Some(EntityKind::MachineGunner) => mg += 1,
            Some(EntityKind::Panzerfaust) => pf += 1,
            _ => {}
        }
    }
    (mg, pf)
}
impl AiStrategy for EcoStrategy {
    fn step(&mut self, frame: &AiFrame, actions: &mut AiActions) {
        let Some(rules) = AiRulebook::for_frame(frame) else {
            return;
        };
        // This strategy deliberately requests the Kriegsia infantry/economy roster. Other
        // faction catalogs fail closed instead of receiving incompatible commands.
        if !rules.is_available(EntityKind::MachineGunner)
            || !rules.is_available(EntityKind::SteelMine)
        {
            return;
        }
        let sites = owned_sites(frame);
        let bases = sites.len();
        self.defense.step(frame, actions, &mut self.planner, &sites);
        for depot in frame
            .owned()
            .iter()
            .filter(|e| e.kind == EntityKind::ResourceDepot && complete(e))
        {
            if frame.owned().iter().any(|e| e.kind == EntityKind::Worker)
                && !self.configured.contains(&depot.id)
            {
                let steel = actions.set_production_repeat(&[depot.id], EntityKind::SteelMine, true);
                let oil = actions.set_production_repeat(&[depot.id], EntityKind::PumpJack, true);
                if steel.is_ok() && oil.is_ok() {
                    self.configured.insert(depot.id);
                }
            }
        }
        if !frame.owned().iter().any(|e| e.kind == EntityKind::Worker) {
            // Finish the current extractor, then free its producer for a replacement builder.
            for d in frame
                .owned()
                .iter()
                .filter(|e| e.kind == EntityKind::ResourceDepot && complete(e))
            {
                if self.configured.remove(&d.id) {
                    let _ = actions.set_production_repeat(&[d.id], EntityKind::SteelMine, false);
                    let _ = actions.set_production_repeat(&[d.id], EntityKind::PumpJack, false);
                }
            }
            if !frame.owned().iter().any(|e| {
                e.production
                    .is_some_and(|p| p.current_kind == Some(EntityKind::Worker))
            }) {
                if let Some(d) = frame
                    .owned()
                    .iter()
                    .find(|e| e.kind == EntityKind::ResourceDepot && idle_producer(e))
                {
                    let _ = actions.train(&[d.id], EntityKind::Worker);
                }
            }
            return;
        }
        let unpaid_travel = frame.submitted_builds().iter().find(|intent| {
            rules
                .entity(intent.kind)
                .and_then(|r| r.footprint)
                .is_some_and(|f| {
                    let center = (
                        (intent.tile_x as f32 + f.width_tiles as f32 / 2.)
                            * frame.map().tile_size as f32,
                        (intent.tile_y as f32 + f.height_tiles as f32 / 2.)
                            * frame.map().tile_size as f32,
                    );
                    !frame.owned().iter().any(|e| {
                        e.kind == intent.kind && construction::dist2(e.position, center) < 1.
                    })
                })
        });
        // Travel orders have not paid yet. Leave the bank intact until the scaffold exists.
        // The host expires stationary orders; also interrupt moving detours with no net
        // approach progress for twenty seconds, rather than preserving an endless orbit.
        if let Some(intent) = unpaid_travel {
            if let Some(worker) = frame.owned().iter().find(|e| e.id == intent.worker_id) {
                let target = (
                    (intent.tile_x as f32 + 0.5) * frame.map().tile_size as f32,
                    (intent.tile_y as f32 + 0.5) * frame.map().tile_size as f32,
                );
                let distance = construction::dist2(worker.position, target);
                let (since, best) = self.travel_progress.get_or_insert((frame.tick(), distance));
                if distance + (frame.map().tile_size as f32).det_powi(2) < *best {
                    *since = frame.tick();
                    *best = distance;
                } else if frame.tick().saturating_sub(*since) >= 600 {
                    if let Ok(group) = crate::sdk::UnitGroup::new([worker.id]) {
                        let _ = actions.hold_position(&group, false);
                    }
                    self.travel_progress = None;
                }
            }
            return;
        }
        self.travel_progress = None;
        let desired = desired_research(frame, bases);
        let mut saving_research = false;
        if let Some(upgrade) = desired {
            let running = frame.owned().iter().any(|e| {
                e.production
                    .is_some_and(|p| p.current_upgrade == Some(upgrade))
            });
            if !running {
                if let Some(centre) = frame
                    .owned()
                    .iter()
                    .find(|e| e.kind == EntityKind::TrainingCentre && idle_producer(e))
                {
                    saving_research = actions.research(&[centre.id], upgrade).is_err();
                }
            }
        }
        let home = frame
            .owned()
            .iter()
            .find(|e| e.kind == EntityKind::ResourceDepot && complete(e))
            .map(|e| e.position)
            .or_else(|| {
                frame
                    .players()
                    .iter()
                    .find(|p| p.id == frame.player_id())
                    .map(|p| {
                        (
                            (p.start_tile.0 as f32 + 0.5) * frame.map().tile_size as f32,
                            (p.start_tile.1 as f32 + 0.5) * frame.map().tile_size as f32,
                        )
                    })
            });
        let Some(home) = home else { return };
        let target = barracks_target(bases).max(if bases >= 4 {
            self.overflow_barracks
        } else {
            0
        });
        let second_base_tech_complete = bases < 2
            || [UpgradeKind::Panzerfausts, UpgradeKind::Methamphetamines]
                .iter()
                .all(|u| frame.completed_upgrades().contains(u));
        let next_base = if bases < 4
            && second_base_tech_complete
            && frame
                .completed_upgrades()
                .contains(&UpgradeKind::Entrenchment)
        {
            expansion(frame)
        } else {
            None
        };
        let next = if count(frame, EntityKind::Barracks) < target {
            Some((EntityKind::Barracks, home))
        } else if count(frame, EntityKind::TrainingCentre) == 0 {
            Some((EntityKind::TrainingCentre, home))
        } else if !saving_research {
            next_base.map(|(_, p)| (EntityKind::ResourceDepot, p))
        } else {
            None
        };
        if self.construct(frame, actions, rules, next) {
            return;
        }
        // Keep the next structure's complete cost in the bank, including when placement is
        // temporarily blocked. Surplus can still sustain defense while the builder retries.
        let reserve = next
            .and_then(|(kind, _)| rules.cost(kind))
            .unwrap_or_default();
        if saving_research {
            return;
        }
        let (mut mg, mut pf) = infantry_counts(frame);
        let mixed = bases >= 2
            && frame
                .completed_upgrades()
                .contains(&UpgradeKind::Panzerfausts);
        for b in frame
            .owned()
            .iter()
            .filter(|e| e.kind == EntityKind::Barracks && idle_producer(e))
        {
            let kind = if mixed && pf < mg {
                EntityKind::Panzerfaust
            } else {
                EntityKind::MachineGunner
            };
            let Some(cost) = rules.cost(kind) else {
                continue;
            };
            let budget = actions.remaining_budget();
            if budget.steel.saturating_sub(reserve.steel) < cost.steel
                || budget.oil.saturating_sub(reserve.oil) < cost.oil
            {
                continue;
            }
            if actions.train(&[b.id], kind).is_ok() {
                if kind == EntityKind::MachineGunner {
                    mg += 1
                } else {
                    pf += 1
                }
            }
        }
        // Scale production only once the four-base economy floats another complete Barracks.
        // Desired capacity persists so destroyed overflow producers are rebuilt.
        if bases >= 4
            && count(frame, EntityKind::Barracks) >= target
            && target < 16
            && frame.economy().steel >= 750
            && frame.economy().oil >= 135
            && frame
                .owned()
                .iter()
                .filter(|e| e.kind == EntityKind::Barracks)
                .all(|e| !idle_producer(e))
        {
            self.overflow_barracks = target + 1;
        }
    }
}
