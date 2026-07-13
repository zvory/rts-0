use super::{Entity, EntityKind};

impl Entity {
    pub(in crate::game) fn lock_tank_armor_reaction_source(
        &mut self,
        source: (f32, f32),
        tick: u32,
    ) {
        if self.kind != EntityKind::Tank || self.hp == 0 {
            return;
        }
        if let Some(combat) = self.combat.as_mut() {
            combat.try_lock_tank_armor_reaction(source, tick);
        }
    }
}
