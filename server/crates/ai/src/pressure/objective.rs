//! Timed geographic assault objectives; no hidden enemy ownership is inferred.
use super::Point;
use crate::sdk::{AiFrame, WorldQueries};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Objective {
    pub(super) enemy_id: u32,
    bases: [Point; 3],
    pub(super) phase: u8,
    targets: [Point; 3],
    count: usize,
}

impl Objective {
    pub(super) fn new(frame: &AiFrame, enemy_id: u32, main: Point) -> Self {
        let queries = WorldQueries::new(frame);
        let second = queries
            .base(enemy_id, 2)
            .map(|base| base.site.position)
            .unwrap_or(main);
        let third = queries
            .base(enemy_id, 3)
            .map(|base| base.site.position)
            .unwrap_or(second);
        Self {
            enemy_id,
            bases: [main, second, third],
            phase: 0,
            targets: [main; 3],
            count: 1,
        }
        .at_tick(frame.tick())
    }

    pub(super) fn at_tick(mut self, tick: u32) -> Self {
        self.phase = match tick / (rts_rules::balance::TICK_HZ * 240) {
            0 => 0,
            1 => 1,
            _ => 2,
        };
        let [main, second, third] = self.bases;
        let route = match self.phase {
            0 => [main, main, main],
            1 => [second, main, main],
            _ => [third, second, main],
        };
        self.targets = [main; 3];
        self.count = 0;
        for target in route {
            if self.count == 0 || self.targets[self.count - 1] != target {
                self.targets[self.count] = target;
                self.count += 1;
            }
        }
        self
    }

    pub(super) fn target(self) -> Point {
        self.targets[0]
    }

    pub(super) fn followups(&self) -> &[Point] {
        &self.targets[1..self.count]
    }

    pub(super) fn minimum_wave_size(self) -> usize {
        5 << self.phase
    }
}
