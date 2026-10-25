//! The live Jeff's spend plan: an income forecast and a schedule of its costly opening steps.
//!
//! Income is close to deterministic. A finished Steel mine or Pump Jack pays 2 every 40 ticks; a
//! finished Depot adds its free Steel mines one at a time (24 s each) and its free Pump Jacks one at
//! a time (36 s each) until every patch and well around it has one; and a patch or well runs dry
//! after its amount. Units and research are charged when ordered, buildings when the foundation goes
//! down. From the extractors standing now, those being built and the Depots on the way, the forecast
//! gives the Steel and Oil mined by any tick of the next three minutes.
//!
//! The schedule lays the opening's costly steps out in order (the natural's Depot, the first
//! Factory, Tank Unlock and the first Tank), each charged at the first moment it is both ready and
//! paid for once the steps ahead of it are. Steel spent now that moves none of them is slack: the
//! Riflemen topping the home up and the surplus Riflemen are bought only from it. The natural's
//! builder goes as soon as the Depot will be paid for by the time it can arrive, rather than once
//! its cost is banked.

use std::collections::BTreeMap;

use super::geometry::{building_center, dist2, squared};
use super::*;

/// The forecast's resolution and reach.
const STEP_TICKS: u32 = 15;
const HORIZON_STEPS: usize = (config::TICK_HZ * 180 / STEP_TICKS) as usize;
/// Steel or Oil per tick from one finished extractor (2 every 40 ticks).
const PER_TICK: f32 = config::STEEL_LOAD as f32 / config::HARVEST_TICKS as f32;
/// An extractor already paying is, on average, half a harvest from its next 2.
const RUNNING_PHASE_TICKS: u32 = config::HARVEST_TICKS / 2;
/// The Depot's free Pump Jack.
const FREE_PUMP_TICKS: u32 = config::AUTOMATIC_PUMP_JACK_BUILD_TICKS;
/// A Pump Jack or Steel mine this close to a patch or well stands on it.
const ON_NODE_TILES: f32 = 0.75;
/// The natural's builder is sent when the forecast, cut by this much, pays for the Depot by its
/// arrival: it then reaches the site no sooner than the money, and the builder never stands waiting
/// (the controller drops a build whose builder has not moved for 10 s).
const DISPATCH_INCOME_SHARE: f32 = 0.9;
/// The home worker's walk to a Factory site behind the HQ.
const FACTORY_WALK_TICKS: u32 = config::TICK_HZ * 3;
/// The builder's walk to a natural not picked yet, for the plan before the natural is due.
const ASSUMED_NATURAL_WALK_TICKS: u32 = config::TICK_HZ * 20;
/// The most Riflemen the slack is sized for in one decision.
const MAX_SLACK_RIFLEMEN: u32 = 8;

/// What the plan remembers between decisions: when each unfinished building and extractor was
/// first seen, and when Tank Unlock was ordered.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct SpendPlanMemory {
    started: BTreeMap<u32, u32>,
    unlock_ordered: Option<u32>,
}

impl SpendPlanMemory {
    pub(super) fn observe(&mut self, observation: &AiObservation, unlock_pending: bool) {
        let tick = observation.tick;
        let mut unfinished = BTreeSet::new();
        for entity in &observation.owned {
            if entity.kind.is_building() && !entity.is_complete && entity.hp > 0 {
                unfinished.insert(entity.id);
                self.started.entry(entity.id).or_insert(tick);
            }
        }
        self.started.retain(|id, _| unfinished.contains(id));
        if observation.upgrades.contains(&UpgradeKind::TankUnlock) {
            self.unlock_ordered = None;
        } else if unlock_pending {
            self.unlock_ordered.get_or_insert(tick);
        } else {
            self.unlock_ordered = None;
        }
    }

    /// When the unfinished building `id` will be finished, given how long it takes to build.
    fn finished_at(&self, id: u32, now: u32, build_ticks: u32) -> u32 {
        self.started
            .get(&id)
            .copied()
            .unwrap_or(now)
            .saturating_add(build_ticks)
            .max(now)
    }
}

/// Steel and Oil mined from now to each step of the horizon.
#[derive(Clone, Debug)]
pub(super) struct Forecast {
    now: u32,
    steel: Vec<f32>,
    oil: Vec<f32>,
}

