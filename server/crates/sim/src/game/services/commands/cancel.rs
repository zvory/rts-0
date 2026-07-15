use crate::game::entity::{EntityKind, EntityStore, ProdItem, ResearchItem};

pub(super) enum Cancelled {
    Construction { kind: EntityKind, cost_paid: bool },
    Unit(ProdItem),
    Upgrade(ResearchItem),
}

/// Apply the entity-side mutation for construction or production cancellation. The caller owns
/// player resource, supply, and scoring settlement for the returned outcome.
pub(super) fn apply(
    entities: &mut EntityStore,
    player: u32,
    building: u32,
    cancel_construction: bool,
) -> Option<Cancelled> {
    if cancel_construction {
        let (kind, cost_paid) = entities.get(building).and_then(|entity| {
            (entity.owner == player && entity.is_building() && entity.under_construction())
                .then_some((entity.kind, entity.construction_cost_paid()))
        })?;
        let builders = entities
            .iter()
            .filter(|entity| {
                entity.hp > 0 && entity.is_unit() && entity.order().build_site() == Some(building)
            })
            .map(|entity| entity.id)
            .collect::<Vec<_>>();
        entities.remove(building)?;
        for builder in builders {
            if let Some(worker) = entities.get_mut(builder) {
                worker.clear_active_order();
            }
        }
        return Some(Cancelled::Construction { kind, cost_paid });
    }

    Some({
        let b = match entities.get_mut(building) {
            Some(b) if b.owner == player && b.is_building() && !b.under_construction() => b,
            _ => return None,
        };
        b.set_repeat_production(None, false);
        if let Some(item) = b.pop_last_research() {
            Cancelled::Upgrade(item)
        } else {
            Cancelled::Unit(b.pop_last_production()?)
        }
    })
}
