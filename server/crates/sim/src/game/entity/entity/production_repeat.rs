use crate::game::entity::EntityKind;

use super::Entity;

impl Entity {
    pub(crate) fn repeat_production(&self) -> Option<EntityKind> {
        let production = self.production.as_ref()?;
        let count = production.repeat_units.len();
        if count == 0 {
            return None;
        }
        production
            .repeat_units
            .get(production.repeat_unit_cursor % count)
            .copied()
    }

    /// `Some(unit)` toggles that unit; `None` clears (false) or advances the repeat cursor (true).
    pub(crate) fn set_repeat_production(
        &mut self,
        unit: Option<EntityKind>,
        enabled: bool,
    ) -> bool {
        let Some(production) = self.production.as_mut() else {
            return false;
        };
        match unit {
            Some(unit) if enabled => {
                if !production.repeat_units.contains(&unit) {
                    production.repeat_units.push(unit);
                }
            }
            Some(unit) => {
                let count = production.repeat_units.len();
                if let Some(removed_index) = production
                    .repeat_units
                    .iter()
                    .position(|&current| current == unit)
                {
                    let cursor = production.repeat_unit_cursor % count;
                    production.repeat_units.remove(removed_index);
                    if production.repeat_units.is_empty() {
                        production.repeat_unit_cursor = 0;
                    } else if removed_index < cursor {
                        production.repeat_unit_cursor = cursor.saturating_sub(1);
                    } else {
                        production.repeat_unit_cursor = cursor % production.repeat_units.len();
                    }
                }
            }
            None if enabled => {
                let count = production.repeat_units.len();
                if count > 0 {
                    production.repeat_unit_cursor =
                        (production.repeat_unit_cursor % count + 1) % count;
                }
            }
            None => {
                production.repeat_units.clear();
                production.repeat_unit_cursor = 0;
            }
        }
        true
    }
}
