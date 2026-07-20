use super::*;

pub(super) fn setup_ticks_for(kind: EntityKind) -> u16 {
    match kind {
        EntityKind::Artillery => config::ARTILLERY_SETUP_TICKS,
        _ => config::ANTI_TANK_GUN_SETUP_TICKS,
    }
}

pub(super) fn teardown_ticks_for(kind: EntityKind) -> u16 {
    match kind {
        EntityKind::MortarTeam => config::MORTAR_TEAM_TEARDOWN_TICKS,
        _ => setup_ticks_for(kind),
    }
}

pub(super) fn choose_smoke_caster(
    map: &Map,
    entities: &EntityStore,
    ability: AbilityKind,
    eligible: &[u32],
    x: f32,
    y: f32,
) -> Option<u32> {
    let mut furthest_in_range: Option<(u32, f32)> = None;
    let mut closest: Option<(u32, f32)> = None;
    for id in eligible {
        let Some(e) = entities.get(*id) else {
            continue;
        };
        let d2 = dist2(e.pos_x, e.pos_y, x, y);
        if closest.is_none_or(|(_, best)| d2 < best) {
            closest = Some((*id, d2));
        }
        if ability_orders::caster_in_range(map, entities, *id, ability, x, y)
            && furthest_in_range.is_none_or(|(_, best)| d2 > best)
        {
            furthest_in_range = Some((*id, d2));
        }
    }
    furthest_in_range.or(closest).map(|(id, _)| id)
}

pub(super) fn clear_queued_orders(entities: &mut EntityStore, ids: &[u32]) {
    for id in ids {
        if let Some(e) = entities.get_mut(*id) {
            e.clear_queued_orders();
        }
    }
}

pub(super) fn clear_staged_anti_tank_gun_setup(entities: &mut EntityStore, ids: &[u32]) {
    for id in ids {
        let Some(e) = entities.get_mut(*id) else {
            continue;
        };
        if e.kind == EntityKind::AntiTankGun {
            e.set_emplacement_facing(None);
            e.set_pending_redeploy_facing(None);
        }
    }
}
