use super::*;

pub(super) fn uses_home_rifle_coverage(profile_id: &str) -> bool {
    matches!(
        profile_id,
        JEFFS_AI_ID
            | JEFFS_AI_BETA_ID
            | JEFFS_AI_PRE_DEFENSE_ENVELOPE_ID
            | JEFFS_AI_PRE_TANK_CATCHUP_ID
            | JEFFS_AI_PRE_OPENING_RUSH_ID
            | JEFFS_AI_PRE_TANK_MICRO_ID
            | JEFFS_AI_PRE_GUARD_HANDOFF_ID
            | JEFFS_AI_PRE_THIRD_PRESSURE_ID
            | JEFFS_AI_PRE_SAFE_BASES_ID
            | JEFFS_AI_PRE_HELD_SITE_ID
            | JEFFS_AI_PRE_EARLY_NATURAL_ID
            | JEFFS_AI_PRE_FAST_NATURAL_ID
            | JEFFS_AI_PRE_SPEND_PLAN_ID
    )
}

pub(super) fn uses_current_jeff_defense(profile_id: &str) -> bool {
    matches!(
        profile_id,
        JEFFS_AI_ID
            | JEFFS_AI_BETA_ID
            | JEFFS_AI_PRE_TANK_CATCHUP_ID
            | JEFFS_AI_PRE_OPENING_RUSH_ID
            | JEFFS_AI_PRE_TANK_MICRO_ID
            | JEFFS_AI_PRE_GUARD_HANDOFF_ID
            | JEFFS_AI_PRE_THIRD_PRESSURE_ID
            | JEFFS_AI_PRE_SAFE_BASES_ID
            | JEFFS_AI_PRE_HELD_SITE_ID
            | JEFFS_AI_PRE_EARLY_NATURAL_ID
            | JEFFS_AI_PRE_FAST_NATURAL_ID
            | JEFFS_AI_PRE_SPEND_PLAN_ID
    )
}

/// Jeff-only mirror of the server's build-site rule for resource nodes: a node body is a half-tile
/// circle and any footprint it touches is rejected. The shared placement query checks only the
/// node's center tile, so on Classic Jeff kept choosing a natural the server refused because two
/// Steel centers sat just outside the footprint edge. Pump Jacks and Steel Mines are exempt, as
/// in the simulation. Kept out of the shared query so other profiles are unaffected.
pub(super) fn resource_body_blocks_site(
    observation: &AiObservation,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> bool {
    if matches!(building, EntityKind::PumpJack | EntityKind::SteelMine) {
        return false;
    }
    let Some(stats) = config::building_stats(building) else {
        return false;
    };
    let ts = observation.map.tile_size as f32;
    let footprint = (
        tile_x as f32 * ts,
        tile_y as f32 * ts,
        tile_x.saturating_add(stats.foot_w) as f32 * ts,
        tile_y.saturating_add(stats.foot_h) as f32 * ts,
    );
    observation.resources.iter().any(|resource| {
        crate::sdk::unit_circle_touches_rect((resource.x, resource.y), ts * 0.5, footprint)
    })
}

/// Jeff's producers send fresh combat units to a safe forward staging point immediately.
/// The normal frontal and defense planners remain authoritative and can redirect them on
/// the next think; this route only removes the idle interval at the production building.
pub(super) fn production_rally(observation: &AiObservation, facts: &AiFacts) -> Option<(f32, f32)> {
    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    let enemy_base = facts.nearest_public_enemy_base?;
    let direction = normalized_direction(own_base, (enemy_base.x, enemy_base.y))?;
    let forward_distance = observation.map.tile_size as f32 * 8.0;
    Some(clamp_to_map(
        (
            own_base.0 + direction.0 * forward_distance,
            own_base.1 + direction.1 * forward_distance,
        ),
        observation.map,
    ))
}

/// Riflemen are permanent home-screen units. Rally them to the base-centric defensive anchor so
/// they do not walk through the forward army staging lane before receiving a stable slot.
pub(super) fn rifleman_home_rally(
    observation: &AiObservation,
    facts: &AiFacts,
) -> Option<(f32, f32)> {
    let anchor = defense::main_steel_cluster_center(observation)
        .unwrap_or_else(|| tile_center(observation.own_start_tile, observation.map.tile_size));
    let enemy_base = facts.nearest_public_enemy_base?;
    let direction = normalized_direction(anchor, (enemy_base.x, enemy_base.y))?;
    let distance = observation.map.tile_size as f32 * 3.5;
    Some(clamp_to_map(
        (
            anchor.0 + direction.0 * distance,
            anchor.1 + direction.1 * distance,
        ),
        observation.map,
    ))
}

/// The main-base buildings Jeff keeps behind its HQ on Crossroads.
pub(super) fn crossroads_sheltered_kind(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Barracks
            | EntityKind::TrainingCentre
            | EntityKind::EngineeringComplex
            | EntityKind::Factory
    )
}