impl Forecast {
    fn empty(now: u32) -> Self {
        Self {
            now,
            steel: vec![0.0; HORIZON_STEPS + 1],
            oil: vec![0.0; HORIZON_STEPS + 1],
        }
    }

    /// Steel and Oil mined from now until `ticks` from now (within the horizon).
    pub(super) fn mined_within(&self, ticks: u32) -> (f32, f32) {
        let step = ((ticks / STEP_TICKS) as usize).min(HORIZON_STEPS);
        (self.steel[step], self.oil[step])
    }

    fn add(&mut self, extractor: &Extractor) {
        let series = if extractor.oil {
            &mut self.oil
        } else {
            &mut self.steel
        };
        for (step, mined) in series.iter_mut().enumerate() {
            let tick = self.now + step as u32 * STEP_TICKS;
            if tick > extractor.pays_from {
                *mined += (PER_TICK * (tick - extractor.pays_from) as f32).min(extractor.amount);
            }
        }
    }
}

/// One extractor's income: from `pays_from` at the full rate until `amount` is mined.
#[derive(Clone, Copy, Debug)]
struct Extractor {
    oil: bool,
    pays_from: u32,
    amount: f32,
}

/// A base whose Depot is, or will be, finished at `ready`.
#[derive(Clone, Copy, Debug)]
struct Base {
    center: (f32, f32),
    ready: u32,
}

fn build_ticks(kind: EntityKind) -> u32 {
    config::building_stats(kind).map_or(0, |stats| stats.build_ticks)
}

fn worker_speed() -> f32 {
    config::unit_stats(EntityKind::Worker).map_or(2.0, |stats| stats.speed)
}

/// Ticks a worker at `from` needs at least to reach `to`: the straight line, so never more than its
/// real walk.
fn walk_ticks(from: (f32, f32), to: (f32, f32)) -> u32 {
    (dist2(from.0, from.1, to.0, to.1).sqrt() / worker_speed()).ceil() as u32
}

fn depot_center(observation: &AiObservation, site: (u32, u32)) -> Option<(f32, f32)> {
    building_center(site, EntityKind::ResourceDepot, observation.map.tile_size)
}

/// Jeff's bases: finished Depots, foundations and Depot orders on the way, as the forecast sees
/// them.
fn bases(observation: &AiObservation, memory: &SpendPlanMemory) -> Vec<Base> {
    let now = observation.tick;
    let depot_ticks = build_ticks(EntityKind::ResourceDepot);
    let mut bases: Vec<Base> = observation
        .owned
        .iter()
        .filter(|entity| entity.kind == EntityKind::ResourceDepot && entity.hp > 0)
        .map(|depot| Base {
            center: (depot.x, depot.y),
            ready: if depot.is_complete {
                now
            } else {
                memory.finished_at(depot.id, now, depot_ticks)
            },
        })
        .collect();
    for intent in observation
        .pending_builds
        .iter()
        .filter(|intent| intent.kind == EntityKind::ResourceDepot)
    {
        let (Some(center), Some(worker)) = (
            depot_center(observation, (intent.tile_x, intent.tile_y)),
            observation
                .owned
                .iter()
                .find(|unit| unit.id == intent.worker_id),
        ) else {
            continue;
        };
        bases.push(Base {
            center,
            ready: now + walk_ticks((worker.x, worker.y), center) + depot_ticks,
        });
    }
    bases
}

