//! Standing production and manual research use the same server scheduler as player orders.
use crate::sdk::{AiActions, AiFrame, EntityKind, UpgradeKind};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(super) struct EcoProduction {
    research_queued: BTreeSet<(u32, UpgradeKind)>,
    configured: BTreeMap<(u32, EntityKind), bool>,
    worker_order: Option<u32>,
}
impl EcoProduction {
    pub(super) fn step(&mut self, frame: &AiFrame, actions: &mut AiActions) {
        let owned = |id: &u32| frame.owned().iter().any(|e| e.id == *id);
        self.research_queued.retain(|(id, _)| owned(id));
        self.configured.retain(|(id, _), _| owned(id));
        let bases = super::owned_sites(frame).len();
        let research = [
            (UpgradeKind::Entrenchment, EntityKind::TrainingCentre, true),
            (
                UpgradeKind::Panzerfausts,
                EntityKind::TrainingCentre,
                bases >= 2,
            ),
            (
                UpgradeKind::Methamphetamines,
                EntityKind::TrainingCentre,
                bases >= 2,
            ),
            (
                UpgradeKind::TankUnlock,
                EntityKind::EngineeringComplex,
                bases >= 2,
            ),
        ];
        for (upgrade, kind, enabled) in research {
            if !enabled
                || frame.completed_upgrades().contains(&upgrade)
                || self
                    .research_queued
                    .iter()
                    .any(|(_, queued)| *queued == upgrade)
            {
                continue;
            }
            if let Some(building) = frame.owned().iter().find(|e| e.kind == kind) {
                if actions.queue_research(&[building.id], upgrade).is_ok() {
                    self.research_queued.insert((building.id, upgrade));
                }
            }
        }
        for (index, barracks) in frame
            .owned()
            .iter()
            .filter(|e| e.kind == EntityKind::Barracks)
            .enumerate()
        {
            // Separate standing lines keep MG production independent of a PF's Oil wait.
            let panzerfaust = frame
                .completed_upgrades()
                .contains(&UpgradeKind::Panzerfausts)
                && !index.is_multiple_of(2);
            self.repeat(actions, barracks.id, EntityKind::Rifleman, false);
            self.repeat(
                actions,
                barracks.id,
                EntityKind::MachineGunner,
                !panzerfaust,
            );
            self.repeat(actions, barracks.id, EntityKind::Panzerfaust, panzerfaust);
        }
        for works in frame
            .owned()
            .iter()
            .filter(|e| e.kind == EntityKind::Factory)
        {
            self.repeat(actions, works.id, EntityKind::Tank, true);
        }
        if frame.owned().iter().any(|e| e.kind == EntityKind::Worker) {
            self.worker_order = None;
        } else {
            if self.worker_order.is_some_and(|id| !owned(&id)) {
                self.worker_order = None;
            }
            if self.worker_order.is_none()
                && !frame.owned().iter().any(|e| {
                    e.production
                        .is_some_and(|p| p.current_kind == Some(EntityKind::Worker))
                })
            {
                if let Some(depot) = frame
                    .owned()
                    .iter()
                    .find(|e| e.kind == EntityKind::ResourceDepot)
                {
                    if actions.queue_train(&[depot.id], EntityKind::Worker).is_ok() {
                        self.worker_order = Some(depot.id);
                    }
                }
            }
        }
    }
    fn repeat(&mut self, actions: &mut AiActions, building: u32, kind: EntityKind, enabled: bool) {
        if self.configured.get(&(building, kind)) != Some(&enabled)
            && actions
                .set_production_repeat(&[building], kind, enabled)
                .is_ok()
        {
            self.configured.insert((building, kind), enabled);
        }
    }
}
