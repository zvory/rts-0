//! Matchup micro (V1.1 switch `mm`): per-unit engagement tactics chosen from the unit's own kind
//! against the enemy kinds in contact, run after every other system has ordered.
//!
//! The 300-game all-map bench (2026-11-14) showed where Jeff's units die: Riflemen to Tanks
//! (about 6,500 across the 300 games), Tank against Tank, Rifleman trades against Panzerfausts and
//! Riflemen, and Riflemen against Rush's Machine Gunners. Each matchup has a different right
//! answer, and each answer is one tactic here:
//! - avoid: a unit that cannot hurt what threatens it (Riflemen and Scout Cars against Tanks)
//!   leaves that unit's reach, unless an own Tank stands between them;
//! - kite: a unit that fires on the move and outranges its target (Scout Car 7 against infantry
//!   5, Tank coax 6 against infantry 5) keeps the range gap;
//! - focus: every unit picks the target it kills fastest per threat removed, sharing out expected
//!   damage so a squad does not overkill one target while another shoots freely;
//! - pull: wounded infantry under fire steps back so the shooter retargets;
//! - charge: infantry closes on a Machine Gunner instead of trading at its longer range;
//! - stand: a Tank in sight of enemy Tanks stops, so its cannon ramps to full range.
//! Every tactic is a parameter (`Params`), so the engagement lab (`ai-engage-lab`) scores each
//! matchup against plain attack-move before any of it runs in a game.
use std::collections::{BTreeMap, BTreeSet};

use crate::ai_core::observation::{AiEntityState, AiEntitySummary, AiObservation};
use rts_rules::math::GameplayMath as _;
use rts_sim::game::entity::EntityKind;
use rts_sim::game::upgrade::UpgradeKind;

const TICK_HZ: f32 = 30.0;
/// A Tank's cannon reach while moving, and after standing still for `TANK_RAMP_TICKS`.
const TANK_BASE_REACH: f32 = 5.0;
const TANK_FULL_REACH: f32 = 14.0;
const TANK_RAMP_TICKS: f32 = 90.0;
/// An enemy that moved less than this between two looks is standing still.
const STILL_TILES: f32 = 0.15;
/// Enemies unseen this long are forgotten.
const TRACK_TICKS: u32 = 300;
/// Reverse hops stay under the simulation's 3-tile limit for driving backwards, so a Tank backing
/// off keeps its front armor toward the threat.
const REVERSE_HOP_TILES: f32 = 2.75;
/// Expected damage a focus pick books against its target, in seconds of the shooter's fire.
const FOCUS_HORIZON_S: f32 = 3.0;

/// The tactics and their tuning, parsed from `key=value,...` (`Params::parse`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Params {
    /// Riflemen, Machine Gunners and Scout Cars leave enemy Tanks' reach.
    pub(crate) avoid: bool,
    /// Tiles past the threat's reach an avoiding unit keeps.
    pub(crate) avoid_margin: f32,
    /// An avoiding unit stays when an own Tank is this many tiles nearer the threat than it
    /// (0 = never stays).
    pub(crate) support: f32,
    /// An avoiding unit stays when an own Tank is within this many tiles of it (0 = off).
    pub(crate) near: f32,
    /// Vehicles keep their range advantage over what they outrange.
    pub(crate) kite: bool,
    /// Tiles past the target's reach a kiting unit keeps.
    pub(crate) kite_margin: f32,
    /// Riflemen with Methamphetamines kite too (they fire on the move).
    pub(crate) kite_rifles: bool,
    /// Units pick targets.
    pub(crate) focus: bool,
    /// A current target is kept unless another scores this many times better.
    pub(crate) focus_keep: f32,
    /// Wounded infantry under fire steps back below this HP share (0 = off).
    pub(crate) pull: f32,
    /// Infantry closes on Machine Gunners within this many tiles (0 = off).
    pub(crate) charge: f32,
    /// Tanks stop to ramp their cannon when enemy Tanks are in sight but not yet this close.
    pub(crate) stand: f32,
    /// ...and only against enemy Tanks on the move.
    pub(crate) stand_moving: bool,
    /// Focus picks among enemies this many tiles past the unit's reach: infantry, vehicles.
    pub(crate) focus_slack: f32,
    pub(crate) focus_slack_vehicle: f32,
    /// What a Tank's focus picks: 0 nothing (the simulation's own targeting), 1 armored targets
    /// only (the coax already shoots infantry on its own), 2 anything.
    pub(crate) tank_focus: u32,
    /// Scout Cars' kite margin (Tanks use `kite_margin`).
    pub(crate) kite_margin_scout: f32,
    /// Avoid parked enemy Tanks too, not only moving ones.
    pub(crate) avoid_parked: bool,
    /// An avoiding unit stays held while the threat is within this many tiles past its avoid line.
    pub(crate) avoid_hold: f32,
    /// Charge only when no enemy Tank is in contact.
    pub(crate) charge_no_tank: bool,
    /// Enemies within this many tiles are in contact.
    pub(crate) contact: f32,
    /// Ticks before a unit is ordered again with the same tactic.
    pub(crate) reorder: u32,
    /// Ticks without contact before a unit goes back to its other orders.
    pub(crate) release: u32,
    /// Which own unit families the micro steers.
    pub(crate) tanks: bool,
    pub(crate) rifles: bool,
    pub(crate) scouts: bool,
    /// Leave units alone while Jeff has them on a building, worker or trap (raids), for this
    /// many ticks after the order or while they still shoot it (0 = off).
    pub(crate) eco: u32,
    /// A unit Jeff sends away from the fight with a plain move keeps that order (only avoid
    /// still steers it).
    pub(crate) respect_retreat: bool,
    /// What a Machine Gunner's focus picks: 0 nothing, 1 only targets already in reach (an
    /// attack order out of reach tears the gun down to chase), 2 like other infantry.
    pub(crate) mg_focus: u32,
    /// A Machine Gunner sets up (holds) when enemy infantry comes within this many tiles but is
    /// not yet in its reach, rather than walking in packed (0 = off).
    pub(crate) mg_stand: f32,
    /// Tanks' focus slack (negative = only targets that far inside the current reach: an attack
    /// order on a target past it moves a parked Tank, which drops its reach to 5).
    pub(crate) focus_slack_tank: f32,
    /// Units within this many tiles of an own building do not avoid: they defend it (0 = off).
    pub(crate) defend: f32,
    /// Focus weighs an armored target by the armor face it shows (side 1.5, rear 1.7).
    pub(crate) facing_focus: bool,
    /// A standing Tank turns its hull to an enemy Tank within this many tiles (and farther than
    /// 7) that it shows its side or rear to (0 = off).
    pub(crate) face: f32,
    /// An avoiding unit stays when at least this many own Panzerfausts are within `near` tiles
    /// of it: it screens them while they hit the Tank (0 = off).
    pub(crate) pzf_screen: u32,
    /// Focus slack applies to soft targets that outrange infantry too (off: infantry steps past
    /// its reach only toward targets it outranges or matches; lab and in-game neutral).
    pub(crate) slack_outranged: bool,
    /// Siege: with no enemy Tank about, a Tank stops this many tiles short of the nearest enemy
    /// infantry and shells it from outside its reach (the cannon ramps to 14 tiles standing; a
    /// Tank that drives on stops only at its 5-6 tile moving reach, inside theirs) (0 = off).
    pub(crate) siege: f32,
    /// A Tank below this HP share that enemies are shooting backs out behind a healthier own
    /// Tank within 8 tiles (0 = off).
    pub(crate) pull_tank: f32,
    /// Fall back when the own force nearby is weaker than the enemy force by this ratio (0 = off).
    pub(crate) odds: f32,
    /// Infantry stays out of the reach of a set-up enemy Machine Gunner while an own Tank is
    /// within this many tiles to shell it instead (0 = off).
    pub(crate) mg_wait: f32,
    /// Gather: a unit ahead of its group, not yet under fire, waits while own units still on the
    /// move within this many tiles behind it are worth at least half of what stands with it
    /// (0 = off).
    pub(crate) gather: f32,
    /// Join: a unit out of contact walks to an enemy that own units are fighting within this
    /// many tiles (0 = off).
    pub(crate) join: f32,
    /// Scout Cars always leave enemy Tanks' reach: no own-Tank screen, no defending.
    pub(crate) scout_free: bool,
    /// A Tank another system has on a raid or a building (reserved or directed) comes under the
    /// micro while an enemy Tank is within this many tiles of it (0 = off): it fights the Tank
    /// instead of shooting the mine while the Tank shoots it.
    pub(crate) tank_break: f32,
    /// ...and the same for the other kinds the micro steers, against any enemy fighter
    /// (0 = off).
    pub(crate) unit_break: f32,
    /// Face turns toward enemy Tanks no nearer than this (a Point turn keeps the cannon's ramp
    /// and the Tank fires while turning, so near Tanks can be faced too).
    pub(crate) face_min: f32,
    /// Face only Tanks that have stood this many ticks (a moving Tank is left to its path).
    pub(crate) face_still: u32,
    /// Mass: Riflemen outside a standing enemy Machine Gunner's reach wait there until this many
    /// Riflemen per Machine Gunner stand ready near it, then all go in on it (0 = off).
    pub(crate) mass: f32,
    /// How far past the Machine Gunner's reach a Rifleman counts as ready (tiles).
    pub(crate) mass_ring: f32,
    /// Join skips, for units that avoid Tanks, enemies inside an enemy Tank's reach (plus the
    /// avoid margin) unless an own Tank is near that enemy.
    pub(crate) join_safe: bool,
    /// Mass also near own buildings (off: defenders go straight in, the gun is shooting the base).
    pub(crate) mass_home: bool,
    /// Join does not send infantry at an enemy within this many tiles past a standing enemy
    /// Machine Gunner's reach (one by one into a gun it outranges); 0 = off.
    pub(crate) join_no_mg: f32,
    /// No join for a unit within this many tiles of an own building (home defenders keep their
    /// posts instead of walking to the next fight); 0 = off.
    pub(crate) join_home: f32,
    /// `standfire`: a Tank already standing at full reach is not held again by the Stand rule; it
    /// focuses what is in its reach, else keeps standing without a fresh Hold.
    pub(crate) stand_fire: bool,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            avoid: true,
            avoid_margin: 3.0,
            support: 0.0,
            near: 8.0,
            kite: true,
            kite_margin: 0.75,
            kite_rifles: false,
            focus: true,
            focus_keep: 1.3,
            pull: 0.0,
            charge: 0.0,
            stand: 6.0,
            stand_moving: true,
            focus_slack: 1.0,
            focus_slack_vehicle: 0.5,
            tank_focus: 1,
            kite_margin_scout: 1.5,
            avoid_parked: true,
            avoid_hold: 3.0,
            charge_no_tank: true,
            contact: 10.0,
            reorder: 15,
            release: 45,
            tanks: true,
            rifles: true,
            scouts: true,
            eco: 150,
            respect_retreat: true,
            mg_focus: 0,
            mg_stand: 0.0,
            focus_slack_tank: 0.5,
            defend: 15.0,
            facing_focus: true,
            face: 16.0,
            pzf_screen: 0,
            slack_outranged: true,
            siege: 0.0,
            pull_tank: 0.0,
            odds: 0.0,
            mg_wait: 0.0,
            gather: 0.0,
            join: 0.0,
            scout_free: false,
            tank_break: 0.0,
            unit_break: 0.0,
            face_min: 7.0,
            face_still: 30,
            mass: 0.0,
            mass_ring: 4.0,
            join_safe: false,
            mass_home: true,
            join_no_mg: 0.0,
            join_home: 0.0,
            stand_fire: false,
        }
    }
}

