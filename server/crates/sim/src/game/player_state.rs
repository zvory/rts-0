use super::PlayerState;
use crate::config;
use crate::game::entity::EntityKind;
use crate::rules::economy::ResourceCost;

impl PlayerState {
    pub(crate) fn reset_for_dev_scenario(&mut self, start_tile: (u32, u32)) {
        self.start_tile = start_tile;
        self.steel = 0;
        self.oil = 10_000;
        self.supply_used = 0;
        self.supply_cap = 0;
        self.score = Default::default();
        self.upgrades.clear();
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

    pub(crate) fn add_gathered_resources(&mut self, kind: EntityKind, amount: u32) {
        match kind {
            EntityKind::Oil => self.oil = self.oil.saturating_add(amount),
            EntityKind::Steel => self.steel = self.steel.saturating_add(amount),
            _ => {}
        }
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
