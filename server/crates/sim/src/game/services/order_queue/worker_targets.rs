//! Revalidate worker targets when queued work is promoted.
use super::*;
pub(super) fn deconstruct_intent_valid(
    entities: &EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    owner: u32,
    worker: u32,
    target: u32,
) -> bool {
    if !matches!(entities.get(worker), Some(e) if e.owner == owner && e.kind == EntityKind::Worker && e.hp > 0)
    {
        return false;
    }
    let Some(target) = entities.get(target) else {
        return false;
    };
    if target.kind != EntityKind::TankTrap || target.hp == 0 || target.under_construction() {
        return false;
    }
    teams.same_team_or_same_owner(owner, target.owner)
        || rules::projection::team_visible_world(owner, target.pos_x(), target.pos_y(), fog, teams)
}

pub(super) fn repair_intent_valid(
    entities: &EntityStore,
    owner: u32,
    worker: u32,
    target: u32,
) -> bool {
    matches!(entities.get(worker), Some(e) if e.owner == owner && e.kind == EntityKind::Worker && e.hp > 0)
        && matches!(entities.get(target), Some(t) if t.can_be_repaired_by(owner))
}
