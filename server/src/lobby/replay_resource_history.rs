use crate::protocol::ReplayResourceSample;
use rts_sim::game::Game;

/// Replay-owned collection and alive-resource timeline, independent of replaceable observer-analysis messages.
#[derive(Default)]
pub(super) struct ReplayResourceHistory {
    pub(super) samples: Vec<ReplayResourceSample>,
    totals: Vec<(u32, i64, i64)>,
}

impl ReplayResourceHistory {
    pub(super) fn truncate(&mut self, tick: u32) {
        self.samples.retain(|sample| sample.tick <= tick);
        self.totals
            .retain(|(sample_tick, _, _)| *sample_tick <= tick);
    }

    pub(super) fn record(&mut self, game: &Game) {
        const INTERVAL: u32 = 30;
        const WINDOW: u32 = 8 * 30;
        let tick = game.tick_count();
        if self
            .samples
            .last()
            .is_some_and(|last| tick < last.tick + INTERVAL)
        {
            return;
        }
        let analysis = game.observer_analysis();
        if analysis.players.len() != 2 {
            return;
        }
        let mut players = analysis.players.iter().collect::<Vec<_>>();
        players.sort_by_key(|player| player.id);
        let steel = i64::from(players[0].resources.lifetime.steel)
            - i64::from(players[1].resources.lifetime.steel);
        let oil = i64::from(players[0].resources.lifetime.oil)
            - i64::from(players[1].resources.lifetime.oil);
        let target = tick.saturating_sub(WINDOW);
        let baseline = (tick >= WINDOW)
            .then(|| {
                self.totals
                    .iter()
                    .rev()
                    .find(|(sample_tick, _, _)| *sample_tick <= target)
            })
            .flatten();
        let (window_steel, window_oil) = match baseline {
            Some((sample_tick, prior_steel, prior_oil)) if target - sample_tick <= INTERVAL => {
                (steel - prior_steel, oil - prior_oil)
            }
            _ => (0, 0),
        };
        self.totals.push((tick, steel, oil));
        self.samples.push(ReplayResourceSample {
            tick,
            steel: window_steel,
            oil: window_oil,
            alive_steel: steel - i64::from(players[0].resources_lost.steel)
                + i64::from(players[1].resources_lost.steel),
            alive_oil: oil - i64::from(players[0].resources_lost.oil)
                + i64::from(players[1].resources_lost.oil),
        });
    }
}
