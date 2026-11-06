use crate::protocol::{ReplayPlayerResourceSample, ReplayResourceSample};
use rts_sim::game::Game;

/// Replay-owned observer metric timeline, independent of replaceable observer-analysis messages.
#[derive(Default)]
pub(super) struct ReplayResourceHistory {
    pub(super) samples: Vec<ReplayResourceSample>,
}

impl ReplayResourceHistory {
    pub(super) fn truncate(&mut self, tick: u32) {
        self.samples.retain(|sample| sample.tick <= tick);
    }

    pub(super) fn record(&mut self, game: &Game) {
        const INTERVAL: u32 = 30;
        const WINDOW: u32 = 8 * 30;
        let tick = game.tick_count();
        if self.samples.last().is_some_and(|last| tick.saturating_sub(last.tick) < INTERVAL) {
            return;
        }
        let analysis = game.observer_analysis();
        if !(2..=4).contains(&analysis.players.len()) {
            return;
        }
        let target = tick.saturating_sub(WINDOW);
        let baseline = self.samples.iter().rev().find(|sample| {
            tick >= WINDOW && sample.tick <= target && target - sample.tick <= INTERVAL
        });
        let mut players = analysis.players.iter().map(|player| {
            let steel = i64::from(player.resources.lifetime.steel);
            let oil = i64::from(player.resources.lifetime.oil);
            let prior = baseline.and_then(|sample| sample.players.iter().find(|row| row.id == player.id));
            let lost_steel = i64::from(player.resources_lost.steel);
            let lost_oil = i64::from(player.resources_lost.oil);
            ReplayPlayerResourceSample {
                id: player.id,
                lifetime_steel: steel,
                lifetime_oil: oil,
                steel: prior.map_or(0, |row| steel - row.lifetime_steel),
                oil: prior.map_or(0, |row| oil - row.lifetime_oil),
                alive_steel: steel - lost_steel,
                alive_oil: oil - lost_oil,
                lost_steel,
                lost_oil,
                unit_steel: player.units.iter().map(|unit| i64::from(unit.steel_value)).sum(),
                unit_oil: player.units.iter().map(|unit| i64::from(unit.oil_value)).sum(),
            }
        }).collect::<Vec<_>>();
        players.sort_by_key(|player| player.id);
        // Keep the established 1v1 advantage representation alongside individual series.
        let difference = |field: fn(&ReplayPlayerResourceSample) -> i64| {
            players.first().map_or(0, field) - players.get(1).map_or(0, field)
        };
        let sample = ReplayResourceSample {
            tick,
            steel: difference(|row| row.steel),
            oil: difference(|row| row.oil),
            alive_steel: difference(|row| row.alive_steel),
            alive_oil: difference(|row| row.alive_oil),
            lost_steel: difference(|row| row.lost_steel),
            lost_oil: difference(|row| row.lost_oil),
            unit_steel: difference(|row| row.unit_steel),
            unit_oil: difference(|row| row.unit_oil),
            players,
        };
        self.samples.push(sample);
    }
}
