//! Pressure's production-gated infantry research and standing Panzerfaust production.
use std::collections::BTreeSet;

use crate::sdk::{AiActions, AiCompletion, AiFrame, EntityKind, UpgradeKind};

// Baseline calibration: nine newly trained Riflemen on most map/side samples at two minutes.
pub(crate) const PANZERFAUST_RIFLEMEN_TRAINED: u32 = 9;

pub(crate) fn uses_panzerfaust_production(upgrades: &[UpgradeKind]) -> bool {
    upgrades.contains(&UpgradeKind::Panzerfausts)
}

#[derive(Default)]
pub(super) struct PressureProduction {
    research_queued: BTreeSet<u32>,
    configured_barracks: BTreeSet<u32>,
}

impl PressureProduction {
    pub(super) fn step(&mut self, frame: &AiFrame, trained_riflemen: u32, actions: &mut AiActions) {
        self.research_queued
            .retain(|id| frame.owned().iter().any(|e| e.id == *id));
        self.configured_barracks
            .retain(|id| frame.owned().iter().any(|e| e.id == *id));
        if trained_riflemen < PANZERFAUST_RIFLEMEN_TRAINED
            && !frame
                .completed_upgrades()
                .contains(&UpgradeKind::Panzerfausts)
        {
            return;
        }
        if !frame
            .completed_upgrades()
            .contains(&UpgradeKind::Panzerfausts)
        {
            // One queue on one Training Centre. If it dies, queue on a surviving/rebuilt one.
            if self.research_queued.is_empty() {
                if let Some(centre) = frame
                    .owned()
                    .iter()
                    .find(|e| e.kind == EntityKind::TrainingCentre)
                {
                    if actions
                        .queue_research(&[centre.id], UpgradeKind::Panzerfausts)
                        .is_ok()
                    {
                        self.research_queued.insert(centre.id);
                    }
                }
            }
            return;
        }
        for barracks in frame
            .owned()
            .iter()
            .filter(|e| e.kind == EntityKind::Barracks && e.completion == AiCompletion::Complete)
        {
            if self.configured_barracks.contains(&barracks.id) {
                continue;
            }
            // Disable future Riflemen without cancelling paid/queued infantry.
            if actions
                .set_production_repeat(&[barracks.id], EntityKind::Rifleman, false)
                .is_ok()
                && actions
                    .set_production_repeat(&[barracks.id], EntityKind::Panzerfaust, true)
                    .is_ok()
            {
                self.configured_barracks.insert(barracks.id);
            }
        }
    }
}
