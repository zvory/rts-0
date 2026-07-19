use crate::game::entity::{Entity, EntityKind, EntityStore, PanzerfaustState};

pub(super) fn panzerfaust_attack_cycle_active(attacker: &Entity) -> bool {
    attacker.kind == EntityKind::Panzerfaust
        && matches!(
            attacker
                .combat
                .as_ref()
                .and_then(|combat| combat.panzerfaust),
            Some(PanzerfaustState::Windup { .. })
        )
}

/// A direct Panzerfaust order finishes as soon as its one-use launcher has fired, allowing a
/// queued movement order to promote while the detached projectile resolves.
pub(super) fn direct_panzerfaust_shot_spent(
    entities: &EntityStore,
    attacker: &Entity,
    target: u32,
) -> bool {
    attacker.kind == EntityKind::Panzerfaust
        && matches!(
            attacker
                .combat
                .as_ref()
                .and_then(|combat| combat.panzerfaust),
            Some(PanzerfaustState::Spent)
        )
        && entities.get(target).is_some_and(|target| {
            crate::rules::combat::is_panzerfaust_loaded_shot_target(target.kind)
        })
}
