//! Entrenchment upgrade, trench footprint, and combat constants.

use crate::EntityKind;

use super::TICK_HZ;

pub const ENTRENCHMENT_DIG_IN_TICKS: u32 = TICK_HZ * 3;
pub const ENTRENCHMENT_RANGE_BONUS_TILES: u32 = 1;
pub const ENTRENCHMENT_DIRECT_MISS_CHANCE: f32 = 0.50;
pub const ENTRENCHMENT_AREA_DAMAGE_REDUCTION: f32 = 0.70;
pub const ENTRENCHMENT_TRENCH_RADIUS_TILES: f32 = 0.375;

pub fn is_entrenchment_eligible_infantry(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Rifleman | EntityKind::MachineGunner | EntityKind::Panzerfaust
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entrenchment_eligibility_names_only_combat_infantry() {
        let eligible = [
            EntityKind::Rifleman,
            EntityKind::MachineGunner,
            EntityKind::Panzerfaust,
        ];
        for kind in EntityKind::ALL {
            assert_eq!(
                is_entrenchment_eligible_infantry(kind),
                eligible.contains(&kind),
                "{kind:?}"
            );
        }
    }
}
