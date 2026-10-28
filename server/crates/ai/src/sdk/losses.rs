use std::collections::BTreeMap;

use super::EntityKind;

/// Cumulative destroyed units and buildings, matching the Losses tab's kind counts and
/// nominal Steel/Oil values. Cancellation, refunds, and administrative removal are excluded.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AiLosses {
    pub units: BTreeMap<EntityKind, u32>,
    pub buildings: BTreeMap<EntityKind, u32>,
    pub steel_value: u32,
    pub oil_value: u32,
}

impl AiLosses {
    pub fn units_lost(&self) -> u32 {
        self.units
            .values()
            .fold(0u32, |total, count| total.saturating_add(*count))
    }

    pub fn buildings_lost(&self) -> u32 {
        self.buildings
            .values()
            .fold(0u32, |total, count| total.saturating_add(*count))
    }

    pub(crate) fn from_counts(counts: &BTreeMap<EntityKind, u32>) -> Self {
        let mut losses = Self::default();
        for (&kind, &count) in counts {
            if count == 0 {
                continue;
            }
            if kind.is_unit() {
                losses.units.insert(kind, count);
            } else if kind.is_building() {
                losses.buildings.insert(kind, count);
            } else {
                continue;
            }
            let (steel, oil) = rts_rules::economy::cost(kind);
            losses.steel_value = losses
                .steel_value
                .saturating_add(steel.saturating_mul(count));
            losses.oil_value = losses.oil_value.saturating_add(oil.saturating_mul(count));
        }
        losses
    }
}
