use super::*;

const APM_OPENING_SECONDS: u32 = 60;

impl PlayerState {
    pub(crate) fn record_entity_created(&mut self, kind: EntityKind) {
        let value = entity_score_value(kind);
        if kind.is_unit() {
            self.score.unit_score = self.score.unit_score.saturating_add(value);
        } else if kind.is_building() {
            self.score.structure_score = self.score.structure_score.saturating_add(value);
        }
    }

    pub(crate) fn record_construction_cancelled(&mut self, kind: EntityKind) {
        if kind.is_building() {
            self.score.structure_score = self
                .score
                .structure_score
                .saturating_sub(entity_score_value(kind));
        }
    }

    pub(crate) fn record_entity_lost(&mut self, kind: EntityKind) {
        if kind.is_unit() {
            self.score.units_lost = self.score.units_lost.saturating_add(1);
            let count = self.score.units_lost_by_kind.entry(kind).or_insert(0);
            *count = count.saturating_add(1);
        } else if kind.is_building() {
            self.score.buildings_lost = self.score.buildings_lost.saturating_add(1);
        }
    }

    pub(crate) fn record_entity_killed(&mut self, kind: EntityKind) {
        if kind.is_unit() {
            self.score.units_killed = self.score.units_killed.saturating_add(1);
        } else if kind.is_building() {
            self.score.buildings_killed = self.score.buildings_killed.saturating_add(1);
        }
    }
}

pub(super) fn entity_score_value(kind: EntityKind) -> u32 {
    let (steel, oil) = economy_rules::cost(kind);
    steel.saturating_add(oil)
}

impl Game {
    pub fn scores(&self) -> Vec<PlayerScore> {
        let duration_ticks = self.state.tick;
        self.state
            .players
            .iter()
            .map(|p| PlayerScore {
                id: p.id,
                team_id: p.team_id,
                name: p.name.clone(),
                color: p.color.clone(),
                apm: average_apm_after_opening(
                    self.state
                        .command_log
                        .iter()
                        .filter(|entry| entry.player_id == p.id)
                        .map(|entry| entry.tick),
                    duration_ticks,
                ),
                unit_score: p.score.unit_score,
                structure_score: p.score.structure_score,
                units_killed: p.score.units_killed,
                units_lost: p.score.units_lost,
                buildings_killed: p.score.buildings_killed,
                buildings_lost: p.score.buildings_lost,
            })
            .collect()
    }
}

fn average_apm_after_opening(action_ticks: impl Iterator<Item = u32>, duration_ticks: u32) -> u32 {
    let opening_ticks = config::TICK_HZ.saturating_mul(APM_OPENING_SECONDS);
    let measured_ticks = duration_ticks.saturating_sub(opening_ticks);
    if measured_ticks == 0 {
        return 0;
    }
    let actions = action_ticks
        .filter(|tick| *tick >= opening_ticks && *tick <= duration_ticks)
        .count() as u64;
    let numerator = actions
        .saturating_mul(config::TICK_HZ as u64)
        .saturating_mul(60);
    let rounded = numerator.saturating_add(measured_ticks as u64 / 2) / measured_ticks as u64;
    rounded.min(u32::MAX as u64) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_apm_excludes_first_minute_and_counts_each_command_once() {
        let minute = config::TICK_HZ * 60;
        let ticks = [1, minute - 1, minute, minute + 1, minute * 2];
        assert_eq!(average_apm_after_opening(ticks.into_iter(), minute * 2), 3);
    }

    #[test]
    fn score_apm_is_zero_until_the_opening_minute_ends() {
        assert_eq!(average_apm_after_opening([1, 30, 900].into_iter(), 1799), 0);
    }

    #[test]
    fn score_apm_scales_and_rounds_the_measured_match_rate() {
        let minute = config::TICK_HZ * 60;
        let action_ticks = std::iter::repeat(minute).take(31);
        assert_eq!(average_apm_after_opening(action_ticks, minute * 3), 16);
    }
}