impl Params {
    /// Everything off: plain attack-move with the simulation's own targeting.
    pub(crate) fn none() -> Self {
        Self {
            avoid: false,
            kite: false,
            kite_rifles: false,
            focus: false,
            pull: 0.0,
            charge: 0.0,
            stand: 0.0,
            ..Self::default()
        }
    }

    /// `key=value` pairs over the defaults; `none` first starts from everything off.
    pub(crate) fn parse(spec: &str) -> Result<Self, String> {
        let mut params = Self::default();
        for pair in spec.split(',').map(str::trim).filter(|pair| !pair.is_empty()) {
            if pair == "none" {
                params = Self::none();
                continue;
            }
            let (key, value) = pair
                .split_once('=')
                .ok_or_else(|| format!("expected key=value, got {pair:?}"))?;
            let number: f32 = value
                .parse()
                .map_err(|_| format!("bad number in {pair:?}"))?;
            let flag = number != 0.0;
            match key {
                "avoid" => params.avoid = flag,
                "am" => params.avoid_margin = number,
                "support" => params.support = number,
                "near" => params.near = number,
                "kite" => params.kite = flag,
                "km" => params.kite_margin = number,
                "kr" => params.kite_rifles = flag,
                "focus" => params.focus = flag,
                "fk" => params.focus_keep = number,
                "pull" => params.pull = number,
                "charge" => params.charge = number,
                "stand" => params.stand = number,
                "sm" => params.stand_moving = flag,
                "fr" => params.focus_slack = number,
                "frv" => params.focus_slack_vehicle = number,
                "tf" => params.tank_focus = number as u32,
                "kms" => params.kite_margin_scout = number,
                "ap" => params.avoid_parked = flag,
                "ah" => params.avoid_hold = number,
                "cnt" => params.charge_no_tank = flag,
                "contact" => params.contact = number,
                "reorder" => params.reorder = number as u32,
                "release" => params.release = number as u32,
                "tanks" => params.tanks = flag,
                "rifles" => params.rifles = flag,
                "scouts" => params.scouts = flag,
                "eco" => params.eco = number as u32,
                "rr" => params.respect_retreat = flag,
                "mgf" => params.mg_focus = number as u32,
                "mgs" => params.mg_stand = number,
                "frt" => params.focus_slack_tank = number,
                "defend" => params.defend = number,
                "ff" => params.facing_focus = flag,
                "face" => params.face = number,
                "nearp" => params.pzf_screen = number as u32,
                "fso" => params.slack_outranged = flag,
                "siege" => params.siege = number,
                "pullt" => params.pull_tank = number,
                "odds" => params.odds = number,
                "mgw" => params.mg_wait = number,
                "gather" => params.gather = number,
                "join" => params.join = number,
                "sf" => params.scout_free = flag,
                "tb" => params.tank_break = number,
                "ub" => params.unit_break = number,
                "fmin" => params.face_min = number,
                "fst" => params.face_still = number as u32,
                "mass" => params.mass = number,
                "mring" => params.mass_ring = number,
                "js" => params.join_safe = flag,
                "mh" => params.mass_home = flag,
                "jnm" => params.join_no_mg = number,
                "jh" => params.join_home = number,
                "stfire" => params.stand_fire = flag,
                other => return Err(format!("unknown matchup key {other:?}")),
            }
        }
        Ok(params)
    }

