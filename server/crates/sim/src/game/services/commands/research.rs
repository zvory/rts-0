use super::*;

pub(super) fn enqueue(
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    events: &mut HashMap<u32, Vec<Event>>,
    player: u32,
    faction_id: &str,
    building: u32,
    upgrade: upgrade::UpgradeKind,
) {
    let definition = upgrade::definition(upgrade);
    let Some(building_kind) = entities.get(building).map(|b| b.kind) else {
        notice(events, player, "Cannot research that here");
        return;
    };
    let ok = matches!(entities.get(building), Some(b)
    if b.owner == player && b.is_building()
    && b.kind == definition.researched_at
    && rules::economy::can_research_for_faction(
        faction_id,
        upgrade,
        building_kind,
    ));
    if !ok {
        notice(events, player, "Cannot research that here");
        return;
    }
    if entities.iter().any(|e| {
        e.owner == player
            && e.research_queue()
                .iter()
                .any(|item| item.upgrade == upgrade)
    }) {
        notice(events, player, "Already researching");
        return;
    }
    let Some(ps) = players.iter_mut().find(|p| p.id == player) else {
        return;
    };
    if ps.upgrades.contains(&upgrade) {
        notice(events, player, "Already researched");
        return;
    }
    let prerequisite_available = definition.requires_upgrade.is_none_or(|required| {
        ps.upgrades.contains(&required)
            || entities.get(building).is_some_and(|b| {
                b.research_queue()
                    .iter()
                    .any(|item| item.upgrade == required)
            })
    });
    if !prerequisite_available {
        notice(events, player, "Requirement not met");
        return;
    }
    let cost =
        rules::economy::ResourceCost::new(definition.cost_steel, definition.cost_oil);
    let (queue_empty, building_complete) =
        entities.get(building).map_or((false, false), |b| {
            (b.research_queue().is_empty(), !b.under_construction())
        });
    let paid = building_complete && queue_empty && ps.spend_cost(cost);

    let queued = entities.get_mut(building).is_some_and(|b| {
        b.push_research(ResearchItem {
            upgrade,
            progress: 0,
            total: definition.research_ticks,
            paid,
        })
    });
    if !queued {
        if paid {
            ps.refund_cost(cost);
        }
        notice(events, player, "Production queue full");
    }
}
