use serde::{Deserialize, Serialize};

use super::entity::EntityKind;
use super::upgrade::UpgradeKind;

pub(crate) const MAX_PRODUCTION_REQUESTS: usize = 128;
pub(crate) const MAX_REQUEST_QUANTITY: u32 = 1_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProductionRequest {
    pub(crate) item: ProductionRequestItem,
    /// `None` is automatic production (an infinite quantity).
    pub(crate) remaining: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum ProductionRequestItem {
    Unit { building: u32, unit: EntityKind },
    Research { building: u32, upgrade: UpgradeKind },
}

impl ProductionRequestItem {
    pub(crate) fn producer_id(&self) -> u32 {
        match *self {
            Self::Unit { building, .. } | Self::Research { building, .. } => building,
        }
    }
}

impl ProductionRequest {
    pub(crate) fn finite(item: ProductionRequestItem, quantity: u32) -> Self {
        Self {
            item,
            remaining: Some(quantity.clamp(1, MAX_REQUEST_QUANTITY)),
        }
    }

    pub(crate) fn automatic(item: ProductionRequestItem) -> Self {
        Self {
            item,
            remaining: None,
        }
    }
}
