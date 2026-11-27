//! `ai-pf-lab`: Panzerfaust tactics against Tanks, one engagement at a time (local analysis tool).
//!
//! Each fight is a Lab game on an open map: two groups spawn a gap apart. The opponent's Tanks
//! (and escorts) are scripted: `attack` attack-moves at the candidate (re-aimed at what it sees),
//! `hold` parks where it spawned. The candidate side runs one Panzerfaust tactic:
//!   amove      everything attack-moves at the enemy (naive; spent Panzerfausts keep fighting)
//!   hold       stand still where spawned (with Entrenchment they dig in after 3 s)
//!   line:S     form a line S tiles apart across the enemy direction, then stand still
//!   charge:T   stand (or walk in vs a parked enemy); once a Tank is seen within T tiles every
//!              loaded Panzerfaust attacks the nearest Tank
//!   split:T    as charge, but Panzerfausts are shared over the Tanks and come in from spread
//!              angles (waypoint 4.5 tiles from the Tank, then attack)
//!   wide:T     as split, but they first fan out 10 tiles from the Tank (outside its sight) and
//!              all go in together once in place (or after 8 s)
//!   ambush:T   stand still; a Panzerfaust attacks a Tank only once it is within T tiles of it
//! Every tactic but amove walks spent Panzerfausts home. Riflemen and Tanks on the candidate side
//! hold in the standing tactics and attack-move once a charge starts.
//! Score: value lost (steel+oil, wounded by HP share) per side, Tanks killed, rockets fired.
//!
//! Usage: ai-pf-lab --scenarios FILE|"name ours theirs opp[;...]" --tactics "hold;charge:11"
//!        [--seeds 6] [--gap 20] [--ticks 2700] [--trench 1] [--meth 1] [--threads 8]

use std::collections::{BTreeMap, BTreeSet};
use std::process;
use std::sync::{Arc, Mutex};

use rayon::prelude::*;
use rts_rules::math::GameplayMath as _;
use rts_sim::game::entity::EntityKind;
use rts_sim::game::lab::{LabOp, LabSetCompletedResearch, LabSetPlayerResources, LabSpawnEntity};
use rts_sim::game::map::Map;
use rts_sim::game::upgrade::UpgradeKind;
use rts_sim::game::{Game, PlayerInit};

use rts_sim::protocol::Event;

use crate::ai_core::observation::{AiEntityState, AiEntitySummary, AiObservation};
use crate::live::{AiAlivePolicy, AiController, CanonicalAiTickDriver};
use crate::sdk::{AiActions, AiFrame, AiStrategy, UnitGroup};

const TILE_PX: f32 = rts_rules::balance::TILE_SIZE as f32;
const MAP_SEED: u32 = 1;

#[derive(Clone, Debug)]
struct Scenario {
    name: String,
    ours: Vec<(EntityKind, usize)>,
    theirs: Vec<(EntityKind, usize)>,
    hold: bool,
    gap: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    AMove,
    Hold,
    Line,
    Charge,
    Split,
    Wide,
    Ambush,
}

#[derive(Clone, Debug)]
struct Tactic {
    label: String,
    kind: Kind,
    p: f32,
    /// `screen`: Riflemen run ahead to 2 tiles short of the Tanks; Panzerfausts follow 1.5 s later.
    screen: bool,
    /// `spread`: Panzerfausts close in line abreast, 2.2 tiles apart, so one HE shell hits one.
    spread: bool,
    /// `fan`: each Panzerfaust walks straight to its own point 4.5 tiles from the Tank, 2.2 tiles
    /// apart across the approach (spread while walking, no stop out of range), then attacks.
    fan: bool,
}

/// Damage the candidate's infantry took, by cause.
#[derive(Clone, Debug, Default)]
struct Hurt {
    pf_cannon: f32,
    pf_splash: f32,
    pf_coax: f32,
    pf_other: f32,
    rifle_cannon: f32,
    rifle_splash: f32,
    /// Tank cannon shots at the candidate's Panzerfausts / Riflemen.
    shots_pf: usize,
    shots_rifle: usize,
    /// HE shells that hurt 2+ Panzerfausts.
    multi_splash: usize,
    /// Panzerfausts that died before firing.
    pf_dead_loaded: usize,
}

#[derive(Clone, Debug, Default)]
struct FightResult {
    hurt: Hurt,
    our_lost: f32,
    their_lost: f32,
    our_dead: usize,
    their_dead: usize,
    our_alive: usize,
    their_alive: usize,
    tanks_killed: usize,
    tanks: usize,
    fired: usize,
    fausts: usize,
}

fn parse_side(text: &str) -> Result<Vec<(EntityKind, usize)>, String> {
    text.split('+')
        .map(|part| {
            let (kind, count) = part.split_once(':').unwrap_or((part, "1"));
            let kind: EntityKind = kind
                .parse()
                .map_err(|_| format!("unknown unit kind {kind:?}"))?;
            let count: usize = count
                .parse()
                .map_err(|_| format!("bad count in {part:?}"))?;
            Ok((kind, count))
        })
        .collect()
}

fn parse_scenarios(text: &str) -> Result<Vec<Scenario>, String> {
    let body = if std::path::Path::new(text).is_file() {
        std::fs::read_to_string(text).map_err(|err| err.to_string())?
    } else {
        text.replace(';', "\n")
    };
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 4 {
                return Err(format!("bad scenario line {line:?}"));
            }
            let mut gap = None;
            for extra in &fields[4..] {
                if let Some(v) = extra.strip_prefix("gap=") {
                    gap = Some(v.parse().map_err(|_| format!("bad gap in {line:?}"))?);
                }
            }
            Ok(Scenario {
                name: fields[0].to_string(),
                ours: parse_side(fields[1])?,
                theirs: parse_side(fields[2])?,
                hold: match fields[3] {
                    "hold" => true,
                    "attack" => false,
                    other => return Err(format!("opp must be attack or hold, got {other:?}")),
                },
                gap,
            })
        })
        .collect()
}

