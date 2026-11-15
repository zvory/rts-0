use super::{Entity, EntityKind, NEUTRAL};

impl Entity {
    /// Wire durability counts crossings; tanks crush a segment in one entry.
    /// Weapons and healing cannot alter wire durability.
    pub(in crate::game) fn consume_wire_crossing(&mut self, crushed: bool) -> bool {
        if self.kind != EntityKind::BarbedWire || self.hp == 0 || self.under_construction() {
            return false;
        }
        self.hp = if crushed {
            0
        } else {
            self.hp.saturating_sub(1)
        };
        true
    }

    /// Completed neutral field obstacles persist independently of players. Targetability and
    /// weapon immunity remain kind-specific; resource nodes are not field obstacles.
    pub fn is_neutral_obstacle(&self) -> bool {
        self.owner == NEUTRAL
            && matches!(self.kind, EntityKind::TankTrap | EntityKind::BarbedWire)
            && !self.under_construction()
    }

    pub(in crate::game) fn neutralize_completed_tank_trap(&mut self) -> bool {
        let completed_tank_trap =
            matches!(self.kind, EntityKind::TankTrap | EntityKind::BarbedWire)
                && !self.under_construction();
        let changed_owner = completed_tank_trap && self.owner != NEUTRAL;
        if completed_tank_trap {
            self.owner = NEUTRAL;
        }
        changed_owner
    }
}
