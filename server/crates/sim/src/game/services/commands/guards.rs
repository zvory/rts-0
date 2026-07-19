use std::collections::HashSet;

use crate::command_budget::{BASE_COMMAND_SUPPLY_CAP, COMMAND_CAR_SUPPLY_CAP_BONUS};
use crate::game::commands::CommandAdmission;
use crate::game::entity::{BuildPhase, EntityKind, EntityStore, RallyIntent, RallyKind};
use crate::game::map::Map;
use crate::rules;
use rts_contract::{LAB_MAX_UNITS_PER_COMMAND, MAX_UNITS_PER_COMMAND};

#[derive(Clone, Copy)]
pub(super) struct CommandAdmissionPolicy {
    pub(super) enforce_budget: bool,
    pub(super) max_units_per_command: usize,
}

pub(super) fn command_admission_for(
    admission: CommandAdmission,
    player_is_ai: bool,
) -> CommandAdmissionPolicy {
    match admission {
        CommandAdmission::Normal => CommandAdmissionPolicy {
            enforce_budget: !player_is_ai,
            max_units_per_command: MAX_UNITS_PER_COMMAND,
        },
        CommandAdmission::LabIgnoreCommandLimits => CommandAdmissionPolicy {
            enforce_budget: false,
            max_units_per_command: LAB_MAX_UNITS_PER_COMMAND,
        },
    }
}

/// Dedupe an already raw-cap-validated command's unit ids, preserving first-seen order.
pub(super) fn dedupe_units(units: Vec<u32>) -> Vec<u32> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(units.len());
    for id in units {
        if seen.insert(id) {
            out.push(id);
        }
    }
    out
}

pub(super) fn dedupe_cap_units(units: Vec<u32>, cap: usize) -> Vec<u32> {
    dedupe_units(units.into_iter().take(cap).collect())
}

pub(super) fn player_is_ai(mut players: impl Iterator<Item = (u32, bool)>, player: u32) -> bool {
    players
        .find(|(candidate, _)| *candidate == player)
        .is_some_and(|(_, is_ai)| is_ai)
}

pub(super) fn command_budget_exceeded(entities: &EntityStore, player: u32, units: &[u32]) -> bool {
    let mut used = 0u32;
    let mut cap = BASE_COMMAND_SUPPLY_CAP;
    for id in units {
        let Some(entity) = entities.get(*id) else {
            continue;
        };
        if entity.owner != player || !entity.is_unit() || entity.under_construction() {
            continue;
        }
        let weight = command_weight(entity.kind);
        used = used.saturating_add(weight);
        if entity.kind == EntityKind::CommandCar {
            cap = cap.saturating_add(COMMAND_CAR_SUPPLY_CAP_BONUS.saturating_add(weight));
        }
    }
    used > cap
}

pub(super) fn unit_can_accept_player_command(
    entities: &EntityStore,
    player: u32,
    unit: u32,
) -> bool {
    matches!(entities.get(unit), Some(entity) if entity.owner == player && entity.is_unit())
        && !is_constructing(entities, unit)
}

pub(super) fn unit_can_accept_ground_command(
    entities: &EntityStore,
    player: u32,
    unit: u32,
) -> bool {
    unit_can_accept_player_command(entities, player, unit)
        && !matches!(entities.get(unit), Some(entity) if entity.kind == EntityKind::ScoutPlane)
}

/// True if this unit is a worker that has already begun laying concrete - it cannot
/// be pulled away until the building finishes or is destroyed.
pub(super) fn is_constructing(entities: &EntityStore, id: u32) -> bool {
    matches!(
        entities.get(id),
        Some(e) if matches!(e.build_phase(), Some(BuildPhase::Constructing { .. }))
    )
}

pub(super) fn rally_intent_for_map(
    map: &Map,
    kind: RallyKind,
    x: f32,
    y: f32,
) -> Option<RallyIntent> {
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    let max = (map.world_size_px() - 1.0).max(0.0);
    Some(RallyIntent::new(kind, x.clamp(0.0, max), y.clamp(0.0, max)))
}

fn command_weight(kind: EntityKind) -> u32 {
    rules::economy::supply_cost(kind).max(1)
}