    /// Whether the micro steers this own kind under these parameters.
    pub(crate) fn steers_kind(&self, kind: EntityKind) -> bool {
        match kind {
            EntityKind::Tank => self.tanks,
            EntityKind::ScoutCar => self.scouts,
            EntityKind::Rifleman | EntityKind::Panzerfaust | EntityKind::MachineGunner => {
                self.rifles
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Mode {
    Avoid,
    Kite,
    Focus,
    Pull,
    Charge,
    Stand,
    Gather,
    Join,
}

impl Mode {
    pub(crate) const ALL: [Mode; 8] = [
        Mode::Avoid,
        Mode::Kite,
        Mode::Focus,
        Mode::Pull,
        Mode::Charge,
        Mode::Stand,
        Mode::Gather,
        Mode::Join,
    ];

    pub(crate) fn short(self) -> &'static str {
        match self {
            Mode::Avoid => "av",
            Mode::Kite => "ki",
            Mode::Focus => "fo",
            Mode::Pull => "pu",
            Mode::Charge => "ch",
            Mode::Stand => "st",
            Mode::Gather => "ga",
            Mode::Join => "jo",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Order {
    Move(f32, f32),
    Attack(u32),
    Hold,
    /// Turn a Tank's hull toward a point.
    Point(f32, f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Track {
    pos: (f32, f32),
    still_since: u32,
    seen: u32,
    /// Unit direction of its last move (or of the last turn the micro ordered): its hull facing.
    heading: Option<(f32, f32)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Held {
    mode: Mode,
    last_contact: u32,
    ordered: u32,
    order: Option<Order>,
}

/// Where another system last sent a unit: attack-move or move, and the point.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Goal {
    attack: bool,
    x: f32,
    y: f32,
}

/// What the micro remembers between decisions.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct MatchupMemory {
    tracks: BTreeMap<u32, Track>,
    held: BTreeMap<u32, Held>,
    goals: BTreeMap<u32, Goal>,
    /// Units on a raid or clearing order: until which tick, and the target they were sent at.
    directed: BTreeMap<u32, (u32, Option<u32>)>,
    /// Decisions a unit in contact was left on Jeff's retreat order, for the trace.
    retreats: u32,
    /// Orders given per tactic over the game, for the trace.
    pub(crate) orders: BTreeMap<Mode, u32>,
}

// Positions are never NaN, so equality is total.
impl Eq for MatchupMemory {}

impl MatchupMemory {
    pub(crate) fn holds(&self, unit: u32) -> bool {
        self.held.contains_key(&unit)
    }

    pub(crate) fn trace(&self) -> String {
        let counts: Vec<String> = Mode::ALL
            .iter()
            .map(|mode| format!("{}{}", mode.short(), self.orders.get(mode).copied().unwrap_or(0)))
            .collect();
        format!(
            "v1_1_mm held={} {} rt{} dir{}",
            self.held.len(),
            counts.join(" "),
            self.retreats,
            self.directed.len()
        )
    }
}

/// One decision's micro: new orders, the units it steers (strip them from every other order), and
/// the units it let go (give them their other orders back).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Plan {
    pub(crate) orders: Vec<(u32, Order)>,
    pub(crate) held: BTreeSet<u32>,
    pub(crate) released: Vec<u32>,
}

/// The kinds the micro steers.
fn steers(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Rifleman
            | EntityKind::Panzerfaust
            | EntityKind::MachineGunner
            | EntityKind::Tank
            | EntityKind::ScoutCar
    )
}

/// Enemy kinds that fight (workers and buildings are left to the other systems).
fn fights(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Rifleman
            | EntityKind::Panzerfaust
            | EntityKind::MachineGunner
            | EntityKind::Tank
            | EntityKind::ScoutCar
            | EntityKind::AntiTankGun
            | EntityKind::MortarTeam
            | EntityKind::Warrior
            | EntityKind::Golem
            | EntityKind::Ekat
    )
}

fn infantry(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Rifleman | EntityKind::Panzerfaust | EntityKind::MachineGunner
    )
}

fn vehicle(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Tank
            | EntityKind::ScoutCar
            | EntityKind::CommandCar
            | EntityKind::RocketLauncher
            | EntityKind::Artillery
    )
}

fn armored(kind: EntityKind) -> bool {
    rts_rules::combat::is_armored(kind)
}

/// The steel plus oil a unit costs.
pub(crate) fn cost(kind: EntityKind) -> f32 {
    rts_rules::balance::unit_stats(kind).map_or(0.0, |s| (s.cost_steel + s.cost_oil) as f32)
}

/// Reach in tiles of `attacker` against `target`; a Tank's cannon ramps from 5 to 14 tiles over
/// its first three seconds standing still, and its coax reaches 6 against soft targets.
pub(crate) fn reach(attacker: EntityKind, target: EntityKind, still_ticks: u32) -> f32 {
    match attacker {
        EntityKind::Tank => {
            let ramp = (still_ticks as f32 / TANK_RAMP_TICKS).min(1.0);
            let cannon = TANK_BASE_REACH + (TANK_FULL_REACH - TANK_BASE_REACH) * ramp;
            if armored(target) {
                cannon
            } else {
                cannon.max(6.0)
            }
        }
        EntityKind::ScoutCar => 7.0,
        EntityKind::MachineGunner => 6.1,
        EntityKind::Rifleman | EntityKind::Panzerfaust | EntityKind::AntiTankGun => 5.0,
        EntityKind::MortarTeam => 17.0,
        EntityKind::Ekat => 6.0,
        EntityKind::Warrior | EntityKind::Golem | EntityKind::Worker => 1.0,
        _ => 0.0,
    }
}

/// Damage per second of `attacker` against `target`, roughly: Machine-gun bursts are three rays
/// at 60% damage and some miss; a Panzerfaust's single rocket is spread over its reload.
pub(crate) fn dps(attacker: EntityKind, target: EntityKind, meth: bool) -> f32 {
    let hard = armored(target);
    let rifle = if hard { 1.0 } else { 5.0 } * TICK_HZ / if meth { 24.0 } else { 32.0 };
    match attacker {
        EntityKind::Rifleman => rifle,
        EntityKind::Panzerfaust => rifle + if vehicle(target) { 10.0 } else { 0.0 },
        EntityKind::MachineGunner => {
            if hard {
                3.0
            } else {
                12.0
            }
        }
        EntityKind::ScoutCar => {
            if hard {
                4.5
            } else {
                19.0
            }
        }
        EntityKind::Tank => {
            if hard {
                12.5
            } else {
                25.0
            }
        }
        EntityKind::AntiTankGun => {
            if hard {
                20.8
            } else {
                6.0
            }
        }
        EntityKind::MortarTeam => 6.0,
        EntityKind::Warrior => 10.0,
        EntityKind::Golem => 10.0,
        EntityKind::Ekat => 8.0,
        EntityKind::Worker => {
            if hard {
                0.6
            } else {
                2.5
            }
        }
        _ => 0.0,
    }
}

/// The pairs where the first kind should not fight the second at all.
fn avoids(own: EntityKind, enemy: EntityKind) -> bool {
    enemy == EntityKind::Tank
        && matches!(
            own,
            EntityKind::Rifleman | EntityKind::MachineGunner | EntityKind::ScoutCar
        )
}

/// Fires on the move without losing its rate of fire.
fn fires_moving(kind: EntityKind, meth: bool, params: &Params) -> bool {
    vehicle(kind) || (params.kite_rifles && meth && matches!(kind, EntityKind::Rifleman))
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    (dx * dx + dy * dy).det_sqrt()
}

fn unit_dir(from: (f32, f32), to: (f32, f32)) -> (f32, f32) {
    let length = dist(from, to);
    if length < 1e-3 {
        (0.0, 0.0)
    } else {
        ((to.0 - from.0) / length, (to.1 - from.1) / length)
    }
}

fn normalized(v: (f32, f32)) -> (f32, f32) {
    unit_dir((0.0, 0.0), v)
}

/// Updates where every visible enemy stands and since when it stood still.
fn track(observation: &AiObservation, memory: &mut MatchupMemory, ts: f32) {
    let held = &memory.held;
    let tick = observation.tick;
    for enemy in observation.visible_enemies.iter().chain(&observation.owned) {
        let pos = (enemy.x, enemy.y);
        memory
            .tracks
            .entry(enemy.id)
            .and_modify(|track| {
                if dist(track.pos, pos) > STILL_TILES * ts {
                    // A Tank kiting under the micro backs up: its hull still faces where it came
                    // from.
                    let reversing = held.get(&enemy.id).is_some_and(|h| h.mode == Mode::Kite);
                    track.heading = Some(if reversing {
                        unit_dir(pos, track.pos)
                    } else {
                        unit_dir(track.pos, pos)
                    });
                    track.still_since = tick;
                }
                track.pos = pos;
                track.seen = tick;
            })
            .or_insert(Track {
                pos,
                // First seen not moving: assume it has stood (a parked Tank is already charged).
                still_since: if enemy.state == AiEntityState::Move {
                    tick
                } else {
                    tick.saturating_sub(TANK_RAMP_TICKS as u32)
                },
                seen: tick,
                heading: None,
            });
    }
    memory
        .tracks
        .retain(|_, track| tick.saturating_sub(track.seen) <= TRACK_TICKS);
}

/// The armor multiplier a shot from `attacker` takes against a Tank at `victim` with hull facing
/// `heading` (front 45 degrees each way, side to 135, rear beyond).
fn facing_mult(heading: (f32, f32), victim: (f32, f32), attacker: (f32, f32)) -> f32 {
    let to = unit_dir(victim, attacker);
    let cos = heading.0 * to.0 + heading.1 * to.1;
    if cos >= std::f32::consts::FRAC_1_SQRT_2 {
        1.0
    } else if cos >= -std::f32::consts::FRAC_1_SQRT_2 {
        1.5
    } else {
        1.7
    }
}

fn heading_of(memory: &MatchupMemory, id: u32) -> Option<(f32, f32)> {
    memory.tracks.get(&id).and_then(|track| track.heading)
}

fn still_ticks(memory: &MatchupMemory, enemy: u32, tick: u32) -> u32 {
    memory
        .tracks
        .get(&enemy)
        .map_or(0, |track| tick.saturating_sub(track.still_since))
}

struct Ctx<'a> {
    observation: &'a AiObservation,
    params: &'a Params,
    memory: &'a MatchupMemory,
    ts: f32,
    meth: bool,
    enemy_meth: bool,
    home: (f32, f32),
    own_tanks: Vec<(f32, f32)>,
    own_buildings: Vec<(f32, f32)>,
    own_pzf: Vec<(f32, f32)>,
    /// Own Tanks: id, place, HP share.
    own_tank_hp: Vec<(u32, (f32, f32), f32)>,
    /// Own fighting units: place, kind, HP.
    own_fighters: Vec<((f32, f32), EntityKind, f32)>,
    /// Own fighting units on the move: place, kind.
    own_moving: Vec<((f32, f32), EntityKind)>,
    /// Every visible enemy fighter: place, kind, HP.
    enemy_fighters: Vec<((f32, f32), EntityKind, f32)>,
    map_px: (f32, f32),
}

impl Ctx<'_> {
    fn clamp(&self, point: (f32, f32)) -> (f32, f32) {
        let margin = self.ts;
        (
            point.0.clamp(margin, (self.map_px.0 - margin).max(margin)),
            point.1.clamp(margin, (self.map_px.1 - margin).max(margin)),
        )
    }

