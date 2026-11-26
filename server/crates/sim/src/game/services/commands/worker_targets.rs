//! Authority checks for worker target orders.
use super::*;

pub(super) fn deconstruct_target_valid(
    entities: &EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    player: u32,
    units: &[u32],
    target: u32,
) -> bool {
    units
        .iter()
        .copied()
        .any(|unit| deconstruct_unit_can_target(entities, teams, fog, smokes, player, unit, target))
}

pub(super) fn deconstruct_unit_can_target(
    entities: &EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    player: u32,
    unit: u32,
    target: u32,
) -> bool {
    if !matches!(entities.get(unit), Some(e) if e.owner == player && e.kind == EntityKind::Worker && e.hp > 0)
    {
        return false;
    }
    let Some(target) = entities.get(target) else {
        return false;
    };
    if target.kind != EntityKind::TankTrap || target.hp == 0 || target.under_construction() {
        return false;
    }
    teams.same_team_or_same_owner(player, target.owner)
        || (rules::projection::team_visible_world(
            player,
            target.pos_x(),
            target.pos_y(),
            fog,
            teams,
        ) && !smokes.point_inside(target.pos_x(), target.pos_y()))
}

pub(super) fn repair_unit_can_target(
    entities: &EntityStore,
    owner: u32,
    unit: u32,
    target: u32,
) -> bool {
    matches!(entities.get(unit), Some(e) if e.owner == owner && e.kind == EntityKind::Worker && e.hp > 0)
        && matches!(entities.get(target), Some(t) if t.can_be_repaired_by(owner))
}

pub(super) fn repair_request(
    entities: &EntityStore,
    player: u32,
    units: &[u32],
    target: u32,
    queued: bool,
) -> planner::OrderRequest {
    planner::OrderRequest {
        units: units.to_vec(),
        mode: issue_mode(queued),
        order: planner::RequestedOrder::Repair {
            target,
            target_valid: units
                .iter()
                .any(|&unit| repair_unit_can_target(entities, player, unit, target)),
        },
    }
}
