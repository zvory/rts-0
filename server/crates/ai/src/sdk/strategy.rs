use rts_rules::faction::UpgradeKind;
use rts_rules::EntityKind;

use super::AiFrame;

/// Maximum requests retained from one strategy step.
pub const MAX_ACTIONS_PER_STEP: usize = 256;

/// Object-safe lifecycle implemented by a custom Rust strategy.
///
/// [`AiStrategy::initialize`] is called exactly once, immediately before the first `step`. Both
/// lifecycle calls receive only a player-scoped [`AiFrame`], and `step` is invoked only on the
/// canonical nine-tick, player-staggered decision cadence.
pub trait AiStrategy: Send {
    fn initialize(&mut self, _frame: &AiFrame) {}

    fn step(&mut self, frame: &AiFrame, actions: &mut AiActions);
}

/// A bounded, call-ordered collection of action requests for one strategy step.
#[derive(Debug, Default)]
pub struct AiActions {
    requests: Vec<AiActionRequest>,
}

impl AiActions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Retains the request when the per-step bound has not been reached.
    pub fn submit(&mut self, request: AiActionRequest) -> bool {
        if self.requests.len() >= MAX_ACTIONS_PER_STEP {
            return false;
        }
        self.requests.push(request);
        true
    }

    pub fn len(&self) -> usize {
        self.requests.len()
    }

    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    pub(crate) fn into_requests(self) -> Vec<AiActionRequest> {
        self.requests
    }
}

/// Phase-3 action vocabulary. Requests express intent but are not acceptance receipts; the host
/// translates them to ordinary simulation commands, whose normal validation remains authoritative.
#[derive(Clone, Debug, PartialEq)]
pub enum AiActionRequest {
    Move {
        units: Vec<u32>,
        x: f32,
        y: f32,
        queued: bool,
    },
    AttackMove {
        units: Vec<u32>,
        x: f32,
        y: f32,
        queued: bool,
    },
    Attack {
        units: Vec<u32>,
        target: u32,
        queued: bool,
    },
    Gather {
        units: Vec<u32>,
        node: u32,
        queued: bool,
    },
    Build {
        units: Vec<u32>,
        building: EntityKind,
        tile_x: u32,
        tile_y: u32,
        queued: bool,
    },
    Train {
        building: u32,
        unit: EntityKind,
    },
    Research {
        building: u32,
        upgrade: UpgradeKind,
    },
    HoldPosition {
        units: Vec<u32>,
        queued: bool,
    },
    SetupAntiTankGuns {
        units: Vec<u32>,
        x: f32,
        y: f32,
        queued: bool,
    },
}
