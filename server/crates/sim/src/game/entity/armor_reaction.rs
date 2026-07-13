use super::Entity;

impl Entity {
    pub(in crate::game) fn lock_tank_armor_reaction_source(
        &mut self,
        source: (f32, f32),
        tick: u32,
    ) {
        if !crate::rules::combat::unit_uses_tank_armor_reaction(self.kind) || self.hp == 0 {
            return;
        }
        if let Some(combat) = self.combat.as_mut() {
            combat.try_lock_tank_armor_reaction(source, tick);
        }
    }
}