/// Every extractor's income at `bases`: those standing, those being built and the free ones each
/// Depot will add, one at a time, until its patches and wells are taken.
fn extractors(
    observation: &AiObservation,
    memory: &SpendPlanMemory,
    bases: &[Base],
) -> Vec<Extractor> {
    let now = observation.tick;
    let ts = observation.map.tile_size as f32;
    let range2 = squared(config::MINING_ANCHOR_RANGE_TILES * ts);
    let on_node2 = squared(ON_NODE_TILES * ts);
    let mut out = Vec::new();
    for (oil, node_kind, extractor_kind, free_ticks) in [
        (
            false,
            EntityKind::Steel,
            EntityKind::SteelMine,
            build_ticks(EntityKind::SteelMine),
        ),
        (true, EntityKind::Oil, EntityKind::PumpJack, FREE_PUMP_TICKS),
    ] {
        let full = if oil {
            config::OIL_GEYSER_AMOUNT
        } else {
            config::STEEL_PATCH_AMOUNT
        } as f32;
        // Each extractor stands on one patch or well, the nearest (it is built on the node's
        // centre); Jeff's own, or anyone else's. Matching by distance alone counted one mine twice
        // where patches are packed tight (Fastest Map Possible has 36 around a Depot).
        let mut on_node: BTreeMap<u32, (&AiEntitySummary, bool)> = BTreeMap::new();
        for (entity, own) in observation
            .owned
            .iter()
            .map(|entity| (entity, true))
            .chain(
                observation
                    .visible_enemies
                    .iter()
                    .chain(&observation.visible_allies)
                    .map(|entity| (entity, false)),
            )
            .filter(|(entity, _)| entity.kind == extractor_kind && entity.hp > 0)
        {
            let nearest = observation
                .resources
                .iter()
                .filter(|node| node.kind == node_kind)
                .min_by(|a, b| {
                    dist2(a.x, a.y, entity.x, entity.y)
                        .total_cmp(&dist2(b.x, b.y, entity.x, entity.y))
                        .then(a.id.cmp(&b.id))
                })
                .filter(|node| dist2(node.x, node.y, entity.x, entity.y) <= on_node2);
            if let Some(node) = nearest {
                on_node.entry(node.id).or_insert((entity, own));
            }
        }
        for (index, base) in bases.iter().enumerate() {
            // Each patch or well belongs to its nearest base in reach.
            let mut nodes: Vec<&AiResourceSummary> = observation
                .resources
                .iter()
                .filter(|node| node.kind == node_kind && node.remaining > 0)
                .filter(|node| dist2(node.x, node.y, base.center.0, base.center.1) <= range2)
                .filter(|node| {
                    bases
                        .iter()
                        .enumerate()
                        .filter(|(other, _)| *other != index)
                        .all(|(_, other)| {
                            dist2(node.x, node.y, other.center.0, other.center.1)
                                > dist2(node.x, node.y, base.center.0, base.center.1)
                        })
                })
                .collect();
            nodes.sort_by(|a, b| {
                dist2(a.x, a.y, base.center.0, base.center.1)
                    .total_cmp(&dist2(b.x, b.y, base.center.0, base.center.1))
                    .then(a.id.cmp(&b.id))
            });
            // A node out of sight reports a placeholder amount; a base's own nodes are in sight,
            // and a planned base's are taken as untouched.
            let amount = |node: &AiResourceSummary| {
                if node.remaining <= 1 {
                    full
                } else {
                    node.remaining as f32
                }
            };
            let mut next_free = base.ready.max(now);
            let mut free_nodes = Vec::new();
            for node in nodes {
                match on_node.get(&node.id) {
                    Some((extractor, true)) if extractor.is_complete => out.push(Extractor {
                        oil,
                        pays_from: now + RUNNING_PHASE_TICKS,
                        amount: amount(node),
                    }),
                    Some((extractor, true)) => {
                        let done = memory.finished_at(extractor.id, now, free_ticks);
                        next_free = next_free.max(done);
                        out.push(Extractor {
                            oil,
                            pays_from: done + config::HARVEST_TICKS,
                            amount: amount(node),
                        });
                    }
                    Some((_, false)) => {}
                    None => free_nodes.push(amount(node)),
                }
            }
            for amount in free_nodes {
                let done = next_free + free_ticks;
                out.push(Extractor {
                    oil,
                    pays_from: done + config::HARVEST_TICKS,
                    amount,
                });
                next_free = done;
            }
        }
    }
    out
}

fn forecast_for(
    observation: &AiObservation,
    memory: &SpendPlanMemory,
    bases: &[Base],
    share: f32,
) -> Forecast {
    let mut forecast = Forecast::empty(observation.tick);
    for extractor in extractors(observation, memory, bases) {
        forecast.add(&extractor);
    }
    if share < 1.0 {
        for mined in forecast.steel.iter_mut().chain(forecast.oil.iter_mut()) {
            *mined *= share;
        }
    }
    forecast
}