fn parse_tactics(text: &str) -> Result<Vec<Tactic>, String> {
    text.split(';')
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(|c| {
            let (name, p) = c.split_once(':').unwrap_or((c, ""));
            let (screen, spread) = match name {
                "screen" | "screenfan" => (true, false),
                "spread" => (false, true),
                "screenspread" => (true, true),
                _ => (false, false),
            };
            let fan = matches!(name, "fan" | "screenfan");
            let kind = match name {
                "amove" => Kind::AMove,
                "hold" => Kind::Hold,
                "line" => Kind::Line,
                "charge" | "screen" | "spread" | "screenspread" | "fan" | "screenfan" => {
                    Kind::Charge
                }
                "split" => Kind::Split,
                "wide" => Kind::Wide,
                "ambush" => Kind::Ambush,
                other => return Err(format!("unknown tactic {other:?}")),
            };
            let default = match kind {
                Kind::Line => 2.5,
                Kind::Ambush => 7.0,
                _ => 11.0,
            };
            let p = if p.is_empty() {
                default
            } else {
                p.parse().map_err(|_| format!("bad param in {c:?}"))?
            };
            Ok(Tactic {
                label: c.to_string(),
                kind,
                p,
                screen,
                spread,
                fan,
            })
        })
        .collect()
}

fn d(a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    (dx * dx + dy * dy).det_sqrt()
}

fn group(ids: impl IntoIterator<Item = u32>) -> Option<UnitGroup> {
    UnitGroup::new(ids).ok()
}

/// The scripted opponent: attack-move at what it sees (or the candidate's spawn), or park.
struct Opponent {
    hold: bool,
    goal: (f32, f32),
    started: bool,
    last_check: u32,
}

impl AiStrategy for Opponent {
    fn step(&mut self, frame: &AiFrame, actions: &mut AiActions) {
        let tick = frame.tick();
        if tick < 3 {
            return;
        }
        let Some(obs) = AiObservation::from_frame(frame) else {
            return;
        };
        let units: Vec<&AiEntitySummary> = obs
            .owned
            .iter()
            .filter(|u| u.hp > 0 && u.kind.is_unit() && u.kind != EntityKind::Worker)
            .collect();
        if self.hold {
            if !self.started {
                if let Some(g) = group(units.iter().map(|u| u.id)) {
                    let _ = actions.hold_position(&g, false);
                }
                self.started = true;
            }
            return;
        }
        let seen: Vec<(f32, f32)> = obs
            .visible_enemies
            .iter()
            .filter(|e| e.kind.is_unit() && e.kind != EntityKind::Worker)
            .map(|e| (e.x, e.y))
            .collect();
        let goal = if seen.is_empty() {
            self.goal
        } else {
            let n = seen.len() as f32;
            (
                seen.iter().map(|p| p.0).sum::<f32>() / n,
                seen.iter().map(|p| p.1).sum::<f32>() / n,
            )
        };
        let send: Vec<u32> = if !self.started {
            self.started = true;
            self.last_check = tick;
            units.iter().map(|u| u.id).collect()
        } else if tick.saturating_sub(self.last_check) >= 30 {
            self.last_check = tick;
            units
                .iter()
                .filter(|u| u.state == AiEntityState::Idle)
                .map(|u| u.id)
                .collect()
        } else {
            Vec::new()
        };
        if let Some(g) = group(send) {
            let _ = actions.attack_move(&g, goal.0, goal.1, false);
        }
    }
}

/// The candidate: one Panzerfaust tactic.
struct Candidate {
    tactic: Tactic,
    spent: Arc<Mutex<BTreeSet<u32>>>,
    home: (f32, f32),
    toward: (f32, f32),
    spawn_center: (f32, f32),
    enemy_center: (f32, f32),
    started: bool,
    charging: bool,
    last_check: u32,
    retreated: BTreeSet<u32>,
    /// Panzerfaust -> (Tank, waypoint, attacking)
    assigned: BTreeMap<u32, (u32, (f32, f32), bool)>,
    wide_since: Option<u32>,
    approach_sent: bool,
    charge_tick: u32,
    rest_pending: bool,
}

impl Candidate {
    fn standing(&self) -> bool {
        matches!(self.tactic.kind, Kind::Hold | Kind::Line | Kind::Ambush)
    }
}

