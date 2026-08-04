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
            if entities
                .spawn_building(owner, group.kind, x, y, group.completed)
                .is_some()
            {
                spawned.push(group.kind);
            }
        }
    }
    spawned
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::faction::StartingEntityGroup;

    const STARTING_MINES: &[StartingEntityGroup] = &[StartingEntityGroup {
        kind: EntityKind::SteelMine,
        count: 1,
        formation: StartingFormation::ResourcePatches,
        completed: true,
    }];
    const LOADOUT: FactionLoadout = FactionLoadout {
        id: "test.resource-patch-start",
        initial_steel: 0,
        initial_oil: 0,
        starting_entities: STARTING_MINES,
        opening_upgrades: &[],
    };

    #[test]
    fn starting_extractor_uses_only_its_newly_spawned_base_resources() {
        let mut entities = EntityStore::new();
        let unrelated = entities
            .spawn_node(EntityKind::Steel, 32.0, 32.0)
            .expect("unrelated steel patch");
        let own = entities
            .spawn_node(EntityKind::Steel, 320.0, 320.0)
            .expect("new base steel patch");

        assert_eq!(
            spawn(&mut entities, 7, LOADOUT, &[own], 32.0, 32.0),
            vec![EntityKind::SteelMine]
        );

        let mine = entities
            .iter()
            .find(|entity| entity.owner == 7 && entity.kind == EntityKind::SteelMine)
            .expect("starting Steel Mine");
        assert_eq!((mine.pos_x, mine.pos_y), (320.0, 320.0));
        assert_ne!((mine.pos_x, mine.pos_y), (32.0, 32.0));
        assert!(entities.contains(unrelated));
    }
}
