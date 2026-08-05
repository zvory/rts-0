use crate::game::entity::{EntityKind, EntityStore};
use crate::rules::faction::{FactionLoadout, StartingFormation};

pub(super) fn spawn(
    entities: &mut EntityStore,
    owner: u32,
    loadout: FactionLoadout,
    base_resource_ids: &[u32],
    home_x: f32,
    home_y: f32,
) -> Vec<EntityKind> {
    let mut spawned = Vec::new();
    let producer_id = entities
        .iter()
        .find(|entity| {
            entity.owner == owner
                && crate::rules::economy::trainable_units(entity.kind)
                    .iter()
                    .any(|kind| kind.is_resource_extractor())
                && entity.hp > 0
                && !entity.under_construction()
        })
        .map(|entity| entity.id);
    for group in loadout
        .starting_entities
        .iter()
        .filter(|group| group.formation == StartingFormation::ResourcePatches)
    {
        let Some(node_kind) = group.kind.extracted_resource_kind() else {
            continue;
        };
        let mut nodes = base_resource_ids
            .iter()
            .filter_map(|id| entities.get(*id))
            .filter(|entity| entity.kind == node_kind && entity.remaining().unwrap_or(0) > 0)
            .map(|entity| {
                let dx = entity.pos_x - home_x;
                let dy = entity.pos_y - home_y;
                (entity.id, dx * dx + dy * dy, entity.pos_x, entity.pos_y)
            })
            .collect::<Vec<_>>();
        nodes.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        for (_, _, x, y) in nodes.into_iter().take(group.count as usize) {
            if let Some(id) = entities.spawn_building(owner, group.kind, x, y, group.completed) {
                if let Some(producer_id) = producer_id {
                    if let Some(extractor) = entities
                        .get_mut(id)
                        .and_then(|entity| entity.resource_extractor.as_mut())
                    {
                        extractor.producer_id = Some(producer_id);
                    }
                }
                spawned.push(group.kind);
            }
        }
    }
    spawned
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::faction::CURRENT_CATALOG;

    #[test]
    fn starting_extractor_uses_only_its_newly_spawned_base_resources() {
        let mut entities = EntityStore::new();
        let loadout = CURRENT_CATALOG.loadout;
        let extractor_kind = loadout
            .starting_entities
            .iter()
            .find(|group| group.formation == StartingFormation::ResourcePatches)
            .expect("catalog should expose a resource-patch starting group")
            .kind;
        let node_kind = extractor_kind
            .extracted_resource_kind()
            .expect("resource-patch entity should identify its node kind");
        let unrelated = entities
            .spawn_node(node_kind, 32.0, 32.0)
            .expect("unrelated resource patch");
        let own = entities
            .spawn_node(node_kind, 320.0, 320.0)
            .expect("new base resource patch");

        assert_eq!(
            spawn(&mut entities, 7, loadout, &[own], 32.0, 32.0),
            vec![extractor_kind]
        );

        let mine = entities
            .iter()
            .find(|entity| entity.owner == 7 && entity.kind == extractor_kind)
            .expect("starting resource extractor");
        assert_eq!((mine.pos_x, mine.pos_y), (320.0, 320.0));
        assert_ne!((mine.pos_x, mine.pos_y), (32.0, 32.0));
        assert!(entities.contains(unrelated));
    }
}
