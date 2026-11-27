use super::*;
use std::collections::VecDeque;
pub(super) struct Asset {
    point: usize,
    base: AiBaseSiteId,
    pub(super) value: f32,
    known: f32,
    pub(super) unknown: f32,
    mine: f32,
    producer: f32,
    tech: f32,
}
fn steel_value(remaining: f32, active: bool) -> f32 {
    remaining.min(270.) / 100. * if active { 1. } else { 0.15 }
        + if active && remaining > 0. { 0.25 } else { 0. }
        + 0.15 * remaining / 625.
}
pub(super) fn assets(g: &Grid, frame: &AiFrame, ids: &[AiBaseSiteId]) -> Vec<Asset> {
    let mut out = vec![];
    for site in frame.base_sites().iter().filter(|b| ids.contains(&b.id)) {
        for r in frame
            .resources()
            .iter()
            .filter(|r| r.kind == EntityKind::Steel && site.resource_ids.contains(&r.id))
        {
            let active = frame.owned().iter().any(|e| {
                e.kind == EntityKind::SteelMine
                    && e.completion == AiCompletion::Complete
                    && e.extractor_active == Some(true)
                    && (e.latched_resource == Some(r.id) || dist2(e.position, r.position) < 1.)
            });
            let (remaining, known, unknown) = match r.remaining {
                AiResourceAmount::Known(n) => (n as f32, n as f32, 0.),
                AiResourceAmount::Unknown => (312.5, 0., 1.),
            };
            if let Some(point) = g.near(r.position) {
                out.push(Asset {
                    point,
                    base: site.id,
                    value: steel_value(remaining, active) * if unknown > 0. { 0.5 } else { 1. },
                    known,
                    unknown,
                    mine: if active { 1. } else { 0. },
                    producer: 0.,
                    tech: 0.,
                });
            }
        }
    }
    for e in frame
        .owned()
        .iter()
        .filter(|e| e.completion == AiCompletion::Complete)
    {
        let (value, producer, tech) = match e.kind {
            EntityKind::ResourceDepot => (0.5, 0., 0.),
            EntityKind::Barracks => (3., 1., 0.),
            EntityKind::Factory => (4.5, 1., 0.),
            EntityKind::Portal => (4.5, 1., 0.),
            EntityKind::TrainingCentre => (1.5, 0., 1.),
            EntityKind::Steelworks => (2.5, 0., 1.),
            _ => continue,
        };
        let base = frame
            .base_sites()
            .iter()
            .filter(|b| ids.contains(&b.id))
            .min_by(|a, b| {
                dist2(a.position, e.position)
                    .total_cmp(&dist2(b.position, e.position))
                    .then_with(|| a.id.cmp(&b.id))
            });
        if let (Some(b), Some(point)) = (base, g.near(e.position)) {
            out.push(Asset {
                point,
                base: b.id,
                value,
                known: 0.,
                unknown: 0.,
                mine: 0.,
                producer,
                tech,
            });
        }
    }
    out
}
fn occupied(g: &Grid, frame: &AiFrame) -> Vec<bool> {
    let radius = rts_rules::balance::unit_placement_radius(EntityKind::MachineGunner);
    let buildings = frame
        .owned()
        .iter()
        .chain(frame.visible_allies())
        .chain(frame.visible_enemies())
        .chain(frame.tank_traps())
        .filter_map(|e| rts_rules::balance::building_stats(e.kind).map(|s| (e, s)))
        .collect::<Vec<_>>();
    (0..g.walk.len())
        .map(|i| {
            let p = g.point(i);
            frame.resources().iter().any(|r| {
                r.remaining != AiResourceAmount::Known(0)
                    && dist2(p, r.position) < (1.7 * g.size).det_powi(2)
            }) || buildings.iter().any(|(e, s)| {
                let hw = s.foot_w as f32 * g.size * 0.5 + radius;
                let hh = s.foot_h as f32 * g.size * 0.5 + radius;
                (p.0 - e.position.0).abs() <= hw && (p.1 - e.position.1).abs() <= hh
            })
        })
        .collect()
}
pub(super) fn score(planner: &mut DefensePlanner, frame: &AiFrame, budget: usize) -> DefensePlan {
    let Some(g) = planner.grid.as_ref() else {
        return DefensePlan {
            unallocated_budget: budget,
            ..Default::default()
        };
    };
    let assets = assets(g, frame, &planner.owned);
    // Keep fields only for current economic assets; a long match with repeatedly relocated
    // production cannot accumulate one full-map distance buffer per historical placement.
    let current_points: BTreeSet<_> = assets.iter().map(|a| a.point).collect();
    planner
        .time_fields
        .retain(|point, _| current_points.contains(point));
    let blocked = occupied(g, frame);
    // Dynamic reachability includes known occupied footprints. Start outside each owned Depot's
    // footprint, never flood through the building center. Reservations ignore transient unit bodies.
    let starts = frame
        .base_sites()
        .iter()
        .filter(|b| planner.owned.contains(&b.id))
        .flat_map(|b| {
            (0..g.walk.len()).filter(|i| {
                g.walk[*i]
                    && !blocked[*i]
                    && dist2(g.point(*i), b.position) <= 25. * g.size * g.size
            })
        })
        .collect::<Vec<_>>();
    let mut reached = vec![false; g.walk.len()];
    let mut q = VecDeque::new();
    for i in starts {
        if !reached[i] {
            reached[i] = true;
            q.push_back(i);
        }
    }
    while let Some(i) = q.pop_front() {
        for (j, _) in g.neighbors(i) {
            let (x, y) = g.tile(i);
            let (xx, yy) = g.tile(j);
            let corners_clear = x == xx
                || y == yy
                || (!blocked[y as usize * g.w + xx as usize]
                    && !blocked[yy as usize * g.w + x as usize]);
            if !blocked[j] && corners_clear && !reached[j] {
                reached[j] = true;
                q.push_back(j);
            }
        }
    }
    let range = planner.firing_range_tiles.unwrap_or_else(|| {
        rts_rules::balance::unit_stats(EntityKind::MachineGunner)
            .map_or(0., |s| s.range_tiles)
            .max(0.)
            - 0.2
    });
    // Representative infantry approach speed derives from the same authoritative MG stats.
    // Road/slow terrain is already included in the integer time-equivalent distance field.
    let approach_speed = rts_rules::balance::unit_stats(EntityKind::MachineGunner)
        .map_or(1.0, |s| {
            s.speed * rts_rules::balance::TICK_HZ as f32 / g.size
        })
        .max(0.01);
    let mut candidates = vec![];
    let mut sectors = vec![];
    let mut weights = vec![];
    for (id, geo) in planner.geometry.iter().enumerate() {
        let center = g.point(geo.center);
        let slots = (0..g.walk.len())
            .filter(|i| g.entrench[*i] && !blocked[*i] && reached[*i])
            .filter(|i| {
                geo.points.iter().any(|p| {
                    dist2(g.point(*i), g.point(*p)) <= range * range * g.size * g.size
                        && g.ray_blocked(*i, *p, &blocked)
                })
            })
            .collect::<Vec<_>>();
        let eta = geo
            .points
            .iter()
            .map(|i| planner.enemy[*i])
            .min()
            .filter(|d| *d < FAR)
            .map(|d| d as f32 / 120. / approach_speed);
        candidates.push(slots);
        sectors.push(DefenseSector {
            id,
            mode: geo.mode,
            base_ids: geo.bases.clone(),
            cut_tiles: if geo.mode == DefenseMode::Cut {
                geo.points.iter().map(|i| g.tile(*i)).collect()
            } else {
                vec![]
            },
            center,
            asset_value: 0.,
            priority: 0.,
            enemy_approach_seconds: eta,
            known_remaining_steel: 0.,
            unknown_resource_count: 0.,
            active_mine_share: 0.,
            production_share: 0.,
            tech_share: 0.,
            supported: !candidates[id].is_empty(),
            slots: vec![],
        });
        weights.push(0.);
    }
    for a in assets {
        let da = planner
            .time_fields
            .entry(a.point)
            .or_insert_with(|| g.field(&[a.point], true));
        let via = planner
            .geometry
            .iter()
            .map(|s| {
                if s.bases.contains(&a.base) {
                    s.points
                        .iter()
                        .filter(|p| planner.enemy[**p] < FAR && da[**p] < FAR)
                        .map(|p| planner.enemy[*p].saturating_add(da[*p]))
                        .min()
                        .unwrap_or(FAR)
                } else {
                    FAR
                }
            })
            .collect::<Vec<_>>();
        let Some(best) = via.iter().copied().filter(|v| *v < FAR).min() else {
            continue;
        };
        let shares = via
            .iter()
            .map(|v| {
                if *v < FAR && v.saturating_sub(best) <= 9600 {
                    (-((*v - best) as f32) / 120. / 20.).det_exp()
                } else {
                    0.
                }
            })
            .collect::<Vec<_>>();
        let sum: f32 = shares.iter().sum();
        if sum == 0. {
            continue;
        }
        for (s, share) in sectors.iter_mut().zip(shares) {
            let share = share / sum;
            s.asset_value += a.value * share;
            s.known_remaining_steel += a.known * share;
            s.unknown_resource_count += a.unknown * share;
            s.active_mine_share += a.mine * share;
            s.production_share += a.producer * share;
            s.tech_share += a.tech * share;
        }
    }
    for (w, s) in weights.iter_mut().zip(&sectors) {
        *w = s.enemy_approach_seconds.map_or(0., |seconds| {
            s.asset_value / (1. + seconds / 30.).det_powi(3)
        });
    }
    let total: f32 = weights.iter().sum();
    for (s, w) in sectors.iter_mut().zip(&weights) {
        s.priority = if total > 0. { *w / total } else { 0. };
    }
    let mut plan = DefensePlan {
        sectors,
        slots: vec![],
        unallocated_budget: budget,
    };
    let mut used = vec![];
    // Allocate only if a physically supported, unoccupied candidate remains. Highest averages
    // gives a meaningful choice even with one unit or many bases, with no per-base minimum.
    for _ in 0..budget.min(g.walk.len()) {
        let selected = (0..plan.sectors.len())
            .filter(|i| weights[*i] > 0. && !candidates[*i].is_empty())
            .max_by(|a, b| {
                let qa = weights[*a] / (plan.sectors[*a].slots.len() + 1) as f32;
                let qb = weights[*b] / (plan.sectors[*b].slots.len() + 1) as f32;
                qa.total_cmp(&qb).then_with(|| b.cmp(a))
            });
        let Some(si) = selected else {
            break;
        };
        let geo = &planner.geometry[si];
        let sample = if plan.sectors[si].slots.is_empty() {
            geo.center
        } else {
            *geo.points
                .iter()
                .max_by(|a, b| {
                    let quality = |p: usize| {
                        plan.sectors[si]
                            .slots
                            .iter()
                            .map(|s| dist2(g.point(p), s.position))
                            .fold(f32::INFINITY, f32::min)
                    };
                    quality(**a).total_cmp(&quality(**b)).then_with(|| b.cmp(a))
                })
                .unwrap_or(&geo.center)
        };
        let sample = g.point(sample);
        let want = if geo.mode == DefenseMode::Cut {
            let base = frame
                .base_sites()
                .iter()
                .filter(|b| geo.bases.contains(&b.id))
                .min_by(|a, b| dist2(a.position, sample).total_cmp(&dist2(b.position, sample)));
            if let Some(b) = base {
                let dx = b.position.0 - sample.0;
                let dy = b.position.1 - sample.1;
                let len = dx.det_hypot(dy).max(1.);
                (
                    sample.0 + 2. * g.size * dx / len,
                    sample.1 + 2. * g.size * dy / len,
                )
            } else {
                sample
            }
        } else {
            g.point(geo.center)
        };
        let Some(slot) = candidates[si].iter().copied().min_by(|a, b| {
            dist2(g.point(*a), want)
                .total_cmp(&dist2(g.point(*b), want))
                .then_with(|| a.cmp(b))
        }) else {
            continue;
        };
        let position = g.point(slot);
        used.push(slot);
        let entry = DefenseSlot {
            position,
            sector_id: si,
        };
        plan.slots.push(entry);
        plan.sectors[si].slots.push(entry);
        for c in &mut candidates {
            c.retain(|i| dist2(g.point(*i), position) > 1.4 * 1.4 * g.size * g.size);
        }
    }
    plan.unallocated_budget = budget.saturating_sub(plan.slots.len());
    plan.slots.sort_by(|a, b| {
        plan.sectors[b.sector_id]
            .priority
            .total_cmp(&plan.sectors[a.sector_id].priority)
            .then_with(|| a.sector_id.cmp(&b.sector_id))
    });
    plan
}
