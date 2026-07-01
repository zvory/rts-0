use super::*;

impl PlayerState {
    pub(crate) fn record_entity_created(&mut self, kind: EntityKind) {
        let value = entity_score_value(kind);
        if kind.is_unit() {
            self.score.unit_score = self.score.unit_score.saturating_add(value);
        } else if kind.is_building() {
            self.score.structure_score = self.score.structure_score.saturating_add(value);
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
        self.state.players
            .iter()
            .map(|p| PlayerScore {
                id: p.id,
                team_id: p.team_id,
                name: p.name.clone(),
                color: p.color.clone(),
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