    /// Moving now, or moved within the last 1.5 seconds.
    fn moving(&self, enemy: &AiEntitySummary) -> bool {
        enemy.state == AiEntityState::Move
            || still_ticks(self.memory, enemy.id, self.observation.tick) < 45
    }

    fn enemy_reach(&self, enemy: &AiEntitySummary, target: EntityKind) -> f32 {
        reach(
            enemy.kind,
            target,
            still_ticks(self.memory, enemy.id, self.observation.tick),
        )
    }

    /// A point `tiles` away from `from` in the direction away from `threat`, leaning home.
    fn away(&self, from: (f32, f32), threat: (f32, f32), tiles: f32, lean_home: f32) -> (f32, f32) {
        let away = unit_dir(threat, from);
        let home = unit_dir(from, self.home);
        let dir = normalized((away.0 + home.0 * lean_home, away.1 + home.1 * lean_home));
        let dir = if dir == (0.0, 0.0) { home } else { dir };
        self.clamp((from.0 + dir.0 * tiles * self.ts, from.1 + dir.1 * tiles * self.ts))
    }
}

/// Plans one decision's micro for the own units `eligible` allows; `home` is where fleeing units
/// lean toward.
pub(crate) fn plan(
    observation: &AiObservation,
    params: &Params,
    memory: &mut MatchupMemory,
    eligible: &dyn Fn(u32) -> bool,
    home: (f32, f32),
) -> Plan {
    let ts = observation.map.tile_size.max(1) as f32;
    let tick = observation.tick;
    track(observation, memory, ts);
    let enemies: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.is_complete && fights(enemy.kind))
        .collect();
    let ctx = Ctx {
        observation,
        params,
        memory,
        ts,
        meth: observation.upgrades.contains(&UpgradeKind::Methamphetamines),
        // Unknown for the enemy: assume it has it, the worse case for our margins.
        enemy_meth: true,
        home,
        own_tanks: observation
            .owned
            .iter()
            .filter(|unit| unit.kind == EntityKind::Tank && unit.hp > 0)
            .map(|unit| (unit.x, unit.y))
            .collect(),
        own_pzf: observation
            .owned
            .iter()
            .filter(|unit| unit.kind == EntityKind::Panzerfaust && unit.hp > 0)
            .map(|unit| (unit.x, unit.y))
            .collect(),
        own_buildings: observation
            .owned
            .iter()
            .filter(|unit| unit.kind.is_building() && unit.hp > 0)
            .map(|unit| (unit.x, unit.y))
            .collect(),
        own_tank_hp: observation
            .owned
            .iter()
            .filter(|unit| unit.kind == EntityKind::Tank && unit.hp > 0)
            .map(|unit| {
                let max = rts_rules::balance::unit_stats(unit.kind).map_or(unit.hp.max(1), |s| s.hp.max(1));
                (unit.id, (unit.x, unit.y), unit.hp as f32 / max as f32)
            })
            .collect(),
        own_fighters: observation
            .owned
            .iter()
            .filter(|unit| unit.hp > 0 && unit.is_complete && steers(unit.kind))
            .map(|unit| ((unit.x, unit.y), unit.kind, unit.hp as f32))
            .collect(),
        own_moving: observation
            .owned
            .iter()
            .filter(|unit| {
                unit.hp > 0 && unit.is_complete && steers(unit.kind) && unit.state == AiEntityState::Move
            })
            .map(|unit| ((unit.x, unit.y), unit.kind))
            .collect(),
        enemy_fighters: enemies
            .iter()
            .map(|enemy| ((enemy.x, enemy.y), enemy.kind, enemy.hp as f32))
            .collect(),
        map_px: (
            observation.map.width as f32 * ts,
            observation.map.height as f32 * ts,
        ),
    };
    let mut assigned: BTreeMap<u32, f32> = BTreeMap::new();
    let mut decisions: Vec<(u32, Option<(Mode, Option<Order>)>)> = Vec::new();
    let mut retreating: Vec<u32> = Vec::new();
    for unit in observation
        .owned
        .iter()
        .filter(|unit| {
            unit.hp > 0
                && unit.is_complete
                && steers(unit.kind)
                && params.steers_kind(unit.kind)
                && eligible(unit.id)
        })
    {
        let pos = (unit.x, unit.y);
        let contact: Vec<&AiEntitySummary> = enemies
            .iter()
            .copied()
            .filter(|enemy| {
                let d = dist(pos, (enemy.x, enemy.y));
                d <= params.contact * ts
                    || (unit.kind == EntityKind::Tank
                        && enemy.kind == EntityKind::Tank
                        && d <= params.face * ts)
                    || (avoids(unit.kind, enemy.kind)
                        && d <= (ctx.enemy_reach(enemy, unit.kind) + params.avoid_margin + params.avoid_hold + 1.0) * ts)
            })
            .collect();
        if contact.is_empty() {
            // Join: walk to the nearest enemy that own units are fighting.
            let target = (params.join > 0.0
                && !(params.join_home > 0.0
                    && ctx
                        .own_buildings
                        .iter()
                        .any(|building| dist(*building, pos) <= params.join_home * ts)))
                .then(|| {
                    enemies
                        .iter()
                        .filter(|enemy| {
                            let at = (enemy.x, enemy.y);
                            dist(pos, at) <= params.join * ts
                                && !avoids(unit.kind, enemy.kind)
                                && dps(unit.kind, enemy.kind, ctx.meth) >= 0.5
                                && ctx.own_fighters.iter().any(|own| {
                                    own.0 != pos && dist(own.0, at) <= params.contact * ts
                                })
                                && !(params.join_no_mg > 0.0
                                    && infantry(unit.kind)
                                    && enemies.iter().any(|gun| {
                                        gun.kind == EntityKind::MachineGunner
                                            && !ctx.moving(gun)
                                            && dist((gun.x, gun.y), at)
                                                <= (ctx.enemy_reach(gun, unit.kind) + params.join_no_mg) * ts
                                    }))
                                && !(params.join_safe
                                    && enemies.iter().any(|tank| {
                                        avoids(unit.kind, tank.kind)
                                            && dist((tank.x, tank.y), at)
                                                <= (ctx.enemy_reach(tank, unit.kind) + params.avoid_margin) * ts
                                    })
                                    && !ctx
                                        .own_tanks
                                        .iter()
                                        .any(|own| dist(*own, at) <= params.near * ts))
                        })
                        .min_by(|a, b| dist(pos, (a.x, a.y)).total_cmp(&dist(pos, (b.x, b.y))))
                })
                .flatten();
            if let Some(target) = target {
                let order = (unit.target_id != Some(target.id)).then_some(Order::Attack(target.id));
                decisions.push((unit.id, Some((Mode::Join, order))));
                continue;
            }
            decisions.push((unit.id, None));
            continue;
        }
        // A unit Jeff is walking away from the fight (a plain move, still on its way, to a point
        // farther from the nearest enemy) keeps that order.
        if params.respect_retreat {
            let nearest = contact
                .iter()
                .map(|enemy| (enemy.x, enemy.y))
                .min_by(|a, b| dist(pos, *a).total_cmp(&dist(pos, *b)));
            let away = ctx.memory.goals.get(&unit.id).is_some_and(|goal| {
                let target = (goal.x, goal.y);
                !goal.attack
                    && dist(pos, target) > 2.0 * ts
                    && nearest.is_some_and(|enemy| dist(target, enemy) > dist(pos, enemy) + 2.0 * ts)
            });
            if away {
                retreating.push(unit.id);
                continue;
            }
        }
        decisions.push((unit.id, decide(&ctx, unit, &contact, &mut assigned)));
    }
    let mut plan = Plan::default();
    for id in &retreating {
        // Held until now: its retreat order may have been stripped, so it gets it back.
        if memory.held.remove(id).is_some() {
            plan.released.push(*id);
        }
        memory.retreats += 1;
    }
    for (id, decision) in decisions {
        match decision {
            None => {
                if let Some(held) = memory.held.get(&id) {
                    if tick.saturating_sub(held.last_contact) >= params.release {
                        memory.held.remove(&id);
                        plan.released.push(id);
                    } else {
                        plan.held.insert(id);
                    }
                }
            }
            Some((mode, order)) => {
                let entry = memory.held.entry(id).or_insert(Held {
                    mode,
                    last_contact: tick,
                    ordered: 0,
                    order: None,
                });
                entry.last_contact = tick;
                plan.held.insert(id);
                let Some(order) = order else {
                    entry.mode = mode;
                    continue;
                };
                let same = match (entry.order, order) {
                    (Some(Order::Move(x0, y0)), Order::Move(x1, y1)) => {
                        dist((x0, y0), (x1, y1)) <= 0.75 * ts
                    }
                    (Some(previous), next) => previous == next,
                    (None, _) => false,
                };
                let since = tick.saturating_sub(entry.ordered);
                let due = if same {
                    since >= params.reorder.saturating_mul(4)
                } else {
                    entry.mode != mode || since >= params.reorder
                };
                if due {
                    entry.mode = mode;
                    entry.ordered = tick;
                    entry.order = Some(order);
                    *memory.orders.entry(mode).or_default() += 1;
                    if let Order::Point(x, y) = order {
                        if let Some(track) = memory.tracks.get_mut(&id) {
                            track.heading = Some(unit_dir(track.pos, (x, y)));
                        }
                    }
                    plan.orders.push((id, order));
                }
            }
        }
    }
    // Units that died or left the eligible set are forgotten.
    let alive: BTreeSet<u32> = observation.owned.iter().map(|unit| unit.id).collect();
    memory.held.retain(|id, _| alive.contains(id) && eligible(*id));
    plan
}