impl AiStrategy for Candidate {
    fn step(&mut self, frame: &AiFrame, actions: &mut AiActions) {
        let tick = frame.tick();
        if tick < 3 {
            return;
        }
        let Some(obs) = AiObservation::from_frame(frame) else {
            return;
        };
        let spent = self.spent.lock().unwrap().clone();
        let units: Vec<&AiEntitySummary> = obs
            .owned
            .iter()
            .filter(|u| u.hp > 0 && u.kind.is_unit() && u.kind != EntityKind::Worker)
            .collect();
        let fausts: Vec<&AiEntitySummary> = units
            .iter()
            .copied()
            .filter(|u| u.kind == EntityKind::Panzerfaust && !spent.contains(&u.id))
            .collect();
        let others: Vec<&AiEntitySummary> = units
            .iter()
            .copied()
            .filter(|u| u.kind != EntityKind::Panzerfaust)
            .collect();
        let tanks: Vec<&AiEntitySummary> = obs
            .visible_enemies
            .iter()
            .filter(|e| e.hp > 0 && e.kind == EntityKind::Tank)
            .collect();
        let enemies: Vec<(f32, f32)> = obs
            .visible_enemies
            .iter()
            .filter(|e| e.hp > 0 && e.kind.is_unit() && e.kind != EntityKind::Worker)
            .map(|e| (e.x, e.y))
            .collect();
        let enemy_goal = if enemies.is_empty() {
            self.enemy_center
        } else {
            let n = enemies.len() as f32;
            (
                enemies.iter().map(|p| p.0).sum::<f32>() / n,
                enemies.iter().map(|p| p.1).sum::<f32>() / n,
            )
        };

        // Spent Panzerfausts walk home (all but the naive tactic).
        if self.tactic.kind != Kind::AMove {
            let back: Vec<u32> = units
                .iter()
                .filter(|u| u.kind == EntityKind::Panzerfaust && spent.contains(&u.id))
                .filter(|u| !self.retreated.contains(&u.id))
                .map(|u| u.id)
                .collect();
            if let Some(g) = group(back.iter().copied()) {
                let _ = actions.move_group(&g, self.home.0, self.home.1, false);
                self.retreated.extend(back);
            }
        }

        if !self.started {
            self.started = true;
            self.last_check = tick;
            match self.tactic.kind {
                Kind::AMove => {
                    if let Some(g) = group(units.iter().map(|u| u.id)) {
                        let _ = actions.attack_move(
                            &g,
                            self.enemy_center.0,
                            self.enemy_center.1,
                            false,
                        );
                    }
                }
                Kind::Hold | Kind::Ambush => {
                    if let Some(g) = group(units.iter().map(|u| u.id)) {
                        let _ = actions.hold_position(&g, false);
                    }
                }
                Kind::Line => {
                    // Panzerfausts in a line across the enemy direction, the rest behind it.
                    let perp = (-self.toward.1, self.toward.0);
                    let n = fausts.len() as f32;
                    for (i, f) in fausts.iter().enumerate() {
                        let off = (i as f32 - (n - 1.0) / 2.0) * self.tactic.p;
                        let x = self.spawn_center.0 + perp.0 * off * TILE_PX;
                        let y = self.spawn_center.1 + perp.1 * off * TILE_PX;
                        if let Some(g) = group([f.id]) {
                            let _ = actions.move_group(&g, x, y, false);
                            let _ = actions.hold_position(&g, true);
                        }
                    }
                    if let Some(g) = group(others.iter().map(|u| u.id)) {
                        let _ = actions.hold_position(&g, false);
                    }
                }
                Kind::Charge | Kind::Split | Kind::Wide => {
                    if let Some(g) = group(units.iter().map(|u| u.id)) {
                        let _ = actions.hold_position(&g, false);
                    }
                }
            }
            return;
        }

        match self.tactic.kind {
            Kind::AMove => {
                if tick.saturating_sub(self.last_check) >= 30 {
                    self.last_check = tick;
                    let idle: Vec<u32> = units
                        .iter()
                        .filter(|u| u.state == AiEntityState::Idle)
                        .map(|u| u.id)
                        .collect();
                    if let Some(g) = group(idle) {
                        let _ = actions.attack_move(&g, enemy_goal.0, enemy_goal.1, false);
                    }
                }
            }
            Kind::Hold | Kind::Line => {}
            Kind::Ambush => {
                // A Panzerfaust goes for a Tank only once it is within reach-plus-a-bit.
                for f in &fausts {
                    if self.assigned.contains_key(&f.id) {
                        continue;
                    }
                    if let Some(t) = tanks
                        .iter()
                        .filter(|t| d((t.x, t.y), (f.x, f.y)) <= self.tactic.p * TILE_PX)
                        .min_by(|a, b| {
                            d((a.x, a.y), (f.x, f.y)).total_cmp(&d((b.x, b.y), (f.x, f.y)))
                        })
                    {
                        if let Some(g) = group([f.id]) {
                            let _ = actions.attack(&g, t.id, false);
                            self.assigned.insert(f.id, (t.id, (t.x, t.y), true));
                        }
                    }
                }
                // Re-aim at a dead target's neighbour.
                self.reassign_dead(&fausts, &tanks, actions, false);
            }
            Kind::Charge | Kind::Split | Kind::Wide => {
                let trigger = self.tactic.p * TILE_PX;
                let contact = tanks
                    .iter()
                    .any(|t| units.iter().any(|u| d((t.x, t.y), (u.x, u.y)) <= trigger));
                if !self.charging {
                    if contact {
                        self.charging = true;
                        self.charge_tick = tick;
                        let screen: Vec<&AiEntitySummary> = if self.tactic.screen {
                            others
                                .iter()
                                .copied()
                                .filter(|u| u.kind == EntityKind::Rifleman)
                                .collect()
                        } else {
                            Vec::new()
                        };
                        // Screen: Riflemen run to 2 tiles short of their nearest Tank, 1.5 tiles
                        // apart, so the cannon's nearest infantry is a Rifleman; then they fight.
                        let n = screen.len() as f32;
                        for (i, r) in screen.iter().enumerate() {
                            let Some(t) = tanks.iter().min_by(|a, b| {
                                d((a.x, a.y), (r.x, r.y)).total_cmp(&d((b.x, b.y), (r.x, r.y)))
                            }) else {
                                continue;
                            };
                            let l = d((r.x, r.y), (t.x, t.y)).max(1.0);
                            let dir = ((r.x - t.x) / l, (r.y - t.y) / l);
                            let perp = (-dir.1, dir.0);
                            let off = (i as f32 - (n - 1.0) / 2.0) * 1.5;
                            let wp = (
                                t.x + (dir.0 * 2.0 + perp.0 * off) * TILE_PX,
                                t.y + (dir.1 * 2.0 + perp.1 * off) * TILE_PX,
                            );
                            if let Some(g) = group([r.id]) {
                                let _ = actions.move_group(&g, wp.0, wp.1, false);
                                let _ = actions.attack_move(&g, t.x, t.y, true);
                            }
                        }
                        // The rest (own Tanks, and Riflemen when not screening) join in.
                        let rest = others
                            .iter()
                            .filter(|u| !screen.iter().any(|s| s.id == u.id))
                            .map(|u| u.id);
                        if let Some(g) = group(rest) {
                            let _ = actions.attack_move(&g, enemy_goal.0, enemy_goal.1, false);
                        }
                    } else if !self.approach_sent && obs.visible_enemies.is_empty() {
                        // Nothing in sight (a parked enemy out of view): walk toward it. A screen
                        // walks its Riflemen first and the rest 1.5 s later.
                        if tick > 60 {
                            self.approach_sent = true;
                            self.charge_tick = tick;
                            let lead = units
                                .iter()
                                .filter(|u| !self.tactic.screen || u.kind == EntityKind::Rifleman)
                                .map(|u| u.id);
                            if let Some(g) = group(lead) {
                                let _ = actions.attack_move(
                                    &g,
                                    self.enemy_center.0,
                                    self.enemy_center.1,
                                    false,
                                );
                            }
                            self.rest_pending = self.tactic.screen;
                        }
                        return;
                    } else {
                        if self.rest_pending && tick >= self.charge_tick + lead_ticks() {
                            self.rest_pending = false;
                            let rest = units
                                .iter()
                                .filter(|u| u.kind != EntityKind::Rifleman)
                                .map(|u| u.id);
                            if let Some(g) = group(rest) {
                                let _ = actions.attack_move(
                                    &g,
                                    self.enemy_center.0,
                                    self.enemy_center.1,
                                    false,
                                );
                            }
                        }
                        return;
                    }
                }
                if tanks.is_empty() {
                    // Tanks gone or out of sight: everyone attack-moves at what is left.
                    if tick.saturating_sub(self.last_check) >= 30 {
                        self.last_check = tick;
                        let idle: Vec<u32> = units
                            .iter()
                            .filter(|u| u.state == AiEntityState::Idle && !spent.contains(&u.id))
                            .map(|u| u.id)
                            .collect();
                        if let Some(g) = group(idle) {
                            let _ = actions.attack_move(&g, enemy_goal.0, enemy_goal.1, false);
                        }
                    }
                    return;
                }
                // Screen: the Panzerfausts start 1.5 s after the Riflemen.
                if self.tactic.screen && tick < self.charge_tick + lead_ticks() {
                    return;
                }
                match self.tactic.kind {
                    Kind::Charge => {
                        let mut by_tank: BTreeMap<u32, Vec<&AiEntitySummary>> = BTreeMap::new();
                        for f in &fausts {
                            let need = match self.assigned.get(&f.id) {
                                Some((t, _, _)) => !tanks.iter().any(|x| x.id == *t),
                                None => true,
                            };
                            if !need {
                                continue;
                            }
                            if let Some(t) = tanks.iter().min_by(|a, b| {
                                d((a.x, a.y), (f.x, f.y)).total_cmp(&d((b.x, b.y), (f.x, f.y)))
                            }) {
                                by_tank.entry(t.id).or_default().push(*f);
                            }
                        }
                        for (tid, mut fs) in by_tank {
                            let Some(t) = tanks.iter().find(|x| x.id == tid) else {
                                continue;
                            };
                            let n = fs.len() as f32;
                            let c = (
                                fs.iter().map(|f| f.x).sum::<f32>() / n,
                                fs.iter().map(|f| f.y).sum::<f32>() / n,
                            );
                            let l = d(c, (t.x, t.y)).max(1.0);
                            let dir = ((c.0 - t.x) / l, (c.1 - t.y) / l);
                            let perp = (-dir.1, dir.0);
                            let across =
                                |f: &AiEntitySummary| (f.x - t.x) * perp.0 + (f.y - t.y) * perp.1;
                            fs.sort_by(|a, b| across(a).total_cmp(&across(b)));
                            for (j, f) in fs.iter().enumerate() {
                                let Some(g) = group([f.id]) else { continue };
                                if self.tactic.spread && d((f.x, f.y), (t.x, t.y)) > 6.5 * TILE_PX {
                                    // Spread: line abreast 6 tiles out, 2.2 tiles apart, then in.
                                    let off = (j as f32 - (n - 1.0) / 2.0) * 2.2;
                                    let wp = (
                                        t.x + (dir.0 * 6.0 + perp.0 * off) * TILE_PX,
                                        t.y + (dir.1 * 6.0 + perp.1 * off) * TILE_PX,
                                    );
                                    let _ = actions.move_group(&g, wp.0, wp.1, false);
                                    let _ = actions.attack(&g, t.id, true);
                                } else if self.tactic.fan
                                    && d((f.x, f.y), (t.x, t.y)) > 5.5 * TILE_PX
                                {
                                    let off = (j as f32 - (n - 1.0) / 2.0) * 2.2;
                                    let wp = (
                                        t.x + (dir.0 * 4.5 + perp.0 * off) * TILE_PX,
                                        t.y + (dir.1 * 4.5 + perp.1 * off) * TILE_PX,
                                    );
                                    let _ = actions.move_group(&g, wp.0, wp.1, false);
                                    let _ = actions.attack(&g, t.id, true);
                                } else {
                                    let _ = actions.attack(&g, t.id, false);
                                }
                                self.assigned.insert(f.id, (t.id, (t.x, t.y), true));
                            }
                        }
                    }
                    Kind::Split | Kind::Wide => {
                        let radius = if self.tactic.kind == Kind::Split {
                            4.5
                        } else {
                            10.0
                        };
                        // Share the loaded Panzerfausts over the Tanks (at most 5 each), nearest first.
                        let mut load: BTreeMap<u32, usize> = BTreeMap::new();
                        for (t, _, _) in self.assigned.values() {
                            *load.entry(*t).or_default() += 1;
                        }
                        let mut fresh = Vec::new();
                        for f in &fausts {
                            let ok = self
                                .assigned
                                .get(&f.id)
                                .is_some_and(|(t, _, _)| tanks.iter().any(|x| x.id == *t));
                            if !ok {
                                if let Some((t, _, _)) = self.assigned.remove(&f.id) {
                                    if let Some(n) = load.get_mut(&t) {
                                        *n = n.saturating_sub(1);
                                    }
                                }
                                fresh.push(*f);
                            }
                        }
                        for f in fresh {
                            let pick = tanks.iter().min_by(|a, b| {
                                let la = *load.get(&a.id).unwrap_or(&0);
                                let lb = *load.get(&b.id).unwrap_or(&0);
                                let ka = (la >= 5) as u32;
                                let kb = (lb >= 5) as u32;
                                (ka, la, d((a.x, a.y), (f.x, f.y)) as u32).cmp(&(
                                    kb,
                                    lb,
                                    d((b.x, b.y), (f.x, f.y)) as u32,
                                ))
                            });
                            let Some(t) = pick else { continue };
                            let slot = *load.get(&t.id).unwrap_or(&0);
                            *load.entry(t.id).or_default() += 1;
                            // Spread angles around the Tank, centred on our side: 0, +40, -40, +80, -80 deg.
                            let base = (f.y - t.y).det_atan2(f.x - t.x);
                            let k =
                                slot.div_ceil(2) as f32 * if slot % 2 == 1 { 1.0 } else { -1.0 };
                            let angle = base + k * 40f32.to_radians();
                            let wp = (
                                t.x + angle.det_cos() * radius * TILE_PX,
                                t.y + angle.det_sin() * radius * TILE_PX,
                            );
                            if let Some(g) = group([f.id]) {
                                if self.tactic.kind == Kind::Split {
                                    let _ = actions.move_group(&g, wp.0, wp.1, false);
                                    let _ = actions.attack(&g, t.id, true);
                                    self.assigned.insert(f.id, (t.id, wp, true));
                                } else {
                                    let _ = actions.move_group(&g, wp.0, wp.1, false);
                                    self.assigned.insert(f.id, (t.id, wp, false));
                                }
                            }
                        }
                        if self.tactic.kind == Kind::Wide {
                            let since = *self.wide_since.get_or_insert(tick);
                            let waiting: Vec<(u32, u32)> = self
                                .assigned
                                .iter()
                                .filter(|(_, (_, _, go))| !go)
                                .map(|(f, (t, _, _))| (*f, *t))
                                .collect();
                            let all_there = self.assigned.iter().all(|(f, (_, wp, go))| {
                                *go || fausts
                                    .iter()
                                    .find(|x| x.id == *f)
                                    .is_none_or(|x| d((x.x, x.y), *wp) <= 1.5 * TILE_PX)
                            });
                            if all_there || tick.saturating_sub(since) >= 240 {
                                for (f, t) in waiting {
                                    if let Some(g) = group([f]) {
                                        let _ = actions.attack(&g, t, false);
                                    }
                                    if let Some(entry) = self.assigned.get_mut(&f) {
                                        entry.2 = true;
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        let _ = self.standing();
    }
}

impl Candidate {
    fn reassign_dead(
        &mut self,
        fausts: &[&AiEntitySummary],
        tanks: &[&AiEntitySummary],
        actions: &mut AiActions,
        _all: bool,
    ) {
        let dead: Vec<u32> = self
            .assigned
            .iter()
            .filter(|(_, (t, _, _))| !tanks.iter().any(|x| x.id == *t))
            .map(|(f, _)| *f)
            .collect();
        for f in dead {
            self.assigned.remove(&f);
            if fausts.iter().any(|x| x.id == f) {
                if let Some(g) = group([f]) {
                    let _ = actions.hold_position(&g, false);
                }
            }
        }
    }
}

fn spawn_first(game: &mut Game, owner: u32, kind: EntityKind, points: &[(f32, f32)]) -> bool {
    points.iter().any(|point| {
        game.apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner,
            kind,
            x: point.0,
            y: point.1,
            completed: true,
            dao: None,
            dao_master: false,
            faction_id: None,
        }))
        .is_ok()
    })
}

fn jitter(seed: u32, index: u32) -> f32 {
    let mut x = seed
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(index.wrapping_mul(0x85EB_CA6B))
        .wrapping_add(0x27D4_EB2F);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x % 10_000) as f32 / 10_000.0 - 0.5
}

struct Setup<'a> {
    scenario: &'a Scenario,
    tactic: &'a Tactic,
    candidate: u32,
    seed: u32,
    map: &'a str,
    ticks: u32,
    gap: f32,
    trench: bool,
    meth: bool,
}

fn cost(kind: EntityKind) -> f32 {
    rts_rules::balance::unit_stats(kind).map_or(0.0, |s| (s.cost_steel + s.cost_oil) as f32)
}

fn run_fight(setup: &Setup<'_>) -> Result<FightResult, String> {
    let candidate = setup.candidate;
    let opponent = 3 - candidate;
    let player = |id: u32| PlayerInit {
        id,
        team_id: id,
        faction_id: "kriegsia".to_string(),
        name: format!("p{id}"),
        color: if id == 1 { "#4cc9f0" } else { "#f72585" }.to_string(),
        is_ai: true,
    };
    let players = vec![player(1), player(2)];
    let map = Map::load_for_players(setup.map, &[(1, 1), (2, 2)], MAP_SEED)?;
    let metadata = Map::metadata_for_name(setup.map)?;
    let mut game = Game::new_lab(&players, setup.seed.wrapping_add(11), map, metadata);
    let depots: BTreeMap<u32, (f32, f32)> = game
        .snapshot_full_for(1)
        .entities
        .iter()
        .filter(|e| e.kind == "resource_depot")
        .map(|e| (e.owner, (e.x, e.y)))
        .collect();
    let home = *depots.get(&candidate).ok_or("candidate has no depot")?;
    let enemy = *depots.get(&opponent).ok_or("opponent has no depot")?;
    for id in [1, 2] {
        let units: Vec<u32> = game
            .lab_owned_units(id)
            .map_err(|err| format!("{err:?}"))?
            .into_iter()
            .map(|(unit, _)| unit)
            .collect();
        game.apply_lab_op(LabOp::DeleteEntities(units))
            .map_err(|err| format!("{err:?}"))?;
        game.apply_lab_op(LabOp::SetPlayerResources(LabSetPlayerResources {
            player_id: id,
            steel: 0,
            oil: 0,
        }))
        .map_err(|err| format!("{err:?}"))?;
        let mut upgrades = vec![UpgradeKind::TankUnlock, UpgradeKind::Panzerfausts];
        if setup.meth {
            upgrades.push(UpgradeKind::Methamphetamines);
        }
        if setup.trench {
            upgrades.push(UpgradeKind::Entrenchment);
        }
        for upgrade in upgrades {
            game.apply_lab_op(LabOp::SetCompletedResearch(LabSetCompletedResearch {
                player_id: id,
                upgrade,
                completed: true,
            }))
            .map_err(|err| format!("{err:?}"))?;
        }
    }
    let (dx, dy) = (enemy.0 - home.0, enemy.1 - home.1);
    let length = (dx * dx + dy * dy).det_sqrt().max(1.0);
    let u = (dx / length, dy / length);
    let perp = (-u.1, u.0);
    let mid = ((home.0 + enemy.0) / 2.0, (home.1 + enemy.1) / 2.0);
    let shift = jitter(setup.seed, 999) * 6.0;
    let center_of = |toward: f32| {
        (
            mid.0 - u.0 * toward * setup.gap / 2.0 * TILE_PX + perp.0 * shift * TILE_PX,
            mid.1 - u.1 * toward * setup.gap / 2.0 * TILE_PX + perp.1 * shift * TILE_PX,
        )
    };
    let mut place = |owner: u32, side: &[(EntityKind, usize)], toward: f32, salt: u32| {
        let center = center_of(toward);
        let at = |along: f32, across: f32| {
            (
                center.0 + (u.0 * along * toward + perp.0 * across) * TILE_PX,
                center.1 + (u.1 * along * toward + perp.1 * across) * TILE_PX,
            )
        };
        let is_vehicle =
            |kind: &EntityKind| matches!(kind, EntityKind::Tank | EntityKind::ScoutCar);
        let vehicles: usize = side
            .iter()
            .filter(|(k, _)| is_vehicle(k))
            .map(|(_, n)| n)
            .sum();
        let mut index = 0u32;
        let mut row_vehicle = 0usize;
        let mut infantry = 0usize;
        let mut spawned = 0usize;
        for (kind, count) in side {
            for _ in 0..*count {
                index += 1;
                let (along, across) = if is_vehicle(kind) {
                    let slot = row_vehicle;
                    row_vehicle += 1;
                    (1.5, (slot as f32 - (vehicles as f32 - 1.0) / 2.0) * 3.0)
                } else {
                    let slot = infantry;
                    infantry += 1;
                    let row = slot / 8;
                    let col = slot % 8;
                    (-(row as f32) * 1.4, (col as f32 - 3.5) * 1.3)
                };
                let jx = jitter(setup.seed.wrapping_add(salt), index) * 0.6;
                let jy = jitter(setup.seed.wrapping_add(salt + 7), index) * 0.6;
                let base = at(along + jx, across + jy);
                let points: Vec<(f32, f32)> = [
                    (0.0, 0.0),
                    (0.7, 0.0),
                    (-0.7, 0.0),
                    (0.0, 0.7),
                    (0.0, -0.7),
                    (1.4, 1.4),
                    (-1.4, -1.4),
                ]
                .iter()
                .map(|(ox, oy)| (base.0 + ox * TILE_PX, base.1 + oy * TILE_PX))
                .collect();
                if spawn_first(&mut game, owner, *kind, &points) {
                    spawned += 1;
                }
            }
        }
        spawned
    };
    let want_ours: usize = setup.scenario.ours.iter().map(|(_, n)| n).sum();
    let want_theirs: usize = setup.scenario.theirs.iter().map(|(_, n)| n).sum();
    if place(candidate, &setup.scenario.ours, 1.0, 101) != want_ours
        || place(opponent, &setup.scenario.theirs, -1.0, 202) != want_theirs
    {
        return Err(format!("spawn failed in {}", setup.scenario.name));
    }
    let start_units = |game: &Game, owner: u32| -> BTreeMap<u32, (EntityKind, u32)> {
        game.snapshot_full_for(owner)
            .entities
            .iter()
            .filter(|e| e.owner == owner)
            .filter_map(|e| {
                let kind: EntityKind = e.kind.parse().ok()?;
                (kind.is_unit() && kind != EntityKind::Worker).then_some((e.id, (kind, e.max_hp)))
            })
            .collect()
    };
    let ours = start_units(&game, candidate);
    let theirs = start_units(&game, opponent);
    let spent = Arc::new(Mutex::new(BTreeSet::new()));
    let cand_center = center_of(1.0);
    let opp_center = center_of(-1.0);
    let cand_side = Candidate {
        tactic: setup.tactic.clone(),
        spent: spent.clone(),
        home,
        toward: u,
        spawn_center: cand_center,
        enemy_center: opp_center,
        started: false,
        charging: false,
        last_check: 0,
        retreated: BTreeSet::new(),
        assigned: BTreeMap::new(),
        wide_since: None,
        approach_sent: false,
        charge_tick: 0,
        rest_pending: false,
    };
    let opp_side = Opponent {
        hold: setup.scenario.hold,
        goal: cand_center,
        started: false,
        last_check: 0,
    };
    let (first, second): (Box<dyn AiStrategy>, Box<dyn AiStrategy>) = if candidate == 1 {
        (Box::new(cand_side), Box::new(opp_side))
    } else {
        (Box::new(opp_side), Box::new(cand_side))
    };
    let mut controllers = vec![
        AiController::with_strategy(1, first),
        AiController::with_strategy(2, second),
    ];
    let hp_of = |game: &Game, owner: u32, units: &BTreeMap<u32, (EntityKind, u32)>| {
        let snapshot = game.snapshot_full_for(owner);
        units
            .iter()
            .map(|(id, (kind, max))| {
                let hp = snapshot
                    .entities
                    .iter()
                    .find(|e| e.id == *id)
                    .map_or(0, |e| e.hp);
                (*kind, hp, *max)
            })
            .collect::<Vec<_>>()
    };
    let alive = |rows: &[(EntityKind, u32, u32)]| rows.iter().filter(|r| r.1 > 0).count();
    let mut last_total = u32::MAX;
    let mut last_change = 0u32;
    let mut fired: BTreeSet<u32> = BTreeSet::new();
    let mut hurt = Hurt::default();
    // Candidate infantry: id -> (kind, hp, x, y, loaded).
    let mut prev_inf: BTreeMap<u32, (EntityKind, u32, f32, f32, bool)> = BTreeMap::new();
    while game.tick_count() < setup.ticks {
        CanonicalAiTickDriver::run(&mut game, &mut controllers, AiAlivePolicy::Normal);
        let events = game.tick();
        // Spent rockets, seen by the lab (the AI frame does not carry them).
        let snapshot = game.snapshot_full_for(candidate);
        {
            let mut attacks: BTreeSet<(u32, u32, String)> = BTreeSet::new();
            let mut shells: Vec<(f32, f32)> = Vec::new();
            for (_, list) in &events {
                for event in list {
                    match event {
                        Event::Attack {
                            from,
                            to,
                            weapon_kind,
                            ..
                        } => {
                            attacks.insert((*from, *to, weapon_kind.clone().unwrap_or_default()));
                        }
                        Event::MortarImpact {
                            x,
                            y,
                            radius_tiles,
                            rocket,
                            ..
                        } if !rocket
                            && (radius_tiles - 1.4).abs() < 0.01
                            && !shells.contains(&(*x, *y)) =>
                        {
                            shells.push((*x, *y));
                        }
                        _ => {}
                    }
                }
            }
            let now_inf: BTreeMap<u32, (EntityKind, u32, f32, f32, bool)> = snapshot
                .entities
                .iter()
                .filter(|e| e.owner == candidate && e.hp > 0)
                .filter_map(|e| {
                    let kind: EntityKind = e.kind.parse().ok()?;
                    matches!(kind, EntityKind::Panzerfaust | EntityKind::Rifleman).then_some((
                        e.id,
                        (kind, e.hp, e.x, e.y, e.panzerfaust_loaded == Some(true)),
                    ))
                })
                .collect();
            let mut shell_pf_hits: BTreeMap<usize, usize> = BTreeMap::new();
            for (id, (kind, hp0, x, y, loaded)) in &prev_inf {
                let hp1 = now_inf.get(id).map_or(0, |v| v.1);
                if hp1 >= *hp0 {
                    continue;
                }
                let drop = (hp0 - hp1) as f32;
                let cannon = attacks.iter().any(|a| a.1 == *id && a.2 == "tank_cannon");
                let coax = attacks.iter().any(|a| a.1 == *id && a.2 == "tank_coax");
                let shell = shells.iter().position(|s| d(*s, (*x, *y)) <= 2.0 * TILE_PX);
                if *kind == EntityKind::Panzerfaust {
                    if cannon {
                        hurt.pf_cannon += drop;
                    } else if let Some(i) = shell {
                        let s = drop.min(30.0);
                        hurt.pf_splash += s;
                        *shell_pf_hits.entry(i).or_default() += 1;
                        if coax {
                            hurt.pf_coax += drop - s;
                        } else {
                            hurt.pf_other += drop - s;
                        }
                    } else if coax {
                        hurt.pf_coax += drop;
                    } else {
                        hurt.pf_other += drop;
                    }
                    if hp1 == 0 && *loaded {
                        hurt.pf_dead_loaded += 1;
                    }
                } else if cannon {
                    hurt.rifle_cannon += drop;
                } else if shell.is_some() {
                    hurt.rifle_splash += drop.min(30.0);
                }
            }
            hurt.multi_splash += shell_pf_hits.values().filter(|n| **n >= 2).count();
            let trace = std::env::var("PFLAB_TRACE").is_ok();
            for (from, to, w) in &attacks {
                if w == "tank_cannon" && trace {
                    if let (Some(v), Some(t)) = (
                        prev_inf.get(to),
                        snapshot.entities.iter().find(|e| e.id == *from),
                    ) {
                        let near = |k: EntityKind| {
                            prev_inf
                                .values()
                                .filter(|u| u.0 == k)
                                .map(|u| d((u.2, u.3), (t.x, t.y)) / TILE_PX)
                                .fold(f32::MAX, f32::min)
                        };
                        eprintln!(
                            "CS {} {} s{} c{} t{} {:?} d{:.1} nearR {:.1} nearPF {:.1}",
                            setup.scenario.name,
                            setup.tactic.label,
                            setup.seed,
                            candidate,
                            game.tick_count(),
                            v.0,
                            d((v.2, v.3), (t.x, t.y)) / TILE_PX,
                            near(EntityKind::Rifleman),
                            near(EntityKind::Panzerfaust)
                        );
                    }
                }
                if w == "tank_cannon" {
                    match prev_inf.get(to).map(|v| v.0) {
                        Some(EntityKind::Panzerfaust) => hurt.shots_pf += 1,
                        Some(EntityKind::Rifleman) => hurt.shots_rifle += 1,
                        _ => {}
                    }
                }
            }
            prev_inf = now_inf;
        }
        let now: Vec<u32> = snapshot
            .entities
            .iter()
            .filter(|e| e.owner == candidate && e.panzerfaust_loaded == Some(false))
            .map(|e| e.id)
            .collect();
        if !now.is_empty() {
            let mut set = spent.lock().unwrap();
            for id in now {
                set.insert(id);
                fired.insert(id);
            }
        }
        if game.tick_count().is_multiple_of(15) {
            let a = hp_of(&game, candidate, &ours);
            let b = hp_of(&game, opponent, &theirs);
            if alive(&a) == 0 || alive(&b) == 0 {
                break;
            }
            let total: u32 = a.iter().chain(&b).map(|row| row.1).sum();
            if total != last_total {
                last_total = total;
                last_change = game.tick_count();
            } else if game.tick_count() - last_change >= 600 {
                break;
            }
        }
    }
    let a = hp_of(&game, candidate, &ours);
    let b = hp_of(&game, opponent, &theirs);
    let lost = |rows: &[(EntityKind, u32, u32)]| -> f32 {
        rows.iter()
            .map(|(kind, hp, max)| cost(*kind) * (1.0 - *hp as f32 / (*max).max(1) as f32))
            .sum()
    };
    Ok(FightResult {
        hurt,
        our_lost: lost(&a),
        their_lost: lost(&b),
        our_dead: a.len() - alive(&a),
        their_dead: b.len() - alive(&b),
        our_alive: alive(&a),
        their_alive: alive(&b),
        tanks_killed: b
            .iter()
            .filter(|r| r.0 == EntityKind::Tank && r.1 == 0)
            .count(),
        tanks: b.iter().filter(|r| r.0 == EntityKind::Tank).count(),
        fired: fired.len(),
        fausts: a.iter().filter(|r| r.0 == EntityKind::Panzerfaust).count(),
    })
}

pub fn run_from_env() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |flag: &str| -> Option<String> {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let scenarios = match get("--scenarios").map(|s| parse_scenarios(&s)) {
        Some(Ok(s)) => s,
        Some(Err(err)) => {
            eprintln!("ai-pf-lab: {err}");
            process::exit(2);
        }
        None => {
            eprintln!("ai-pf-lab: --scenarios is required");
            process::exit(2);
        }
    };
    let tactics =
        match parse_tactics(&get("--tactics").unwrap_or_else(|| "amove;hold;charge".to_string())) {
            Ok(t) => t,
            Err(err) => {
                eprintln!("ai-pf-lab: {err}");
                process::exit(2);
            }
        };
    let seeds: u32 = get("--seeds").and_then(|s| s.parse().ok()).unwrap_or(6);
    if scenarios.is_empty() || tactics.is_empty() || seeds == 0 {
        eprintln!("ai-pf-lab: at least one scenario, tactic and seed are required");
        process::exit(2);
    }
    let map = get("--map").unwrap_or_else(|| "1v1 No Terrain".to_string());
    let ticks: u32 = get("--ticks").and_then(|s| s.parse().ok()).unwrap_or(2700);
    let gap: f32 = get("--gap").and_then(|s| s.parse().ok()).unwrap_or(20.0);
    let threads: usize = get("--threads")
        .and_then(|s| s.parse().ok())
        .unwrap_or(8)
        .min(8);
    let trench = get("--trench").is_none_or(|s| s != "0");
    let meth = get("--meth").is_none_or(|s| s != "0");
    let mut jobs = Vec::new();
    for si in 0..scenarios.len() {
        for ti in 0..tactics.len() {
            for seed in 0..seeds {
                for candidate in [1u32, 2] {
                    jobs.push((si, ti, seed, candidate));
                }
            }
        }
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("thread pool");
    let results: Vec<((usize, usize), Result<FightResult, String>)> = pool.install(|| {
        jobs.par_iter()
            .map(|&(si, ti, seed, candidate)| {
                let scenario = &scenarios[si];
                let setup = Setup {
                    scenario,
                    tactic: &tactics[ti],
                    candidate,
                    seed,
                    map: &map,
                    ticks,
                    gap: scenario.gap.unwrap_or(gap),
                    trench,
                    meth,
                };
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_fight(&setup)))
                        .unwrap_or_else(|_| Err("panic".to_string()));
                if let Err(err) = &result {
                    eprintln!(
                        "ai-pf-lab: scenario {:?}, tactic {:?}, seed {seed}, p{candidate}: {err}",
                        scenario.name, tactics[ti].label
                    );
                }
                ((si, ti), result)
            })
            .collect()
    });
    println!(
        "{:<14} {:<10} {:>3} {:>4} {:>4} {:>7} {:>7} {:>6} {:>7} {:>9} {:>8} {:>8}",
        "scenario",
        "tactic",
        "n",
        "win",
        "loss",
        "ourLost",
        "thrLost",
        "trade",
        "net",
        "tanks k",
        "fired",
        "dead o/t"
    );
    for (si, scenario) in scenarios.iter().enumerate() {
        for (ti, tactic) in tactics.iter().enumerate() {
            let rows: Vec<&FightResult> = results
                .iter()
                .filter(|((s, t), _)| *s == si && *t == ti)
                .filter_map(|(_, r)| r.as_ref().ok())
                .collect();
            let errors = results
                .iter()
                .filter(|((s, t), r)| *s == si && *t == ti && r.is_err())
                .count();
            if rows.is_empty() {
                println!("{:<14} {:<10} errors={errors}", scenario.name, tactic.label);
                continue;
            }
            let n = rows.len() as f32;
            let wins = rows
                .iter()
                .filter(|r| r.their_alive == 0 && r.our_alive > 0)
                .count();
            let losses = rows
                .iter()
                .filter(|r| r.our_alive == 0 && r.their_alive > 0)
                .count();
            let our: f32 = rows.iter().map(|r| r.our_lost).sum();
            let their: f32 = rows.iter().map(|r| r.their_lost).sum();
            let tk: f32 = rows.iter().map(|r| r.tanks_killed as f32).sum::<f32>() / n;
            let tt = rows[0].tanks;
            let fired: f32 = rows.iter().map(|r| r.fired as f32).sum::<f32>() / n;
            let ff = rows[0].fausts;
            let od: f32 = rows.iter().map(|r| r.our_dead as f32).sum::<f32>() / n;
            let td: f32 = rows.iter().map(|r| r.their_dead as f32).sum::<f32>() / n;
            println!(
                "{:<14} {:<10} {:>3} {:>4} {:>4} {:>7.0} {:>7.0} {:>6.2} {:>7.0} {:>5.1}/{:<3} {:>4.1}/{:<3} {:>4.1}/{:<4.1}{}",
                scenario.name,
                tactic.label,
                rows.len(),
                wins,
                losses,
                our / n,
                their / n,
                their / our.max(1.0),
                (their - our) / n,
                tk,
                tt,
                fired,
                ff,
                od,
                td,
                if errors > 0 { format!(" err={errors}") } else { String::new() }
            );
        }
    }
    // Per fight: Panzerfaust HP lost by cause, Rifleman HP lost to the cannon, cannon shots at
    // Panzerfausts / Riflemen, HE shells that hurt 2+ Panzerfausts, Panzerfausts killed loaded.
    println!(
        "\n{:<14} {:<13} {:>7} {:>7} {:>7} {:>7} {:>8} {:>8} {:>7} {:>6} {:>8}",
        "scenario",
        "tactic",
        "pfCann",
        "pfSplsh",
        "pfCoax",
        "pfOther",
        "rfCann",
        "rfSplsh",
        "shotsPF",
        "shotsR",
        "multiHE/deadL"
    );
    for (si, scenario) in scenarios.iter().enumerate() {
        for (ti, tactic) in tactics.iter().enumerate() {
            let rows: Vec<&FightResult> = results
                .iter()
                .filter(|((s, t), _)| *s == si && *t == ti)
                .filter_map(|(_, r)| r.as_ref().ok())
                .collect();
            if rows.is_empty() {
                continue;
            }
            let n = rows.len() as f32;
            let avg = |f: &dyn Fn(&Hurt) -> f32| rows.iter().map(|r| f(&r.hurt)).sum::<f32>() / n;
            println!(
                "{:<14} {:<13} {:>7.0} {:>7.0} {:>7.0} {:>7.0} {:>8.0} {:>8.0} {:>7.1} {:>6.1} {:>4.2}/{:<4.1}",
                scenario.name,
                tactic.label,
                avg(&|h| h.pf_cannon),
                avg(&|h| h.pf_splash),
                avg(&|h| h.pf_coax),
                avg(&|h| h.pf_other),
                avg(&|h| h.rifle_cannon),
                avg(&|h| h.rifle_splash),
                avg(&|h| h.shots_pf as f32),
                avg(&|h| h.shots_rifle as f32),
                avg(&|h| h.multi_splash as f32),
                avg(&|h| h.pf_dead_loaded as f32),
            );
        }
    }
    if results.iter().any(|(_, result)| result.is_err()) {
        process::exit(1);
    }
}

/// Screen lead: ticks the Riflemen start ahead of the Panzerfausts (`PFLAB_LEAD`, default 45).
fn lead_ticks() -> u32 {
    std::env::var("PFLAB_LEAD")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(45)
}