/// One costly step of the opening.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StepKind {
    /// An opening gun still to order before the natural is due.
    Gun,
    EngineeringComplex,
    Natural,
    Factory,
    TankUnlock,
    Tank,
}

impl StepKind {
    fn cost(self) -> (u32, u32) {
        match self {
            StepKind::Gun => rts_rules::economy::cost(EntityKind::MachineGunner),
            StepKind::EngineeringComplex => {
                rts_rules::economy::cost(EntityKind::EngineeringComplex)
            }
            StepKind::Natural => rts_rules::economy::cost(EntityKind::ResourceDepot),
            StepKind::Factory => rts_rules::economy::cost(EntityKind::Factory),
            StepKind::TankUnlock => {
                let definition = upgrade::definition(UpgradeKind::TankUnlock);
                (definition.cost_steel, definition.cost_oil)
            }
            StepKind::Tank => rts_rules::economy::cost(EntityKind::Tank),
        }
    }

    fn label(self) -> &'static str {
        match self {
            StepKind::Gun => "gun",
            StepKind::EngineeringComplex => "ec",
            StepKind::Natural => "natural",
            StepKind::Factory => "factory",
            StepKind::TankUnlock => "unlock",
            StepKind::Tank => "tank",
        }
    }
}

/// The state the steps start from.
#[derive(Clone, Copy, Debug)]
pub(super) struct PlanInput {
    /// The natural is due (and has no Depot yet).
    pub(super) natural_due: bool,
    /// The natural's site, and the worker that would build it or is on the way.
    pub(super) natural_site: Option<(u32, u32)>,
    pub(super) natural_builder: Option<u32>,
    /// Ticks the natural's order still waits for its guards to secure the site.
    pub(super) natural_wait: u32,
}

/// The opening's steps with the tick each is charged, laid out from now.
#[derive(Clone, Debug)]
pub(super) struct SpendPlan {
    pub(super) steps: Vec<(StepKind, Option<u32>)>,
    /// The natural's Depot is paid for by the time its builder can arrive, on a forecast cut to
    /// `DISPATCH_INCOME_SHARE`.
    pub(super) natural_funded_on_arrival: bool,
    /// The most Steel that can be spent now without moving any step.
    pub(super) slack_steel: u32,
    /// Whether the slack binds: the opening's chain is under way (the natural due or ordered, or a
    /// Factory up) and no Tank is out yet.
    pub(super) gates: bool,
    /// Steel and Oil the forecast expects from now to 15, 30 and 60 s out, the natural as planned
    /// included.
    pub(super) expected: [(f32, f32); 3],
}

impl SpendPlan {
    /// Whether `steel` more fits the slack after what this decision already spent off the plan.
    pub(super) fn affords_off_plan(&self, intents: &[AiIntent], steel: u32) -> bool {
        !self.gates || off_plan_spent(intents).saturating_add(steel) <= self.slack_steel
    }

    /// The Rifleman count (in training included) the slack pays for, while it binds.
    pub(super) fn rifle_cap(
        &self,
        intents: &[AiIntent],
        counts: &[(EntityKind, usize)],
    ) -> Option<usize> {
        if !self.gates {
            return None;
        }
        let rifle = rts_rules::economy::cost(EntityKind::Rifleman).0.max(1);
        let spare = self.slack_steel.saturating_sub(off_plan_spent(intents)) / rifle;
        let current = counts
            .iter()
            .find_map(|(kind, count)| (*kind == EntityKind::Rifleman).then_some(*count))
            .unwrap_or(0);
        Some(current + spare as usize)
    }

    pub(super) fn charged_at(&self, kind: StepKind) -> Option<u32> {
        self.steps
            .iter()
            .find(|(step, _)| *step == kind)
            .and_then(|(_, tick)| *tick)
    }