/// One unit's tactic: `None` leaves it to its other orders; `Some((mode, None))` keeps it held on
/// its current order.
fn decide(
    ctx: &Ctx<'_>,
    unit: &AiEntitySummary,
    contact: &[&AiEntitySummary],
    assigned: &mut BTreeMap<u32, f32>,
) -> Option<(Mode, Option<Order>)> {
    let params = ctx.params;
    let ts = ctx.ts;
    let pos = (unit.x, unit.y);
    let max_hp = rts_rules::balance::unit_stats(unit.kind).map_or(unit.hp.max(1), |s| s.hp.max(1));
    let d = |enemy: &AiEntitySummary| dist(pos, (enemy.x, enemy.y));
    let own_still = still_ticks(ctx.memory, unit.id, ctx.observation.tick);
    // `standfire`: a ramped Tank the Stand rule would hold, left to focus first.
    let mut stand_ready = false;

    // Avoid: leave the reach of what this unit cannot hurt, unless an own Tank screens it.
    let mut wary = false;
    let defending = params.defend > 0.0
        && ctx
            .own_buildings
            .iter()
            .any(|building| dist(*building, pos) <= params.defend * ts);

    // Odds: the force around this unit against the enemy force around it (hit points times the
    // damage each side does to the mix it faces); badly outmatched, it falls back.
    if params.odds > 0.0 && !defending {
        let ours: Vec<&((f32, f32), EntityKind, f32)> = ctx
            .own_fighters
            .iter()
            .filter(|own| dist(own.0, pos) <= 10.0 * ts)
            .collect();
        let theirs: Vec<&((f32, f32), EntityKind, f32)> = ctx
            .enemy_fighters
            .iter()
            .filter(|enemy| dist(enemy.0, pos) <= 14.0 * ts)
            .collect();
        if !ours.is_empty() && !theirs.is_empty() {
            let power = |side: &[&((f32, f32), EntityKind, f32)], other: &[&((f32, f32), EntityKind, f32)], meth: bool| -> f32 {
                let hp: f32 = side.iter().map(|unit| unit.2).sum();
                let rate: f32 = side
                    .iter()
                    .map(|unit| other.iter().map(|foe| dps(unit.1, foe.1, meth)).sum::<f32>() / other.len() as f32)
                    .sum();
                hp * rate
            };
            let mine = power(&ours, &theirs, ctx.meth);
            let foe = power(&theirs, &ours, ctx.enemy_meth);
            if mine < foe * params.odds * params.odds {
                let (sx, sy) = theirs.iter().fold((0.0, 0.0), |acc, enemy| (acc.0 + enemy.0 .0, acc.1 + enemy.0 .1));
                let centroid = (sx / theirs.len() as f32, sy / theirs.len() as f32);
                let step = if unit.kind == EntityKind::Tank { REVERSE_HOP_TILES } else { 5.0 };
                let point = ctx.away(pos, centroid, step, 0.5);
                return Some((Mode::Avoid, Some(Order::Move(point.0, point.1))));
            }
        }
    }

    // MG wait: infantry leaves a set-up Machine Gunner to the Tanks beside it.
    if params.mg_wait > 0.0
        && infantry(unit.kind)
        && !defending
        && ctx.own_tanks.iter().any(|tank| dist(*tank, pos) <= params.mg_wait * ts)
    {
        let nest = contact
            .iter()
            .filter(|enemy| {
                enemy.kind == EntityKind::MachineGunner
                    && !ctx.moving(enemy)
                    && d(enemy) <= (ctx.enemy_reach(enemy, unit.kind) + 1.5) * ts
            })
            .min_by(|a, b| d(a).total_cmp(&d(b)));
        if let Some(nest) = nest {
            let needed = ctx.enemy_reach(nest, unit.kind) + 1.5 - d(nest) / ts;
            let point = ctx.away(pos, (nest.x, nest.y), (needed + 1.0).clamp(1.5, 4.0), 0.2);
            return Some((Mode::Avoid, Some(Order::Move(point.0, point.1))));
        }
    }
    let pzf_screened = params.pzf_screen > 0
        && params.near > 0.0
        && ctx
            .own_pzf
            .iter()
            .filter(|pzf| dist(**pzf, pos) <= params.near * ts)
            .count()
            >= params.pzf_screen as usize;
    let free_scout = params.scout_free && unit.kind == EntityKind::ScoutCar;
    if params.avoid && (free_scout || (!defending && !pzf_screened)) {
        let danger: Vec<&&AiEntitySummary> = contact
            .iter()
            .filter(|enemy| {
                avoids(unit.kind, enemy.kind)
                    && (params.avoid_parked || ctx.moving(enemy))
                    && d(enemy) <= (ctx.enemy_reach(enemy, unit.kind) + params.avoid_margin) * ts
            })
            .collect();
        // Out of reach but still near: stay held (no walking back in) while the threat lingers.
        let watched = contact.iter().copied().find(|enemy| {
            avoids(unit.kind, enemy.kind)
                && (params.avoid_parked || ctx.moving(enemy))
                && d(enemy)
                    <= (ctx.enemy_reach(enemy, unit.kind) + params.avoid_margin + params.avoid_hold)
                        * ts
        });
        if let Some(enemy) = watched {
            let threat = (enemy.x, enemy.y);
            wary = params.avoid_hold > 0.0
                && (free_scout || !((params.support > 0.0
                    && ctx.own_tanks.iter().any(|tank| {
                        dist(*tank, pos) <= 10.0 * ts
                            && dist(*tank, threat) + params.support * ts <= dist(pos, threat)
                    }))
                    || (params.near > 0.0
                        && ctx
                            .own_tanks
                            .iter()
                            .any(|tank| dist(*tank, pos) <= params.near * ts))));
        }
        if let Some(nearest) = danger
            .iter()
            .min_by(|a, b| d(a).total_cmp(&d(b)))
            .copied()
        {
            let threat = (nearest.x, nearest.y);
            let screened = (params.support > 0.0
                && ctx.own_tanks.iter().any(|tank| {
                    dist(*tank, pos) <= 10.0 * ts
                        && dist(*tank, threat) + params.support * ts <= dist(pos, threat)
                }))
                || (params.near > 0.0
                    && ctx
                        .own_tanks
                        .iter()
                        .any(|tank| dist(*tank, pos) <= params.near * ts));
            if free_scout || !screened {
                let (sx, sy, n) = danger.iter().fold((0.0, 0.0, 0.0), |acc, enemy| {
                    (acc.0 + enemy.x, acc.1 + enemy.y, acc.2 + 1.0)
                });
                let centroid = (sx / n, sy / n);
                let needed = danger
                    .iter()
                    .map(|enemy| {
                        ctx.enemy_reach(enemy, unit.kind) + params.avoid_margin - d(enemy) / ts
                    })
                    .fold(0.0_f32, f32::max);
                let step = (needed + 2.0).clamp(2.0, 8.0);
                let point = ctx.away(pos, centroid, step, 0.35);
                return Some((Mode::Avoid, Some(Order::Move(point.0, point.1))));
            }
        }
    }

    // Pull: wounded infantry under fire steps back so the shooter retargets.
    if params.pull > 0.0 && infantry(unit.kind) && (unit.hp as f32) < params.pull * max_hp as f32 {
        let shooters: Vec<&&AiEntitySummary> = contact
            .iter()
            .filter(|enemy| enemy.target_id == Some(unit.id))
            .collect();
        if let Some(shooter) = shooters.first() {
            let point = ctx.away(pos, (shooter.x, shooter.y), 3.0, 0.5);
            return Some((Mode::Pull, Some(Order::Move(point.0, point.1))));
        }
    }

    // Tank pull: a badly hurt Tank under fire backs out behind a healthier one.
    if params.pull_tank > 0.0 && unit.kind == EntityKind::Tank {
        let share = unit.hp as f32 / max_hp as f32;
        if share < params.pull_tank {
            let shooters: Vec<&&AiEntitySummary> = contact
                .iter()
                .filter(|enemy| enemy.target_id == Some(unit.id))
                .collect();
            let cover = ctx.own_tank_hp.iter().any(|(id, at, other)| {
                *id != unit.id && dist(*at, pos) <= 8.0 * ts && *other >= share + 0.25
            });
            if !shooters.is_empty() && cover {
                let (sx, sy) = shooters.iter().fold((0.0, 0.0), |acc, enemy| (acc.0 + enemy.x, acc.1 + enemy.y));
                let centroid = (sx / shooters.len() as f32, sy / shooters.len() as f32);
                let point = ctx.away(pos, centroid, REVERSE_HOP_TILES, 0.0);
                return Some((Mode::Pull, Some(Order::Move(point.0, point.1))));
            }
        }
    }

    // Kite: keep the range gap over what this unit outranges.
    if params.kite && fires_moving(unit.kind, ctx.meth, params) {
        // A Tank inside an enemy Tank's reach fights it rather than backing off infantry.
        let tank_fight = unit.kind == EntityKind::Tank
            && contact.iter().any(|enemy| {
                enemy.kind == EntityKind::Tank
                    && d(enemy) <= (ctx.enemy_reach(enemy, unit.kind) + 1.0).max(10.0) * ts
            });
        let margin = if unit.kind == EntityKind::ScoutCar {
            params.kite_margin_scout
        } else {
            params.kite_margin
        };
        if !tank_fight {
            let outranged = contact
                .iter()
                .filter(|enemy| {
                    let mine = reach(unit.kind, enemy.kind, 0);
                    let theirs = ctx.enemy_reach(enemy, unit.kind);
                    mine >= theirs + 0.5 && dps(unit.kind, enemy.kind, ctx.meth) >= 2.0
                })
                .min_by(|a, b| d(a).total_cmp(&d(b)));
            if let Some(enemy) = outranged {
                let mine = reach(unit.kind, enemy.kind, 0);
                let theirs = ctx.enemy_reach(enemy, unit.kind);
                let keep = (theirs + margin).min(mine - 0.25);
                let now = d(enemy) / ts;
                if now < keep {
                    let cap = if unit.kind == EntityKind::Tank {
                        REVERSE_HOP_TILES
                    } else {
                        3.0
                    };
                    let step = (keep - now + 0.75).min(cap);
                    let point = ctx.away(pos, (enemy.x, enemy.y), step, 0.0);
                    return Some((Mode::Kite, Some(Order::Move(point.0, point.1))));
                }
            }
        }
    }

    // Face: a standing Tank turns its front to an enemy Tank before it comes close.
    if params.face > 0.0 && unit.kind == EntityKind::Tank && own_still >= params.face_still {
        // A hull never seen moving is turned once too.
        if let (heading, Some(enemy)) = (
            heading_of(ctx.memory, unit.id),
            contact
                .iter()
                .filter(|enemy| enemy.kind == EntityKind::Tank)
                .min_by(|a, b| d(a).total_cmp(&d(b))),
        ) {
            let tiles = d(enemy) / ts;
            if tiles > params.face_min
                && tiles <= params.face
                && heading.is_none_or(|heading| facing_mult(heading, pos, (enemy.x, enemy.y)) > 1.0)
            {
                return Some((Mode::Stand, Some(Order::Point(enemy.x, enemy.y))));
            }
        }
    }

    // Stand: a Tank in sight of enemy Tanks but not yet close stops to ramp its cannon.
    if params.stand > 0.0 && unit.kind == EntityKind::Tank {
        // Only against Tanks on the move: a parked Tank already outranges a stopping one, and
        // standing inside its reach just takes the shells.
        let nearest_tank = contact
            .iter()
            .filter(|enemy| {
                enemy.kind == EntityKind::Tank && (!params.stand_moving || ctx.moving(enemy))
            })
            .map(|enemy| d(enemy) / ts)
            .fold(f32::INFINITY, f32::min);
        if nearest_tank.is_finite() && nearest_tank > params.stand && nearest_tank <= 15.0 {
            // `standfire`: a Tank already standing at full reach fires on what is in it (focus
            // below) and otherwise just keeps standing. A fresh Hold clears its target: five full
            // Tanks held so at 13 tiles from a 131-HP Tank in Jeff's main for 75 s never fired
            // (Wald des Todes vs Pressure AI, 5:30-6:55), the micro stripping every focus order.
            if params.stand_fire
                && unit.state != AiEntityState::Move
                && own_still >= TANK_RAMP_TICKS as u32
            {
                stand_ready = true;
            } else {
                return Some((Mode::Stand, Some(Order::Hold)));
            }
        }
    }

    // Siege: no enemy Tank about, so stop short of the enemy infantry and shell it from outside
    // its reach.
    if params.siege > 0.0
        && unit.kind == EntityKind::Tank
        && !contact.iter().any(|enemy| armored(enemy.kind))
    {
        if let Some(enemy) = contact
            .iter()
            .filter(|enemy| !armored(enemy.kind))
            .min_by(|a, b| d(a).total_cmp(&d(b)))
        {
            let tiles = d(enemy) / ts;
            let theirs = ctx.enemy_reach(enemy, unit.kind);
            if tiles <= params.siege && tiles > theirs + 0.5 {
                if unit.state != AiEntityState::Move && own_still >= 15 {
                    return Some((Mode::Stand, None));
                }
                return Some((Mode::Stand, Some(Order::Hold)));
            }
        }
    }

    // MG stand: set up before enemy infantry walks into reach.
    if params.mg_stand > 0.0 && unit.kind == EntityKind::MachineGunner {
        let nearest = contact
            .iter()
            .filter(|enemy| {
                infantry(enemy.kind)
                    && (ctx.moving(enemy)
                        || d(enemy) <= reach(unit.kind, enemy.kind, own_still) * ts)
            })
            .map(|enemy| d(enemy) / ts)
            .fold(f32::INFINITY, f32::min);
        if nearest.is_finite() && nearest <= params.mg_stand {
            if unit.state == AiEntityState::Idle || unit.target_id.is_some() {
                return Some((Mode::Stand, None));
            }
            return Some((Mode::Stand, Some(Order::Hold)));
        }
    }

    // Mass: no trickling into a Machine Gunner's longer reach; enough Riflemen go in at once.
    if params.mass > 0.0 && unit.kind == EntityKind::Rifleman && (params.mass_home || !defending) {
        if let Some(mg) = contact
            .iter()
            .filter(|enemy| enemy.kind == EntityKind::MachineGunner && !ctx.moving(enemy))
            .min_by(|a, b| d(a).total_cmp(&d(b)))
        {
            let at = (mg.x, mg.y);
            let mg_reach = ctx.enemy_reach(mg, unit.kind);
            let tiles = d(mg) / ts;
            if tiles > mg_reach + 0.3 && tiles <= mg_reach + params.mass_ring {
                let ready = ctx
                    .own_fighters
                    .iter()
                    .filter(|own| {
                        own.1 == EntityKind::Rifleman
                            && dist(own.0, at) <= (mg_reach + params.mass_ring) * ts
                    })
                    .count() as f32;
                let guns = ctx
                    .enemy_fighters
                    .iter()
                    .filter(|enemy| enemy.1 == EntityKind::MachineGunner && dist(enemy.0, at) <= 6.0 * ts)
                    .count()
                    .max(1) as f32;
                if ready < params.mass * guns {
                    if tiles < mg_reach + 1.0 {
                        let point = ctx.away(pos, at, mg_reach + 1.2 - tiles + 0.3, 0.0);
                        return Some((Mode::Gather, Some(Order::Move(point.0, point.1))));
                    }
                    if unit.state != AiEntityState::Move {
                        return Some((Mode::Gather, None));
                    }
                    return Some((Mode::Gather, Some(Order::Hold)));
                }
                if unit.target_id == Some(mg.id) {
                    return Some((Mode::Charge, None));
                }
                return Some((Mode::Charge, Some(Order::Attack(mg.id))));
            }
        }
    }

    // Gather: ahead of the group and not yet under fire, wait for the units still coming.
    if params.gather > 0.0 && !defending {
        let free = contact.iter().all(|enemy| {
            d(enemy) > (ctx.enemy_reach(enemy, unit.kind) + 2.0) * ts
                && d(enemy) > (reach(unit.kind, enemy.kind, own_still) + 1.0) * ts
        });
        if let (true, Some(nearest)) = (free, contact.iter().min_by(|a, b| d(a).total_cmp(&d(b)))) {
            let foe = (nearest.x, nearest.y);
            let here: f32 = ctx
                .own_fighters
                .iter()
                .filter(|own| dist(own.0, pos) <= 4.0 * ts)
                .map(|own| cost(own.1))
                .sum();
            let coming: f32 = ctx
                .own_moving
                .iter()
                .filter(|own| {
                    dist(own.0, pos) <= params.gather * ts
                        && dist(own.0, pos) > 4.0 * ts
                        && dist(own.0, foe) >= d(nearest) + 3.0 * ts
                })
                .map(|own| cost(own.1))
                .sum();
            if coming >= 0.5 * here {
                if unit.state != AiEntityState::Move {
                    return Some((Mode::Gather, None));
                }
                return Some((Mode::Gather, Some(Order::Hold)));
            }
        }
    }

    // Charge: infantry closes on a Machine Gunner rather than trading at its reach.
    let tank_near = contact.iter().any(|enemy| enemy.kind == EntityKind::Tank);
    if params.charge > 0.0 && infantry(unit.kind) && !(params.charge_no_tank && tank_near) {
        if let Some(mg) = contact
            .iter()
            .filter(|enemy| enemy.kind == EntityKind::MachineGunner && d(enemy) <= params.charge * ts)
            .min_by(|a, b| d(a).total_cmp(&d(b)))
        {
            if unit.target_id == Some(mg.id) {
                return Some((Mode::Charge, None));
            }
            return Some((Mode::Charge, Some(Order::Attack(mg.id))));
        }
    }

    // Focus: the target this unit kills fastest per threat removed, sharing out expected damage.
    let tank_focus_off = (unit.kind == EntityKind::Tank && params.tank_focus == 0)
        || (unit.kind == EntityKind::MachineGunner && params.mg_focus == 0);
    if params.focus && !tank_focus_off {
        let slack = if unit.kind == EntityKind::MachineGunner && params.mg_focus == 1 {
            0.0
        } else if unit.kind == EntityKind::Tank {
            params.focus_slack_tank
        } else if vehicle(unit.kind) {
            params.focus_slack_vehicle
        } else {
            params.focus_slack
        };
        let armored_only = unit.kind == EntityKind::Tank && params.tank_focus == 1;
        let score =|enemy: &AiEntitySummary, booked: f32| -> f32 {
            let face = if params.facing_focus && armored(enemy.kind) {
                heading_of(ctx.memory, enemy.id)
                    .map_or(1.0, |heading| facing_mult(heading, (enemy.x, enemy.y), pos))
            } else {
                1.0
            };
            let rate = dps(unit.kind, enemy.kind, ctx.meth).max(0.05) * face;
            let left = (enemy.hp as f32 - booked).max(5.0);
            let threat = dps(enemy.kind, EntityKind::Rifleman, ctx.enemy_meth)
                .max(dps(enemy.kind, EntityKind::Tank, ctx.enemy_meth))
                + 0.01 * cost(enemy.kind);
            threat * rate / left
        };
        let in_reach: Vec<&&AiEntitySummary> = contact
            .iter()
            .filter(|enemy| {
                let mine = reach(unit.kind, enemy.kind, own_still);
                // Infantry goes past its reach only toward soft targets that do not outrange it:
                // walking at a Machine Gunner (6.1 against 5) just takes its fire. Panzerfausts
                // still close on Tanks, Tanks keep their slack.
                let slack = if params.slack_outranged
                    || !infantry(unit.kind)
                    || armored(enemy.kind)
                    || ctx.enemy_reach(enemy, unit.kind) <= mine
                {
                    slack
                } else {
                    0.0
                };
                d(enemy) <= (mine + slack) * ts
                    && dps(unit.kind, enemy.kind, ctx.meth) >= 0.5
                    && (!armored_only || armored(enemy.kind))
            })
            .collect();
        let best = in_reach.iter().max_by(|a, b| {
            score(a, assigned.get(&a.id).copied().unwrap_or(0.0))
                .total_cmp(&score(b, assigned.get(&b.id).copied().unwrap_or(0.0)))
        });
        if let Some(best) = best {
            let current = unit
                .target_id
                .and_then(|id| in_reach.iter().find(|enemy| enemy.id == id));
            let pick = match current {
                Some(current)
                    if score(current, assigned.get(&current.id).copied().unwrap_or(0.0))
                        * params.focus_keep
                        >= score(best, assigned.get(&best.id).copied().unwrap_or(0.0)) =>
                {
                    current
                }
                _ => best,
            };
            *assigned.entry(pick.id).or_default() +=
                dps(unit.kind, pick.kind, ctx.meth) * FOCUS_HORIZON_S;
            if unit.target_id == Some(pick.id) {
                return Some((Mode::Focus, None));
            }
            return Some((Mode::Focus, Some(Order::Attack(pick.id))));
        }
    }
    if wary {
        return Some((Mode::Avoid, None));
    }
    if stand_ready {
        return Some((Mode::Stand, None));
    }
    None
}

