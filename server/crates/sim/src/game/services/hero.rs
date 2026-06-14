use crate::config;
use crate::game::entity::{EntityKind, EntityStore};

pub(crate) fn hero_regeneration_system(entities: &mut EntityStore, tick: u32) {
    if !tick.is_multiple_of(config::EKATERINA_REGEN_TICKS) {
        return;
    }
    let max_hp = config::unit_stats(EntityKind::Ekaterina)
        .map(|stats| stats.hp)
        .unwrap_or(0);
    if max_hp == 0 {
        return;
    }
    for id in entities.ids() {
        let Some(entity) = entities.get_mut(id) else {
            continue;
        };
        if entity.kind != EntityKind::Ekaterina || entity.hp == 0 || entity.hp >= max_hp {
            continue;
        }
        entity.restore_hp(config::EKATERINA_REGEN_HP);
    }
}
