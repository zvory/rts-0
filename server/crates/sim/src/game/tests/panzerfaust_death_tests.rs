use super::panzerfaust_tests::{
    enqueue_attack, panzerfaust_damage_to, panzerfaust_fixture, player_events,
};
use super::*;

#[test]
fn panzerfaust_death_during_travel_does_not_cancel_hit() {
    let (mut game, panzerfaust, tank) = panzerfaust_fixture();
    game.state
        .entities
        .get_mut(panzerfaust)
        .expect("panzerfaust exists")
        .set_invulnerable(false);
    let tank_hp = game.state.entities.get(tank).expect("tank exists").hp;
    enqueue_attack(&mut game, panzerfaust, tank, false);

    let mut saw_launch = false;
    for _ in 0..30 {
        let events = game.tick();
        if player_events(&events, 1)
            .iter()
            .any(|event| matches!(event, Event::PanzerfaustLaunch { .. }))
        {
            saw_launch = true;
            break;
        }
    }
    assert!(
        saw_launch,
        "test setup should reach launch before killing panzerfaust"
    );
    game.state
        .entities
        .get_mut(panzerfaust)
        .expect("panzerfaust exists")
        .apply_damage(u32::MAX, None);

    let mut impact_hp = None;
    let mut saw_death_as_panzerfaust = false;
    let mut saw_conversion = false;
    for _ in 0..70 {
        let events = game.tick();
        let owner_events = player_events(&events, 1);
        if impact_hp.is_none()
            && owner_events
                .iter()
                .any(|event| matches!(event, Event::PanzerfaustImpact { .. }))
        {
            impact_hp = game.state.entities.get(tank).map(|tank| tank.hp);
        }
        saw_death_as_panzerfaust |= owner_events.iter().any(|event| {
            matches!(event, Event::Death { id, kind, .. }
                if *id == panzerfaust && kind == crate::protocol::kinds::PANZERFAUST)
        });
        saw_conversion |= owner_events.iter().any(
            |event| matches!(event, Event::PanzerfaustConversion { id, .. } if *id == panzerfaust),
        );
    }

    assert!(game.state.entities.get(panzerfaust).is_none());
    assert_eq!(
        impact_hp,
        Some(tank_hp.saturating_sub(panzerfaust_damage_to(EntityKind::Tank)))
    );
    assert!(saw_death_as_panzerfaust);
    assert!(!saw_conversion);
}
