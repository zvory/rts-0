use crate::game::entity::{Entity, EntityKind};

use super::priority::{self, AttackPriorityContext, TargetCandidate};
use super::weapons::anti_tank_gun_target_inside_field_of_fire;

pub(super) fn choose_target_preferring_anti_tank_field(
    context: &AttackPriorityContext,
    attacker: &Entity,
    px: f32,
    py: f32,
    candidates: &[TargetCandidate],
    filter: impl Fn(&TargetCandidate) -> bool,
) -> Option<u32> {
    if attacker.kind == EntityKind::AntiTankGun {
        let in_field = priority::choose_target(context, candidates.iter().filter(|candidate| {
            filter(candidate)
                && anti_tank_gun_target_inside_field_of_fire(
                    attacker,
                    (candidate.pos_y - py).atan2(candidate.pos_x - px),
                )
        }));
        if in_field.is_some() {
            return in_field;
        }
    }
    priority::choose_target(context, candidates.iter().filter(|candidate| filter(candidate)))
}