// In-game hook: Jeff's AI V1.1 with the `mm` switch.

use rts_sim::game::command::SimCommand as Command;

/// The live parameters: `JEFF_V1_1_MM` (`key=value,...`, or `JEFF_V11_MM` in local testing) over
/// the tuned defaults.
fn live_params() -> Params {
    static PARAMS: std::sync::OnceLock<Params> = std::sync::OnceLock::new();
    *PARAMS.get_or_init(|| {
        let spec = std::env::var("JEFF_V11_MM")
            .unwrap_or_else(|_| crate::ai_core::profiles::JEFF_V1_1_MM.to_string());
        let mut params = Params::parse(&spec).unwrap_or_default();
        // `standfire` as a V1.1 switch too.
        params.stand_fire |= !crate::ai_core::profiles::jeff_v1_1_feature_off("standfire");
        params
    })
}

/// The units a command orders, for the commands that order units.
fn units_of(command: &Command) -> &[u32] {
    match command {
        Command::Move { units, .. }
        | Command::FormationMove { units, .. }
        | Command::AttackMove { units, .. }
        | Command::ClearObstacleArea { units, .. }
        | Command::Attack { units, .. }
        | Command::AttackTankTrapCluster { units, .. }
        | Command::UseAbility { units, .. }
        | Command::RecastAbility { units, .. }
        | Command::PointTanks { units, .. }
        | Command::Stop { units }
        | Command::HoldPosition { units, .. } => units,
        _ => &[],
    }
}

