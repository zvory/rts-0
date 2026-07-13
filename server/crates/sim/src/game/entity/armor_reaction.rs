use super::Entity;

impl Entity {
    pub(in crate::game) fn record_incoming_direct_ap_threat(
        &mut self,
        attacker_id: u32,
        attacker_pos: (f32, f32),
        damage_weight: u32,
        tick: u32,
    ) {
        if !crate::rules::combat::unit_reacts_to_direct_ap(self.kind) || self.hp == 0 {
            return;
        }
        if let Some(combat) = self.combat.as_mut() {
            combat.record_incoming_direct_ap_threat(attacker_id, attacker_pos, damage_weight, tick);
        }
    }
}
