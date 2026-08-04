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
        let (kind, cost_paid, producer_id) = entities.get(building).and_then(|entity| {
            (entity.owner == player && entity.is_building() && entity.under_construction())
                .then_some((
                    entity.kind,
                    entity.construction_cost_paid(),
                    entity.construction_producer_id(),
                ))
        })?;
        if let Some(producer_id) = producer_id {
            let cancelled = entities.get_mut(producer_id).and_then(|producer| {
                (producer.owner == player
                    && producer
                        .prod_queue()
                        .first()
                        .is_some_and(|front| front.unit == kind))
                .then(|| producer.remove_front_production())
                .flatten()
            });
            entities.remove(building)?;
            return cancelled.map(Cancelled::Unit);
        }
        let builders = entities
            .iter()
            .filter_map(|entity| {
                (entity.hp > 0 && entity.is_unit() && entity.order().build_site() == Some(building))
                    .then_some(entity.id)
            })
            .collect::<Vec<_>>();
        entities.remove(building)?;
        for builder in builders {
            if let Some(worker) = entities.get_mut(builder) {
                worker.clear_active_order();
            }
        }
        return Some(Cancelled::Construction { kind, cost_paid });
    }

    let (cancelled, remove_extractor_scaffold) = {
        let b = match entities.get_mut(building) {
            Some(b) if b.owner == player && b.is_building() && !b.under_construction() => b,
            _ => return None,
        };
        b.set_repeat_production(None, false);
        if let Some(item) = b.pop_last_research() {
            (Cancelled::Upgrade(item), false)
        } else {
            let item = b.pop_last_production()?;
            let remove_scaffold = item.unit.is_resource_extractor() && b.prod_queue().is_empty();
            (Cancelled::Unit(item), remove_scaffold)
        }
    };
    if remove_extractor_scaffold {
        let scaffold = entities
            .iter()
            .find(|entity| entity.construction_producer_id() == Some(building))
            .map(|entity| entity.id);
        if let Some(scaffold) = scaffold {
            entities.remove(scaffold);
        }
    }
    Some(cancelled)
}