/// Takes `held` out of a movement or attack order; abilities (smoke) and economy orders stay.
/// Returns false when the order no longer orders anyone.
fn strip(command: &mut Command, held: &BTreeSet<u32>) -> bool {
    match command {
        Command::FormationMove { units, points, .. } => {
            let pairs: Vec<(u32, (f32, f32))> = units
                .iter()
                .copied()
                .zip(points.iter().copied())
                .filter(|(unit, _)| !held.contains(unit))
                .collect();
            if pairs.len() != units.len() {
                *units = pairs.iter().map(|(unit, _)| *unit).collect();
                *points = pairs.iter().map(|(_, point)| *point).collect();
            }
            !units.is_empty()
        }
        Command::Move { units, .. }
        | Command::AttackMove { units, .. }
        | Command::ClearObstacleArea { units, .. }
        | Command::Attack { units, .. }
        | Command::AttackTankTrapCluster { units, .. }
        | Command::PointTanks { units, .. }
        | Command::Stop { units }
        | Command::HoldPosition { units, .. } => {
            units.retain(|unit| !held.contains(unit));
            !units.is_empty()
        }
        _ => true,
    }
}

/// Remembers where this decision's orders send each unit.
fn note_goals(commands: &[Command], memory: &mut MatchupMemory) {
    for command in commands {
        match command {
            Command::Move { units, x, y, .. } | Command::AttackMove { units, x, y, .. } => {
                let attack = matches!(command, Command::AttackMove { .. });
                for unit in units {
                    memory.goals.insert(*unit, Goal { attack, x: *x, y: *y });
                }
            }
            Command::FormationMove {
                units,
                points,
                attack_move,
                ..
            } => {
                for (unit, point) in units.iter().zip(points) {
                    memory.goals.insert(
                        *unit,
                        Goal {
                            attack: *attack_move,
                            x: point.0,
                            y: point.1,
                        },
                    );
                }
            }
            _ => {}
        }
    }
}

