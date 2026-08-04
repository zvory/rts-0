use crate::game::entity::{EntityKind, EntityStore};
use crate::rules::faction::{FactionLoadout, StartingFormation};

pub(super) fn spawn(
    entities: &mut EntityStore,
    owner: u32,
    loadout: FactionLoadout,
    home_x: f32,
    home_y: f32,
) -> Vec<EntityKind> {
    let mut spawned = Vec::new();
    let range = crate::config::MINING_ANCHOR_RANGE_TILES * crate::config::TILE_SIZE as f32;
    let range2 = range * range + 0.01;
    for group in loadout
        .starting_entities
        .iter()
        .filter(|group| group.formation == StartingFormation::ResourcePatches)
    {
        let node_kind = match group.kind {
            EntityKind::SteelMine => EntityKind::Steel,
            EntityKind::PumpJack => EntityKind::Oil,
            _ => continue,
        };
        let mut nodes = entities
            .iter()
            .filter(|entity| entity.kind == node_kind && entity.remaining().unwrap_or(0) > 0)
            .filter_map(|entity| {
                let dx = entity.pos_x - home_x;
                let dy = entity.pos_y - home_y;
                let dist2 = dx * dx + dy * dy;
                (dist2 <= range2).then_some((entity.id, dist2, entity.pos_x, entity.pos_y))
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
