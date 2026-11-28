//! Economic infantry and Tank defense, authored exclusively against the public, fog-filtered SDK.
use crate::sdk::{
    AiActions, AiBaseSiteId, AiCompletion, AiEntity, AiFrame, AiRulebook, AiStrategy,
    DefensePlanner, EntityKind, KnownBuildSiteExclusions, UpgradeKind, WorldQueries,
};
use std::collections::BTreeMap;
mod armor;
mod construction;
mod offense;
mod production;
mod recon;
mod support;
mod tactics;
#[cfg(test)]
mod tests;

fn base_target(frame: &AiFrame) -> usize {
    frame.base_sites().len() / 2 + 1
}
const SCREEN_LEAD_TICKS: u32 = rts_rules::balance::TICK_HZ * 5;
pub(crate) const ECO_AI_ID: &str = "eco_ai";

pub(crate) struct EcoStrategy {
    exclusions: KnownBuildSiteExclusions,
    last_build: Option<(EntityKind, u32, u32)>,
    failed_sites: usize,
    unattended: BTreeMap<u32, u32>,
    overflow_works: usize,
    planner: DefensePlanner,
    defense: tactics::DefenseAssignments,
    production: production::EcoProduction,
    offense: offense::EcoOffense,
    support: support::CommandSupport,
    recon: recon::Recon,
}
impl Default for EcoStrategy {
    fn default() -> Self {
        Self {
            exclusions: Default::default(),
            last_build: None,
            failed_sites: 0,
            unattended: Default::default(),
            overflow_works: 3,
            planner: DefensePlanner::with_firing_range_tiles(4.8),
            defense: Default::default(),
            production: Default::default(),
            offense: Default::default(),
            support: Default::default(),
            recon: Default::default(),
        }
    }
}
fn complete(e: &AiEntity) -> bool {
    e.completion == AiCompletion::Complete
}
fn count(frame: &AiFrame, kind: EntityKind) -> usize {
    frame.owned().iter().filter(|e| e.kind == kind).count()
}
fn barracks_target(bases: usize) -> usize {
    match bases {
        0 | 1 => 1,
        2 => 2,
        _ => 3,
    }
}

fn works_target(bases: usize) -> usize {
    match bases {
        0 | 1 => 0,
        2 => 1,
        3 => 2,
        _ => 3,
    }
}
fn next_structure(frame: &AiFrame, bases: usize, overflow_works: usize) -> Option<EntityKind> {
    if count(frame, EntityKind::Barracks) == 0 {
        Some(EntityKind::Barracks)
    } else if count(frame, EntityKind::TrainingCentre) == 0 {
        Some(EntityKind::TrainingCentre)
    } else if bases >= 2 && count(frame, EntityKind::EngineeringComplex) == 0 {
        Some(EntityKind::EngineeringComplex)
    } else if count(frame, EntityKind::Factory)
        < works_target(bases).max(if bases >= base_target(frame) {
            overflow_works
        } else {
            0
        })
    {
        Some(EntityKind::Factory)
    } else if count(frame, EntityKind::Barracks) < barracks_target(bases) {
        Some(EntityKind::Barracks)
    } else {
        None
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
                && q.known_base_depots(b.site.id).iter().all(|d| {
                    frame.players().iter().any(|p| {
                        p.id == d.owner
                            && p.id != frame.player_id()
                            && (p.team_id == 0 || p.team_id != frame.team_id())
                    })
                })
        })
        .map(|b| (b.site.id, b.site.position))
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
        self.defense.finish_completed_expansion(&sites);
        let required_structure = next_structure(frame, bases, self.overflow_works);
        // Plan expansion cover alongside the owned-base defenses while the Engineer is home.
        // The prospective site remains selected only through travel and construction.
        let expansion_tech = bases < 2
            || [UpgradeKind::Panzerfausts, UpgradeKind::Methamphetamines]
                .iter()
                .all(|u| frame.completed_upgrades().contains(u));
        let next_base = (bases < base_target(frame)
            && expansion_tech
            && frame
                .completed_upgrades()
                .contains(&UpgradeKind::Entrenchment))
        .then(|| expansion(frame))
        .flatten();
        let unfinished_base = frame
            .owned()
            .iter()
            .find(|e| e.kind == EntityKind::ResourceDepot && !complete(e))
            .and_then(|depot| {
                frame.base_sites().iter().min_by(|a, b| {
                    construction::dist2(a.position, depot.position)
                        .total_cmp(&construction::dist2(b.position, depot.position))
                })
            })
            .map(|site| site.id);
        let depot_travel = frame
            .submitted_builds()
            .iter()
            .any(|b| b.kind == EntityKind::ResourceDepot);
        if let Some(site) = unfinished_base {
            self.defense.prepare_expansion(site);
        } else if !depot_travel
            && !frame.owned().iter().any(|e| {
                e.kind == EntityKind::Worker && e.state == crate::sdk::AiEntityState::Build
            })
            && frame.submitted_builds().is_empty()
            && required_structure.is_none()
        {
            if let Some((site, _)) = next_base {
                self.defense.prepare_expansion(site);
            }
        }
        self.refresh_depot_reservation(frame, rules);
        if bases >= base_target(frame) {
            self.offense.observe(frame);
        }
        self.recon
            .step(frame, actions, self.defense.recon_target(frame));
        let push = self.defense.expansion_push_ids(frame);
        let mut reserved = self.defense.screen_ids().clone();
        reserved.extend(&push);
        let outbound = self.offense.controlled(frame, &reserved);
        // Give the support vehicle a turn before large infantry batches consume action capacity.
        let mut mobilized: std::collections::BTreeSet<u32> = self
            .defense
            .attacking_ids()
            .union(self.offense.attacking_ids())
            .copied()
            .collect();
        mobilized.extend(&push);
        if push.is_empty() {
            self.support.step(frame, actions, &mobilized);
        } else {
            self.support
                .step_mobilized(frame, actions, &mobilized, true);
        }
        let tactical = if outbound.is_empty() {
            self.defense.step(frame, actions, &mut self.planner, &sites)
        } else {
            self.defense
                .step_with_outbound(frame, actions, &mut self.planner, &sites, &outbound)
        };
        self.offense.step(frame, actions, &outbound, &tactical);
        // Configure standing orders before travel/construction can return from this decision.
        self.production.step(frame, actions);
        if !frame.owned().iter().any(|e| e.kind == EntityKind::Worker)
            || !frame.submitted_builds().is_empty()
        {
            return;
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
        let next = required_structure
            .map(|kind| (kind, home))
            .or_else(|| next_base.map(|(_, point)| (EntityKind::ResourceDepot, point)));
        let builder_next = next.filter(|(kind, _)| {
            *kind != EntityKind::ResourceDepot
                || next_base.is_some_and(|(site, _)| self.defense.ready_for_builder(frame, site))
        });
        if self.construct(frame, actions, rules, builder_next) {
            return;
        }
        // Add capacity after six bases when every existing Works is actively producing.
        let target = works_target(bases).max(if bases >= base_target(frame) {
            self.overflow_works
        } else {
            0
        });
        if bases >= base_target(frame)
            && count(frame, EntityKind::Factory) >= target
            && frame
                .owned()
                .iter()
                .filter(|e| e.kind == EntityKind::Factory)
                .all(|e| {
                    complete(e)
                        && e.production.is_some_and(|p| {
                            p.current_kind == Some(EntityKind::Tank)
                                && p.waiting_for_resources != Some(true)
                        })
                })
        {
            self.overflow_works = target.saturating_add(1);
        }
    }
}