/// On Crossroads raids walk straight into the first building on their way to the HQ. Every
/// Factory placed ahead of the HQ on the enemy's walk in (7-8 tiles nearer the enemy than the HQ)
/// was destroyed, usually more than once, while each main has room behind the HQ, walled off by
/// water, that the enemy can only reach by walking past it. This is the site's `site_depth`;
/// `None` for other buildings, other maps and without map analysis.
pub(super) fn crossroads_site_depth(
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> Option<f32> {
    if !crossroads_sheltered_kind(kind)
        || defense::crossroads_wall_aware_approach_direction(observation).is_none()
    {
        return None;
    }
    site_depth(observation, analysis?, kind, tile_x, tile_y)
}

/// How many tiles farther the nearest enemy walks to a `kind` building at this site than to Jeff's
/// HQ: negative ahead of the HQ, positive behind it.
pub(super) fn site_depth(
    observation: &AiObservation,
    analysis: &AiMapAnalysis,
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> Option<f32> {
    let tile_size = observation.map.tile_size.max(1);
    let center = geometry::building_center((tile_x, tile_y), kind, tile_size)?;
    let center_tile = (
        (center.0 / tile_size as f32) as u32,
        (center.1 / tile_size as f32) as u32,
    );
    let enemy_walk = |tile: (u32, u32)| {
        observation
            .players
            .iter()
            .filter(|player| player.is_alive && observation.is_enemy_player(player.id))
            .filter_map(|player| analysis.ground_distance_from_start(player.id, tile))
            .min_by(f32::total_cmp)
    };
    Some(enemy_walk(center_tile)? - enemy_walk(observation.own_start_tile)?)
}

/// Jeff's home Riflemen while no raid is being answered: the current Jeff's defensive pocket, or
/// a frozen profile's coverage or screen. `assigned`: units home defense already ordered, and units
/// the route line holds.
pub(super) fn stage_home_riflemen(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &AiDecisionMemory,
    profile: &AiProfile,
    facts: &AiFacts,
    (local_defense_assigned, route_line_reserved): (&BTreeSet<u32>, &BTreeSet<u32>),
) {
    let riflemen: Vec<u32> = if uses_home_rifle_coverage(profile.id) {
        observation
            .owned
            .iter()
            .filter(|unit| {
                unit.kind == EntityKind::Rifleman
                    && unit.is_complete
                    && unit.hp > 0
                    && !local_defense_assigned.contains(&unit.id)
                    && !memory.containment.active_riflemen.contains(&unit.id)
                    && !memory.expansion_security.riflemen.contains(&unit.id)
                    && !route_line_reserved.contains(&unit.id)
            })
            .map(|unit| unit.id)
            .collect()
    } else {
        actions::select_ready_combat_units(&observation.owned, &[EntityKind::Rifleman])
            .into_iter()
            .filter(|id| !local_defense_assigned.contains(id))
            .collect()
    };
    let own_base = geometry::tile_center(observation.own_start_tile, observation.map.tile_size);
    let fallback_armor = observation
        .owned
        .iter()
        .filter(|entity| {
            entity.is_complete
                && matches!(entity.kind, EntityKind::Tank | EntityKind::ScoutCar)
                && !memory.containment.active_tanks.contains(&entity.id)
                && memory.containment.active_scout != Some(entity.id)
        })
        .min_by(|left, right| {
            geometry::dist2(left.x, left.y, own_base.0, own_base.1)
                .total_cmp(&geometry::dist2(right.x, right.y, own_base.0, own_base.1))
        })
        .map(|entity| entity.id);
    if let Some(enemy_base) = facts.nearest_public_enemy_base {
        let staged = if uses_current_jeff_defense(profile.id) {
            stage_home_defensive_pocket_riflemen(
                actions,
                observation,
                map_analysis,
                &riflemen,
                enemy_base,
                memory.opening_rush.missed_contact(),
            )
        } else if profile.id == JEFFS_AI_PRE_DEFENSE_ENVELOPE_ID {
            defense::stage_home_rifleman_coverage(
                actions,
                observation,
                map_analysis,
                &riflemen,
                enemy_base,
            )
        } else if profile.id == JEFFS_AI_PRE_RIFLE_COVERAGE_ID {
            memory
                .home_defensive_tank
                .or(fallback_armor)
                .and_then(|armor_id| {
                    stage_home_rifleman_screen(
                        actions,
                        observation,
                        &riflemen,
                        armor_id,
                        enemy_base,
                        3.0,
                        1.75,
                    )
                })
        } else {
            None
        };
        if let Some(units) = staged {
            intents.push(AiIntent::Stage { units });
        }
    }
}

/// Last: the current Jeff's resting Tanks with no other orders gather on the home post, so none
/// sits idle in a corner of the main or out on the map. `excluded` holds units already ordered by
/// home defense or reserved elsewhere in the decision; returns the Tanks sent.
pub(super) fn gather_resting_tanks(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    mut excluded: BTreeSet<u32>,
) -> Vec<u32> {
    excluded.extend(memory.home_defensive_tank);
    excluded.extend(memory.later_bases.guards.iter().copied());
    excluded.extend(memory.route_line.picket());
    let ordered: BTreeSet<u32> = actions
        .unit_orders_since(0)
        .into_iter()
        .map(|(unit, _)| unit)
        .collect();
    let covered_sites: Vec<(f32, f32)> = [
        memory.later_bases.covered_site(),
        memory.expansion_security.site,
    ]
    .into_iter()
    .flatten()
    .filter_map(|site| {
        geometry::building_center(site, EntityKind::ResourceDepot, observation.map.tile_size)
    })
    .collect();
    home_armor::gather_resting_tanks(
        actions,
        observation,
        memory,
        &excluded,
        &ordered,
        &covered_sites,
    )
}

/// Whether the current Jeff's builder may go for the natural now. `secured`: the site is secured or
/// off the enemy's way in. With the live Jeff's spend plan the Depot must be paid for by the
/// builder's arrival; otherwise its cost must be banked.
pub(super) fn natural_attempt_ready(
    (observation, map_analysis, memory): (
        &AiObservation,
        Option<&AiMapAnalysis>,
        &AiDecisionMemory,
    ),
    profile_id: &str,
    secured: bool,
    spend_plan: Option<&spend_plan::SpendPlan>,
) -> bool {
    match spend_plan {
        Some(plan) => expansion_security::natural_attempt_ready_funded(
            observation,
            map_analysis,
            memory,
            secured,
            !runs_live_jeff(profile_id),
            plan.natural_funded_on_arrival,
        ),
        None => expansion_security::natural_attempt_ready(
            observation,
            map_analysis,
            memory,
            secured,
            !runs_live_jeff(profile_id),
        ),
    }
}

/// The current Jeff's launched push keeps its units: home defense answers raids with what stayed
/// home, and the push keeps its own orders meanwhile. It used to lose all but two Tanks to home
/// defense within moments of leaving.
pub(super) fn push_units(profile_id: &str, memory: &AiDecisionMemory) -> BTreeSet<u32> {
    if uses_current_jeffs_ai_policy(profile_id)
        && memory.containment.wave_launched
        && !memory.containment.recovery_active
    {
        memory
            .containment
            .active_tanks
            .iter()
            .copied()
            .chain(memory.containment.active_scout)
            .chain(memory.containment.active_riflemen.iter().copied())
            .collect()
    } else {
        BTreeSet::new()
    }
}

/// The live Jeff builds at home with the Engineer nearest its HQ. By id the first pick was the
/// natural's builder, which stands at the natural once a second Engineer is home.
pub(super) fn home_builders(
    observation: &AiObservation,
    profile_id: &str,
    idle_builders: &[u32],
) -> Vec<u32> {
    let mut home_builders = idle_builders.to_vec();
    if runs_live_jeff(profile_id) {
        let home = tile_center(observation.own_start_tile, observation.map.tile_size);
        let from_home = |id: &u32| {
            observation
                .owned
                .iter()
                .find(|unit| unit.id == *id)
                .map_or(f32::MAX, |unit| {
                    geometry::dist2(unit.x, unit.y, home.0, home.1)
                })
        };
        home_builders.sort_by(|a, b| from_home(a).total_cmp(&from_home(b)).then(a.cmp(b)));
    }
    home_builders
}

/// The live Jeff sends the builder to a natural off the enemy's way in without waiting for its
/// guards to secure it: they follow it out, and when the natural is due (about 1:40) there are
/// no spare Riflemen to send first.
pub(super) fn natural_sheltered(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &AiDecisionMemory,
    profile_id: &str,
) -> bool {
    runs_live_jeff(profile_id)
        && map_analysis
            .zip(memory.expansion_security.site)
            .is_some_and(|(analysis, site)| {
                expansion::site_on_enemy_route(
                    observation,
                    analysis,
                    EntityKind::ResourceDepot,
                    site,
                ) == Some(false)
            })
}

/// The live Jeff's spend plan (decision/spend_plan.rs): the income forecast and the opening's
/// costly steps laid out from it. The natural's builder goes once the Depot will be paid for by
/// its arrival, and Steel the steps do not need is the only Steel routine Riflemen and a second
/// Barracks may take. `secured`: the natural's site is secured or off the enemy's way in.
#[allow(clippy::too_many_arguments)]
pub(super) fn live_spend_plan(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
    memory: &AiDecisionMemory,
    map_analysis: Option<&AiMapAnalysis>,
    actions: &AiActionContext<'_>,
    expansion_builder_pools: &[&[u32]],
    natural_pending: bool,
    secured: bool,
) -> Option<spend_plan::SpendPlan> {
    (profile.id == JEFFS_AI_ID).then(|| {
        let site = memory.expansion_security.site;
        let natural_builder = site
            .and_then(|site| {
                observation
                    .pending_builds
                    .iter()
                    .find(|intent| {
                        intent.kind == EntityKind::ResourceDepot
                            && (intent.tile_x, intent.tile_y) == site
                    })
                    .map(|intent| intent.worker_id)
            })
            .or_else(|| {
                expansion_builder_pools
                    .iter()
                    .flat_map(|pool| pool.iter())
                    .copied()
                    .find(|id| !actions.reservations().worker_reserved(*id))
            });
        spend_plan::plan(
            observation,
            facts,
            &memory.spend_plan,
            spend_plan::PlanInput {
                natural_due: !natural_pending
                    && expansion_security::expansion_is_next(observation, facts, profile),
                natural_site: site,
                natural_builder,
                natural_wait: if secured {
                    0
                } else {
                    expansion_security::secure_wait_ticks(observation, map_analysis, memory)
                        .unwrap_or(config::TICK_HZ * 30)
                },
            },
        )
    })
}

/// The Engineers the live Jeff wants and has: `(target, count)`.
///
/// It trains a second Engineer once its natural is ordered ahead of its Factory, so the Factory
/// does not wait for the natural's builder to walk back. With the Factory already out the Steel
/// goes elsewhere: on Chokes, where Steel is short, it delayed Tank Unlock 23 s.
///
/// It counts the Engineers in training: with the natural's Depot up as well, each Depot trained
/// one, and Fastest Map Possible got three Engineers for the one wanted.
pub(super) fn engineers(
    observation: &AiObservation,
    facts: &AiFacts,
    profile_id: &str,
    target_workers: usize,
) -> (usize, usize) {
    let target_workers = if runs_live_jeff(profile_id)
        && facts.building_count(EntityKind::ResourceDepot) >= 2
        && facts.building_count(EntityKind::Factory) == 0
    {
        target_workers.max(2)
    } else {
        target_workers
    };
    let workers = facts.worker_count
        + if profile_id == JEFFS_AI_ID {
            observation
                .owned
                .iter()
                .filter(|depot| {
                    depot.kind == EntityKind::ResourceDepot
                        && depot.production_kind == Some(EntityKind::Worker)
                })
                .map(|depot| depot.production_queue_len.unwrap_or(0))
                .sum::<usize>()
        } else {
            0
        };
    (target_workers, workers)
}

/// Riflemen recruited ahead of tech for the natural, up to six (in training included), placed in
/// `priorities`. While the live Jeff's opening guns are short before its first Tank, they come
/// after the next gun: ahead of it they took the Barracks from guns three and four once the
/// starters were lost (Fastest Map Possible: two guns home at 3:00). A Rifleman still comes while
/// the gun waits for Oil; on Wald the Factory takes it first, and the Rifleman that came then held
/// the 2:12 raid.
pub(super) fn security_rifle_target(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
    memory: &AiDecisionMemory,
    priorities: &mut Vec<EntityKind>,
) -> Option<usize> {
    let security_rifle_target = (uses_current_jeffs_ai_policy(profile.id)
        && memory.expansion_security.site.is_some()
        && facts.unit_count(EntityKind::Rifleman) < 6)
        .then_some(6);
    if security_rifle_target.is_some() {
        // Once the live Jeff's natural is ordered, its fifth and sixth guns come first too. They
        // used to be bought with the Steel the builder's walk left spare, and a builder sent ahead
        // of its money leaves none: 1v1 No Terrain s2p2 met its 2:24 raid with four guns and fell.
        let gun_target =
            if profile.id == JEFFS_AI_ID && facts.building_count(EntityKind::ResourceDepot) >= 2 {
                profile
                    .defensive_machine_gunners
                    .map_or(expansion_security::OPENING_GUNS, |policy| {
                        policy.target_count
                    })
            } else {
                expansion_security::OPENING_GUNS
            };
        let guns_first = runs_live_jeff(profile.id)
            && facts.unit_count(EntityKind::Tank) == 0
            && !expansion_security::opening_guns_ordered(observation, facts, gun_target);
        let at = priorities
            .iter()
            .position(|unit| *unit == EntityKind::MachineGunner)
            .filter(|_| guns_first)
            .map_or(0, |gun| gun + 1);
        priorities.insert(at, EntityKind::Rifleman);
    }
    security_rifle_target
}

/// The live Jeff's two extra guns are opening guns: once a Tank is out, lost guns are replaced
/// only up to the usual four, and the Oil goes to Tanks.
pub(super) fn cap_opening_guns(
    profile_id: &str,
    facts: &AiFacts,
    max_counts: &mut [(EntityKind, usize)],
) {
    if runs_live_jeff(profile_id) && facts.unit_count(EntityKind::Tank) > 0 {
        for (kind, max) in max_counts {
            if *kind == EntityKind::MachineGunner {
                *max = (*max).min(expansion_security::OPENING_GUNS);
            }
        }
    }
}

/// The live Jeff's routine Riflemen come only from Steel its spend plan leaves spare; raid
/// answers and a defensive panic are not routine.
pub(super) fn cap_routine_riflemen(
    plan: Option<&spend_plan::SpendPlan>,
    routine: bool,
    intents: &[AiIntent],
    counts: &[(EntityKind, usize)],
    max_counts: &mut Vec<(EntityKind, usize)>,
) {
    if let Some(cap) = plan
        .filter(|_| routine)
        .and_then(|plan| plan.rifle_cap(intents, counts))
    {
        match max_counts
            .iter_mut()
            .find(|(kind, _)| *kind == EntityKind::Rifleman)
        {
            Some((_, max)) => *max = (*max).min(cap),
            None => max_counts.push((EntityKind::Rifleman, cap)),
        }
    }
}
