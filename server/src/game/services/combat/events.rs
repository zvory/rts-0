use std::collections::HashMap;

use crate::game::entity::{Entity, EntityKind};
use crate::game::fog::Fog;
use crate::protocol::{AttackReveal, Event, NoticeSeverity};
use crate::rules::projection;

pub(super) fn attack_reveal_for(attacker: Option<&Entity>) -> Option<AttackReveal> {
    let attacker = attacker?;
    if attacker.kind != EntityKind::AtTeam {
        return None;
    }
    Some(AttackReveal {
        owner: attacker.owner,
        kind: attacker.kind.to_protocol_str().to_string(),
        x: attacker.pos_x,
        y: attacker.pos_y,
        facing: Some(attacker.facing()),
        weapon_facing: attacker.weapon_facing(),
        setup_state: Some(attacker.weapon_setup().to_protocol_str().to_string()),
    })
}

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
    reveal: Option<AttackReveal>,
) {
    let player_ids: Vec<u32> = events.keys().copied().collect();
    for pid in player_ids {
        if !projection::attack_event_visible_to(pid, ax, ay, vx, vy, attacker_owner, fog) {
            continue;
        }
        events.entry(pid).or_default().push(Event::Attack {
            from: attacker,
            to: victim,
            reveal: reveal.clone(),
            to_pos: Some([vx, vy]),
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