    /// The trace line: the income expected and each step's planned tick.
    pub(super) fn trace_line(&self, observation: &AiObservation) -> String {
        let expected = self
            .expected
            .iter()
            .zip(["15s", "30s", "60s"])
            .map(|((steel, oil), label)| format!("{label}:{:.0}/{:.0}", steel, oil))
            .collect::<Vec<_>>()
            .join(" ");
        let steps = self
            .steps
            .iter()
            .map(|(kind, tick)| {
                format!(
                    "{}:{}",
                    kind.label(),
                    tick.map_or("-".to_string(), |tick| tick.to_string())
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "plan bank={}/{} mined {expected} steps {steps} slack_steel={} dispatch={}",
            observation.economy.steel,
            observation.economy.oil,
            self.slack_steel,
            self.natural_funded_on_arrival
        )
    }
}

struct Step {
    kind: StepKind,
    /// The step can be charged no sooner than this.
    ready: Ready,
    /// The natural ordered only once its cost is banked: its cost is set aside then and charged a
    /// walk later.
    banked: bool,
}

#[derive(Clone, Copy)]
enum Ready {
    At(u32),
    /// Ticks after the previous gun is ordered: the Barracks trains one at a time.
    AfterGun(u32),
    /// Ticks after the opening guns and the Engineering Complex are ordered: the natural is due
    /// then, and its builder walks this long.
    AfterOpening(u32),
    /// Ticks after the natural's order goes out (its charge less the builder's walk).
    AfterNaturalOrder(u32),
    /// Ticks after the Factory is charged.
    AfterFactory(u32),
    /// When both the Factory and Tank Unlock are done: `(Factory build, Unlock research)` after
    /// each is charged, or the tick each is done if already under way.
    FactoryAndUnlock {
        factory_done: Option<u32>,
        unlock_done: Option<u32>,
    },
}

/// The opening's steps still ahead, in order.
fn steps(
    observation: &AiObservation,
    facts: &AiFacts,
    memory: &SpendPlanMemory,
    input: PlanInput,
    natural_walk: Option<u32>,
) -> Vec<Step> {
    let now = observation.tick;
    let mut steps = Vec::new();
    let natural_pending = input.natural_site.is_some_and(|site| {
        observation.pending_builds.iter().any(|intent| {
            intent.kind == EntityKind::ResourceDepot && (intent.tile_x, intent.tile_y) == site
        })
    });
    let planned_natural = (input.natural_due || natural_pending) && natural_walk.is_some();
    let factory_planned = facts.building_count(EntityKind::Factory) > 0;
    // Before the natural is due: the opening guns and Engineering Complex that make it due, then the
    // natural.
    let natural_ahead =
        !planned_natural && !factory_planned && facts.building_count(EntityKind::ResourceDepot) < 2;
    if planned_natural {
        let wait = if natural_pending {
            0
        } else {
            input.natural_wait
        };
        steps.push(Step {
            kind: StepKind::Natural,
            banked: !natural_pending && factory_planned,
            ready: Ready::At(now + wait + natural_walk.unwrap_or(0)),
        });
    } else if natural_ahead {
        if facts.building_count(EntityKind::EngineeringComplex) == 0 {
            steps.push(Step {
                kind: StepKind::EngineeringComplex,
                banked: false,
                ready: Ready::At(now + FACTORY_WALK_TICKS),
            });
        }
        let queued: usize = observation
            .owned
            .iter()
            .filter(|unit| {
                unit.kind == EntityKind::Barracks
                    && unit.production_kind == Some(EntityKind::MachineGunner)
            })
            .map(|unit| unit.production_queue_len.unwrap_or(0))
            .sum();
        let guns = (facts.unit_count(EntityKind::MachineGunner) + queued)
            .min(expansion_security::OPENING_GUNS);
        let train =
            config::unit_stats(EntityKind::MachineGunner).map_or(400, |stats| stats.build_ticks);
        for index in guns..expansion_security::OPENING_GUNS {
            steps.push(Step {
                kind: StepKind::Gun,
                banked: false,
                ready: if index == guns {
                    // A gun in training holds the Barracks about half its time more.
                    Ready::At(now + if queued > 0 { train / 2 } else { 0 })
                } else {
                    Ready::AfterGun(train)
                },
            });
        }
        steps.push(Step {
            kind: StepKind::Natural,
            banked: false,
            ready: Ready::AfterOpening(natural_walk.unwrap_or(ASSUMED_NATURAL_WALK_TICKS)),
        });
    }
    let factory = observation
        .owned
        .iter()
        .find(|entity| entity.kind == EntityKind::Factory && entity.hp > 0);
    let factory_ticks = build_ticks(EntityKind::Factory);
    let second_engineer = if facts.worker_count < 2 {
        config::unit_stats(EntityKind::Worker).map_or(150, |stats| stats.build_ticks)
    } else {
        0
    };
    let factory_done = match factory {
        Some(factory) if factory.is_complete => Some(now),
        Some(factory) => Some(memory.finished_at(factory.id, now, factory_ticks)),
        None => None,
    };
    if factory.is_none() {
        let pending = observation
            .pending_builds
            .iter()
            .find(|intent| intent.kind == EntityKind::Factory);
        let ready = match pending {
            Some(intent) => {
                let arrival = observation
                    .owned
                    .iter()
                    .find(|unit| unit.id == intent.worker_id)
                    .zip(building_center(
                        (intent.tile_x, intent.tile_y),
                        EntityKind::Factory,
                        observation.map.tile_size,
                    ))
                    .map_or(FACTORY_WALK_TICKS, |(worker, center)| {
                        walk_ticks((worker.x, worker.y), center)
                    });
                Ready::At(now + arrival)
            }
            // The live Jeff's tech waits for the natural's order while the natural is due, and with
            // one Engineer, which builds the natural, for the second to be trained.
            None if (planned_natural || natural_ahead) && !natural_pending => {
                Ready::AfterNaturalOrder(FACTORY_WALK_TICKS + second_engineer)
            }
            None if natural_pending => Ready::At(now + FACTORY_WALK_TICKS + second_engineer),
            None => Ready::At(now + FACTORY_WALK_TICKS),
        };
        steps.push(Step {
            kind: StepKind::Factory,
            ready,
            banked: false,
        });
    }
    let unlock_research = config::TICK_HZ * 20;
    let unlocked = observation.upgrades.contains(&UpgradeKind::TankUnlock);
    let unlock_done = if unlocked {
        Some(now)
    } else {
        memory
            .unlock_ordered
            .map(|ordered| (ordered + unlock_research).max(now))
    };
    if !unlocked && memory.unlock_ordered.is_none() {
        steps.push(Step {
            kind: StepKind::TankUnlock,
            banked: false,
            ready: if factory.is_some() {
                Ready::At(now)
            } else {
                Ready::AfterFactory(0)
            },
        });
    }
    let tank_out = facts.unit_count(EntityKind::Tank) > 0
        || observation.owned.iter().any(|entity| {
            entity.kind == EntityKind::Factory
                && entity.production_kind == Some(EntityKind::Tank)
                && entity.production_queue_len.unwrap_or(0) > 0
        });
    if !tank_out {
        steps.push(Step {
            kind: StepKind::Tank,
            banked: false,
            ready: Ready::FactoryAndUnlock {
                factory_done,
                unlock_done,
            },
        });
    }
    steps
}

/// Steel this decision has spent on orders that are not steps.
fn off_plan_spent(intents: &[AiIntent]) -> u32 {
    intents
        .iter()
        .map(|intent| match intent {
            AiIntent::Train { kind } if *kind != EntityKind::Tank => {
                rts_rules::economy::cost(*kind).0
            }
            AiIntent::Build { kind }
                if !matches!(kind, EntityKind::ResourceDepot | EntityKind::Factory) =>
            {
                rts_rules::economy::cost(*kind).0
            }
            AiIntent::Research { upgrade } if *upgrade != UpgradeKind::TankUnlock => {
                upgrade::definition(*upgrade).cost_steel
            }
            _ => 0,
        })
        .sum()
}

/// Orders out but not yet charged that are not steps (a Barracks on the way, say): the bank pays
/// them first.
fn off_plan_orders(observation: &AiObservation, input: PlanInput, steps: &[Step]) -> (f32, f32) {
    let factory_step = steps.iter().any(|step| step.kind == StepKind::Factory);
    observation
        .pending_builds
        .iter()
        .filter(|intent| {
            let natural = intent.kind == EntityKind::ResourceDepot
                && Some((intent.tile_x, intent.tile_y)) == input.natural_site;
            !natural && !(factory_step && intent.kind == EntityKind::Factory)
        })
        .map(|intent| rts_rules::economy::cost(intent.kind))
        .fold((0.0, 0.0), |(s, o), (cs, co)| {
            (s + cs as f32, o + co as f32)
        })
}

/// Lays the steps out from now with `extra` Steel and Oil spent now. Returns each step's charge
/// tick (None past the horizon, and for every step after one past it).
fn lay_out(
    observation: &AiObservation,
    memory: &SpendPlanMemory,
    input: PlanInput,
    steps: &[Step],
    base_forecast: &Forecast,
    natural_walk: Option<u32>,
    extra: (u32, u32),
) -> (Vec<(StepKind, Option<u32>)>, Forecast) {
    let now = observation.tick;
    let factory_ticks = build_ticks(EntityKind::Factory);
    let unlock_research = config::TICK_HZ * 20;
    let mut forecast = base_forecast.clone();
    let mut spent = (
        extra.0 as f32 + off_plan_orders(observation, input, steps).0,
        extra.1 as f32 + off_plan_orders(observation, input, steps).1,
    );
    let bank = (
        observation.economy.steel as f32,
        observation.economy.oil as f32,
    );
    let mut out = Vec::new();
    let mut natural_charge = None;
    let mut gun_charge: Option<u32> = None;
    let mut ec_charge: Option<u32> = None;
    let mut factory_charge = None;
    let mut unlock_charge = None;
    let mut blocked = false;
    for step in steps {
        if blocked {
            out.push((step.kind, None));
            continue;
        }
        let ready = match step.ready {
            Ready::At(tick) => Some(tick),
            Ready::AfterGun(train) => gun_charge.map(|charge| charge + train),
            Ready::AfterOpening(walk) => {
                Some(gun_charge.unwrap_or(now).max(ec_charge.unwrap_or(now)) + walk)
            }
            Ready::AfterNaturalOrder(walk) => natural_charge
                .map(|charge: u32| charge.saturating_sub(natural_walk.unwrap_or(0)).max(now) + walk)
                .or(Some(now + walk)),
            Ready::AfterFactory(delay) => factory_charge.map(|charge: u32| charge + delay),
            Ready::FactoryAndUnlock {
                factory_done,
                unlock_done,
            } => {
                let factory = factory_done.or(factory_charge.map(|charge| charge + factory_ticks));
                let unlock = unlock_done.or(unlock_charge.map(|charge| charge + unlock_research));
                factory
                    .zip(unlock)
                    .map(|(factory, unlock)| factory.max(unlock))
            }
        };
        let Some(ready) = ready else {
            blocked = true;
            out.push((step.kind, None));
            continue;
        };
        let (cost_steel, cost_oil) = step.kind.cost();
        let earliest = if step.banked {
            ready.saturating_sub(natural_walk.unwrap_or(0))
        } else {
            ready
        };
        let first = (earliest.saturating_sub(now)).div_ceil(STEP_TICKS) as usize;
        let charge = (first..=HORIZON_STEPS).find(|&index| {
            bank.0 + forecast.steel[index] - spent.0 >= cost_steel as f32
                && bank.1 + forecast.oil[index] - spent.1 >= cost_oil as f32
        });
        let Some(index) = charge else {
            blocked = true;
            out.push((step.kind, None));
            continue;
        };
        let tick = if step.banked {
            (now + index as u32 * STEP_TICKS + natural_walk.unwrap_or(0)).max(ready)
        } else {
            (now + index as u32 * STEP_TICKS).max(ready)
        };
        spent = (spent.0 + cost_steel as f32, spent.1 + cost_oil as f32);
        out.push((step.kind, Some(tick)));
        match step.kind {
            StepKind::Gun => gun_charge = Some(tick),
            StepKind::EngineeringComplex => ec_charge = Some(tick),
            StepKind::Natural => {
                natural_charge = Some(tick);
                // The natural's own mines and Pump Jacks pay from its foundation on.
                if let Some(center) = input
                    .natural_site
                    .and_then(|site| depot_center(observation, site))
                {
                    let natural = [Base {
                        center,
                        ready: tick + build_ticks(EntityKind::ResourceDepot),
                    }];
                    let known = bases(observation, memory);
                    if !known.iter().any(|base| {
                        dist2(base.center.0, base.center.1, center.0, center.1)
                            < squared(observation.map.tile_size as f32)
                    }) {
                        let added = forecast_for(observation, memory, &natural, 1.0);
                        for (mined, more) in forecast.steel.iter_mut().zip(&added.steel) {
                            *mined += more;
                        }
                        for (mined, more) in forecast.oil.iter_mut().zip(&added.oil) {
                            *mined += more;
                        }
                    }
                }
            }
            StepKind::Factory => factory_charge = Some(tick),
            StepKind::TankUnlock => unlock_charge = Some(tick),
            StepKind::Tank => {}
        }
    }
    (out, forecast)
}

/// The live Jeff's plan for this decision.
pub(super) fn plan(
    observation: &AiObservation,
    facts: &AiFacts,
    memory: &SpendPlanMemory,
    input: PlanInput,
) -> SpendPlan {
    let natural_walk = input.natural_site.and_then(|site| {
        let center = depot_center(observation, site)?;
        let builder = observation
            .owned
            .iter()
            .find(|unit| Some(unit.id) == input.natural_builder)?;
        Some(walk_ticks((builder.x, builder.y), center))
    });
    let known = bases(observation, memory);
    let steps = steps(observation, facts, memory, input, natural_walk);
    let base = forecast_for(observation, memory, &known, 1.0);
    let (laid, forecast) = lay_out(
        observation,
        memory,
        input,
        &steps,
        &base,
        natural_walk,
        (0, 0),
    );
    // Slack: the most Steel spent now that moves no step.
    let moves_a_step = |steel: u32| {
        let (with, _) = lay_out(
            observation,
            memory,
            input,
            &steps,
            &base,
            natural_walk,
            (steel, 0),
        );
        with.iter()
            .zip(&laid)
            .any(|((_, after), (_, before))| match (before, after) {
                (Some(before), Some(after)) => after > before,
                (Some(_), None) => true,
                _ => false,
            })
    };
    let rifle = rts_rules::economy::cost(EntityKind::Rifleman).0.max(1);
    let mut low = 0u32;
    let mut high = MAX_SLACK_RIFLEMEN;
    if !moves_a_step(high * rifle) {
        low = high;
    } else {
        while low < high {
            let mid = (low + high).div_ceil(2);
            if moves_a_step(mid * rifle) {
                high = mid - 1;
            } else {
                low = mid;
            }
        }
    }
    let slack_steel = (low * rifle).min(observation.economy.steel);
    // Dispatch: on a forecast cut to DISPATCH_INCOME_SHARE, the natural is charged by the time its
    // builder can arrive.
    // With a Factory up the natural waits for its cost banked, as before: ordered on credit it took
    // the Steel Tank Unlock used to get during the builder's walk (Chokes: Unlock 2:32 to 2:56).
    let (depot_steel, depot_oil) = StepKind::Natural.cost();
    let natural_funded_on_arrival = input.natural_due
        && if facts.building_count(EntityKind::Factory) > 0 {
            observation.economy.steel >= depot_steel && observation.economy.oil >= depot_oil
        } else {
            natural_walk.is_some_and(|walk| {
                let cautious = forecast_for(observation, memory, &known, DISPATCH_INCOME_SHARE);
                let (steel, oil) = cautious.mined_within(walk);
                let (owed_steel, owed_oil) = off_plan_orders(observation, input, &steps);
                let (cost_steel, cost_oil) = StepKind::Natural.cost();
                observation.economy.steel as f32 + steel - owed_steel >= cost_steel as f32
                    && observation.economy.oil as f32 + oil - owed_oil >= cost_oil as f32
            })
        };
    let expected = [450, 900, 1800].map(|ticks: u32| forecast.mined_within(ticks));
    let natural_pending = input.natural_site.is_some_and(|site| {
        observation.pending_builds.iter().any(|intent| {
            intent.kind == EntityKind::ResourceDepot && (intent.tile_x, intent.tile_y) == site
        })
    });
    let gates = steps.iter().any(|step| step.kind == StepKind::Tank)
        && (input.natural_due || natural_pending || facts.building_count(EntityKind::Factory) > 0);
    SpendPlan {
        steps: laid,
        natural_funded_on_arrival,
        slack_steel,
        gates,
        expected,
    }
}
