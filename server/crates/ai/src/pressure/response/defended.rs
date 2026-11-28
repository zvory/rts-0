//! Opt-in incident relevance for a caller's persistent defensive roles.
use super::*;

#[derive(Clone, Debug)]
pub(crate) struct DefendedArea {
    pub(crate) anchor: Point,
    /// Fixed building bubble; None preserves the staffed-post weapon envelope.
    pub(crate) radius_tiles: Option<f32>,
    /// Assigned guard IDs or protected own building IDs, never outbound/scouting units.
    pub(crate) members: BTreeSet<u32>,
}

#[derive(Default, Debug)]
pub(super) struct DefendedResponse {
    pub(super) changed_incident: bool,
    incidents: BTreeMap<u32, Incident>,
}
#[derive(Debug)]
struct Incident {
    anchor: Point,
    members: BTreeSet<u32>,
    leash: f32,
    position: Point,
    range: Option<f32>,
    target: Option<u32>,
    value: u32,
    kind: EntityKind,
    seen: u32,
    active: bool,
}
/// Last observed remaining combat value; never consult hidden health for remembered contacts.
fn observed_value(book: AiRulebook, enemy: &AiEntity) -> u32 {
    let maximum = u64::from(enemy.health.maximum);
    if maximum == 0 {
        return 0;
    }
    let current = u64::from(enemy.health.current.min(enemy.health.maximum));
    (u64::from(value(book, enemy)) * current).div_ceil(maximum) as u32
}
fn reach(frame: &AiFrame, enemy: &AiEntity) -> Option<f32> {
    let profile = rts_rules::combat::attack_profile(enemy.kind);
    // Artillery applies shell damage outside its zero-direct-damage weapon profile; loaded
    // Panzerfaust shots also have no ordinary cooldown. Reach, not those sentinels, defines
    // the public threat envelope. Noncombatants have no positive attack range.
    if !profile.range_tiles.is_finite() || profile.range_tiles <= 0. {
        return None;
    }
    Some(super::cohesion::weapon_reach(frame, enemy) + 2. * frame.map().tile_size as f32)
}
fn envelope(frame: &AiFrame, area: &DefendedArea, weapon: Option<f32>) -> f32 {
    area.radius_tiles
        .map(|r| r * frame.map().tile_size as f32)
        .unwrap_or_else(|| weapon.unwrap_or(2. * frame.map().tile_size as f32))
}
fn relevant_area<'a>(
    frame: &AiFrame,
    position: Point,
    range: Option<f32>,
    target: Option<u32>,
    areas: &'a [DefendedArea],
) -> Option<&'a DefendedArea> {
    let targeted = target.filter(|id| {
        frame
            .owned()
            .iter()
            .chain(frame.visible_allies())
            .any(|entity| entity.id == *id && entity.health.current > 0)
    });
    // A visible attacker targeting a controlled defender/building is relevant on either side
    // of the midpoint. Membership comes from the caller's assigned roles, not combat state.
    let matching = |area: &&DefendedArea| targeted.is_some_and(|id| area.members.contains(&id));
    areas
        .iter()
        .filter(matching)
        .min_by(|a, b| distance(position, a.anchor).total_cmp(&distance(position, b.anchor)))
        .or_else(|| {
            areas
                .iter()
                .filter(|area| distance(position, area.anchor) <= envelope(frame, area, range))
                .min_by(|a, b| {
                    distance(position, a.anchor).total_cmp(&distance(position, b.anchor))
                })
        })
}
impl DefendedResponse {
    pub(super) fn update(
        &mut self,
        frame: &AiFrame,
        book: AiRulebook,
        areas: &[DefendedArea],
    ) -> BTreeMap<u32, Contact> {
        self.changed_incident = false;
        self.incidents
            .retain(|_, incident| frame.tick().saturating_sub(incident.seen) < MEMORY_TICKS);
        for enemy in frame
            .visible_enemies()
            .iter()
            .filter(|enemy| enemy.kind.is_unit() || enemy.kind.is_building())
        {
            let range = reach(frame, enemy);
            let anchor = (enemy.health.current > 0)
                .then(|| relevant_area(frame, enemy.position, range, enemy.target, areas))
                .flatten();
            let Some(mut area) = anchor else {
                if enemy.health.current == 0 {
                    self.incidents.remove(&enemy.id);
                } else if let Some(incident) = self.incidents.get_mut(&enemy.id) {
                    // Keep the original leash through visible target loss/reacquisition too.
                    // Relevance releases ownership immediately; it must not reset chase bounds.
                    incident.position = enemy.position;
                    incident.range = range;
                    incident.target = enemy.target;
                    incident.seen = frame.tick();
                    incident.active = false;
                }
                continue;
            };
            if let Some(incident) = self.incidents.get(&enemy.id) {
                if distance(enemy.position, incident.anchor) > incident.leash {
                    // A different protected role is a new incident, not continued pursuit.
                    // Same-guard post drift cannot rearm the old incident's chase leash.
                    let next = areas
                        .iter()
                        .filter(|next| {
                            !next.members.is_empty()
                                // A new distant target order alone cannot hop between guard
                                // identities and extend pursuit. The shooter must reach the
                                // new protected area's weapon envelope before handoff.
                                && distance(enemy.position, next.anchor) <= envelope(frame,next,range)
                                && next.members.is_disjoint(&incident.members)
                                && (next.radius_tiles.is_some() || distance(next.anchor, incident.anchor)
                                    > 2. * frame.map().tile_size as f32)
                                && relevant_area(
                                    frame,
                                    enemy.position,
                                    range,
                                    enemy.target,
                                    std::slice::from_ref(*next),
                                )
                                .is_some()
                        })
                        .min_by(|a, b| {
                            distance(enemy.position, a.anchor)
                                .total_cmp(&distance(enemy.position, b.anchor))
                        });
                    if let Some(next) = next {
                        area = next;
                        self.incidents.remove(&enemy.id);
                        self.changed_incident = true;
                    }
                }
            }
            let anchor = area.anchor;
            let incident = self.incidents.entry(enemy.id).or_insert_with(|| Incident {
                anchor,
                members: area.members.clone(),
                // The original defended post fixes the incident's pursuit bound. Direct
                // targeting also admits a distant shooter, but never expands its initial leash.
                leash: envelope(frame, area, range).max(distance(anchor, enemy.position)),
                position: enemy.position,
                range,
                target: enemy.target,
                value: observed_value(book, enemy),
                kind: enemy.kind,
                seen: frame.tick(),
                active: true,
            });
            incident.position = enemy.position;
            incident.range = range;
            incident.target = enemy.target;
            incident.value = observed_value(book, enemy);
            incident.kind = enemy.kind;
            incident.seen = frame.tick();
            incident.active = distance(enemy.position, incident.anchor) <= incident.leash;
            // Retain a visible leashed-out incident even though it no longer contributes value.
            // Otherwise the next decision would re-admit it with a longer pursuit leash.
        }
        let q = crate::sdk::WorldQueries::new(frame);
        for (id, incident) in &mut self.incidents {
            let visible = frame.visible_enemies().iter().any(|enemy| enemy.id == *id);
            let relevant = relevant_area(
                frame,
                incident.position,
                incident.range,
                incident.target,
                areas,
            )
            .is_some();
            let leashed = distance(incident.position, incident.anchor) <= incident.leash;
            let observed_empty = !visible
                && q.world_point(incident.position.0, incident.position.1)
                    .is_some_and(|point| q.ground_position_is_observable(point));
            if !relevant || !leashed || observed_empty {
                incident.active = false;
            }
            // Cleared ground releases responders, but keep the bounded incident identity until
            // its visibility memory expires. Reappearing beyond the leash cannot restart chase.
        }
        self.incidents
            .iter()
            .filter(|(_, incident)| incident.active)
            .map(|(id, incident)| {
                (
                    *id,
                    Contact {
                        position: incident.position,
                        value: incident.value,
                        kind: incident.kind,
                        seen: incident.seen,
                    },
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
