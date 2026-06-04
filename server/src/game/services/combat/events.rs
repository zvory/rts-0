use std::collections::HashMap;

use crate::game::fog::Fog;
use crate::protocol::{Event, NoticeSeverity};
use crate::rules::projection;

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_attack_event(
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    attacker: u32,
    victim: u32,
    attacker_owner: u32,
    ax: f32,
    ay: f32,
    vx: f32,
    vy: f32,
) {
    let player_ids: Vec<u32> = events.keys().copied().collect();
    for pid in player_ids {
        if !projection::attack_event_visible_to(pid, ax, ay, vx, vy, attacker_owner, fog) {
            continue;
        }
        events.entry(pid).or_default().push(Event::Attack {
            from: attacker,
            to: victim,
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn push_under_attack_notices_for_visible_attack(
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    victim_owner: u32,
    attacker_owner: u32,
    ax: f32,
    ay: f32,
    vx: f32,
    vy: f32,
) {
    let player_ids: Vec<u32> = events.keys().copied().collect();
    for pid in player_ids {
        if !projection::attack_event_visible_to(pid, ax, ay, vx, vy, attacker_owner, fog) {
            continue;
        }
        push_under_attack_notice(events, pid, victim_owner, attacker_owner, vx, vy);
    }
}

pub(super) fn push_under_attack_notice(
    events: &mut HashMap<u32, Vec<Event>>,
    recipient: u32,
    victim_owner: u32,
    attacker_owner: u32,
    x: f32,
    y: f32,
) {
    if victim_owner == 0 || victim_owner == attacker_owner || recipient != victim_owner {
        return;
    }
    events.entry(recipient).or_default().push(Event::Notice {
        msg: "alert:under_attack".to_string(),
        x: Some(x),
        y: Some(y),
        severity: NoticeSeverity::Alert,
    });
}
