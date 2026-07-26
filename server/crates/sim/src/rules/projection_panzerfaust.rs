use crate::game::entity::{Entity, EntityKind, PanzerfaustState};
use crate::protocol::EntityView;

pub(super) fn project_panzerfaust_state(entity: &Entity, view: &mut EntityView) {
    if entity.kind != EntityKind::Panzerfaust {
        return;
    }

    let state = entity.combat.as_ref().and_then(|combat| combat.panzerfaust);
    view.panzerfaust_loaded = state.map(|state| {
        matches!(
            state,
            PanzerfaustState::Loaded | PanzerfaustState::Windup { .. }
        )
    });
    if let Some(PanzerfaustState::Windup {
        ticks_remaining,
        total_ticks,
        ..
    }) = state
    {
        let total = total_ticks.max(1);
        let elapsed = total.saturating_sub(ticks_remaining);
        view.panzerfaust_windup_progress = Some((elapsed as f32 / total as f32).clamp(0.0, 1.0));
    }
}