/// Marks the units this decision sends at a building, worker or obstacle (raids, trap clearing),
/// and forgets marks that ran out (a mark lasts `eco` ticks, or while the unit still shoots it).
fn note_directed(
    commands: &[Command],
    observation: &AiObservation,
    params: &Params,
    memory: &mut MatchupMemory,
) {
    let tick = observation.tick;
    if params.eco == 0 {
        memory.directed.clear();
        return;
    }
    let soft = |target: u32| {
        observation
            .visible_enemies
            .iter()
            .find(|enemy| enemy.id == target)
            .is_none_or(|enemy| !fights(enemy.kind))
    };
    for command in commands {
        match command {
            Command::Attack { units, target, .. } if soft(*target) => {
                for unit in units {
                    memory.directed.insert(*unit, (tick + params.eco, Some(*target)));
                }
            }
            Command::ClearObstacleArea { units, .. }
            | Command::AttackTankTrapCluster { units, .. } => {
                for unit in units {
                    memory.directed.insert(*unit, (tick + params.eco, None));
                }
            }
            _ => {}
        }
    }
    let shooting: BTreeMap<u32, Option<u32>> = observation
        .owned
        .iter()
        .map(|unit| (unit.id, unit.target_id))
        .collect();
    memory.directed.retain(|unit, (until, target)| {
        shooting.get(unit).is_some_and(|now| {
            tick < *until || (target.is_some() && *now == *target)
        })
    });
}

fn to_command(unit: u32, order: Order) -> Command {
    match order {
        Order::Move(x, y) => Command::Move {
            units: vec![unit],
            x,
            y,
            queued: false,
        },
        Order::Attack(target) => Command::Attack {
            units: vec![unit],
            target,
            queued: false,
        },
        Order::Point(x, y) => Command::PointTanks {
            units: vec![unit],
            x,
            y,
            queued: false,
        },
        Order::Hold => Command::HoldPosition {
            units: vec![unit],
            queued: false,
        },
    }
}

/// Runs the matchup micro over V1.1's finished decision: the units it steers lose every other
/// movement order, it adds its own, and units it lets go get their last other order back.
pub(in crate::ai_core::decision) fn finish(
    decision: &mut crate::ai_core::decision::AiDecision,
    observation: &AiObservation,
    memory: &mut MatchupMemory,
    reserved: &BTreeSet<u32>,
) {
    if !crate::ai_core::profiles::is_jeff_v1_1(decision.profile_id)
        || crate::ai_core::profiles::jeff_v1_1_feature_off("mm")
    {
        return;
    }
    let params = live_params();
    let ts = observation.map.tile_size.max(1) as f32;
    let home = (
        (observation.own_start_tile.0 as f32 + 0.5) * ts,
        (observation.own_start_tile.1 as f32 + 0.5) * ts,
    );
    note_goals(&decision.commands, memory);
    note_directed(&decision.commands, observation, &params, memory);
    let directed: BTreeSet<u32> = memory.directed.keys().copied().collect();
    let kinds: BTreeMap<u32, EntityKind> = observation
        .owned
        .iter()
        .map(|unit| (unit.id, unit.kind))
        .collect();
    memory.goals.retain(|id, _| kinds.contains_key(id));
    // Units on another system's errand that an enemy is about to fight: the micro takes them.
    let broken: BTreeSet<u32> = if params.tank_break > 0.0 || params.unit_break > 0.0 {
        observation
            .owned
            .iter()
            .filter(|unit| unit.hp > 0 && (reserved.contains(&unit.id) || directed.contains(&unit.id)))
            .filter(|unit| {
                let (range, tanks_only) = if unit.kind == EntityKind::Tank {
                    (params.tank_break, true)
                } else {
                    (params.unit_break, false)
                };
                range > 0.0
                    && observation.visible_enemies.iter().any(|enemy| {
                        enemy.hp > 0
                            && if tanks_only { enemy.kind == EntityKind::Tank } else { fights(enemy.kind) }
                            && dist((unit.x, unit.y), (enemy.x, enemy.y)) <= range * ts
                    })
            })
            .map(|unit| unit.id)
            .collect()
    } else {
        BTreeSet::new()
    };
    let eligible = |id: u32| {
        kinds.get(&id).is_some_and(|kind| params.steers_kind(*kind))
            && (broken.contains(&id) || (!reserved.contains(&id) && !directed.contains(&id)))
    };
    let plan = plan(observation, &params, memory, &eligible, home);
    if !plan.held.is_empty() {
        decision
            .commands
            .retain_mut(|command| strip(command, &plan.held));
    }
    for (unit, order) in &plan.orders {
        decision.commands.push(to_command(*unit, *order));
    }
    let ordered: BTreeSet<u32> = decision
        .commands
        .iter()
        .flat_map(|command| units_of(command).iter().copied())
        .collect();
    for unit in &plan.released {
        if ordered.contains(unit) {
            continue;
        }
        if let Some(goal) = memory.goals.get(unit) {
            decision.commands.push(if goal.attack {
                Command::AttackMove {
                    units: vec![*unit],
                    x: goal.x,
                    y: goal.y,
                    queued: false,
                }
            } else {
                Command::Move {
                    units: vec![*unit],
                    x: goal.x,
                    y: goal.y,
                    queued: false,
                }
            });
        }
    }
    decision.trace.matchup = Some(memory.trace());
}
