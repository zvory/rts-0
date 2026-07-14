use super::PlayerState;
use crate::config;
use crate::game::entity::EntityKind;
use crate::game::upgrade::UpgradeKind;
use crate::protocol::{ObserverAnalysisResourceTotals, ObserverAnalysisResources};
use crate::rules::economy::ResourceCost;

const RESOURCE_WINDOW_5S_TICKS: u32 = config::TICK_HZ * 5;
const RESOURCE_WINDOW_MINUTE_TICKS: u32 = config::TICK_HZ * 60;

impl PlayerState {
    pub(crate) fn reset_for_dev_scenario(&mut self, start_tile: (u32, u32)) {
        self.start_tile = start_tile;
        self.steel = 0;
        self.oil = 10_000;
        self.supply_used = 0;
        self.supply_cap = 0;
        self.score = Default::default();
        self.upgrades.clear();
        self.ability_cooldowns.clear();
    }

    pub(crate) fn can_afford(&self, steel: u32, oil: u32) -> bool {
        self.steel >= steel && self.oil >= oil
    }

    pub(crate) fn spend_resources(&mut self, steel: u32, oil: u32) -> bool {
        if !self.can_afford(steel, oil) {
            return false;
        }
        self.steel -= steel;
        self.oil -= oil;
        true
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn set_resources(&mut self, steel: u32, oil: u32) {
        self.steel = steel;
        self.oil = oil;
    }

    pub(crate) fn spend_cost(&mut self, cost: ResourceCost) -> bool {
        self.spend_resources(cost.steel, cost.oil)
    }

    pub(crate) fn refund_resources(&mut self, steel: u32, oil: u32) {
        self.steel = self.steel.saturating_add(steel);
        self.oil = self.oil.saturating_add(oil);
    }

    pub(crate) fn refund_cost(&mut self, cost: ResourceCost) {
        self.refund_resources(cost.steel, cost.oil);
    }

    pub(crate) fn add_gathered_resources(&mut self, kind: EntityKind, amount: u32, tick: u32) {
        if amount == 0 {
            return;
        }
        let (steel, oil) = match kind {
            EntityKind::Oil => (0, amount),
            EntityKind::Steel => (amount, 0),
            _ => return,
        };
        match kind {
            EntityKind::Oil => self.oil = self.oil.saturating_add(amount),
            EntityKind::Steel => self.steel = self.steel.saturating_add(amount),
            _ => {}
        }
        self.score.resources_mined.steel = self.score.resources_mined.steel.saturating_add(steel);
        self.score.resources_mined.oil = self.score.resources_mined.oil.saturating_add(oil);
        record_resource_income(&mut self.score.resource_income_history, tick, steel, oil);
        let cutoff = tick.saturating_sub(RESOURCE_WINDOW_MINUTE_TICKS);
        self.score
            .resource_income_history
            .retain(|entry| entry.tick >= cutoff);
    }

    pub(crate) fn observer_analysis_resources(
        &self,
        current_tick: u32,
    ) -> ObserverAnalysisResources {
        let history = &self.score.resource_income_history;
        ObserverAnalysisResources {
            lifetime: observer_resource_totals(self.score.resources_mined),
            last_5s: resource_income_in_window(history, current_tick, RESOURCE_WINDOW_5S_TICKS),
            last_minute: resource_income_in_window(
                history,
                current_tick,
                RESOURCE_WINDOW_MINUTE_TICKS,
            ),
        }
    }

    pub(crate) fn has_upgrade(&self, upgrade: UpgradeKind) -> bool {
        self.upgrades.contains(&upgrade)
    }

    pub(crate) fn reserve_supply(&mut self, supply: u32) -> bool {
        let Some(next) = self.supply_used.checked_add(supply) else {
            return false;
        };
        if next > self.supply_cap {
            return false;
        }
        self.supply_used = next;
        true
    }

    pub(crate) fn release_supply(&mut self, supply: u32) {
        self.supply_used = self.supply_used.saturating_sub(supply);
    }

    pub(crate) fn set_supply_counts(&mut self, used: u32, cap: u32) {
        self.supply_used = used;
        self.supply_cap = cap.min(config::SUPPLY_CAP_MAX);
    }

    pub(crate) fn reset_supply(&mut self) {
        self.set_supply_counts(0, 0);
    }
}

fn record_resource_income(
    history: &mut Vec<super::ResourceIncomeRecord>,
    tick: u32,
    steel: u32,
    oil: u32,
) {
    if let Some(last) = history.last_mut() {
        if last.tick == tick {
            last.steel = last.steel.saturating_add(steel);
            last.oil = last.oil.saturating_add(oil);
            return;
        }
        if last.tick < tick {
            history.push(super::ResourceIncomeRecord { tick, steel, oil });
            return;
        }
    }
    if let Some(entry) = history.iter_mut().find(|entry| entry.tick == tick) {
        entry.steel = entry.steel.saturating_add(steel);
        entry.oil = entry.oil.saturating_add(oil);
        return;
    }
    history.push(super::ResourceIncomeRecord { tick, steel, oil });
}

fn resource_income_in_window(
    history: &[super::ResourceIncomeRecord],
    current_tick: u32,
    window_ticks: u32,
) -> ObserverAnalysisResourceTotals {
    let cutoff = current_tick.saturating_sub(window_ticks);
    let mut totals = ObserverAnalysisResourceTotals::default();
    for entry in history {
        if entry.tick < cutoff {
            continue;
        }
        totals.steel = totals.steel.saturating_add(entry.steel);
        totals.oil = totals.oil.saturating_add(entry.oil);
    }
    totals
}

fn observer_resource_totals(totals: super::ResourceTotals) -> ObserverAnalysisResourceTotals {
    ObserverAnalysisResourceTotals {
        steel: totals.steel,
        oil: totals.oil,
    }
}
