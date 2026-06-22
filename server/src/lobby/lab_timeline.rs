//! Room-local lab timeline recording.
//!
//! The room task owns this alongside the authoritative lab `Game`. It records enough typed data
//! for a later seek/rebuild pass without making timeline state part of the simulation crate.

use crate::protocol::Command;
use rts_sim::game::lab::LabOp;
use rts_sim::game::Game;

#[allow(dead_code)]
pub(super) struct LabTimeline {
    keyframes: Vec<LabTimelineKeyframe>,
    entries: Vec<LabTimelineEntry>,
    next_sequence: u64,
}

#[allow(dead_code)]
pub(super) struct LabTimelineKeyframe {
    pub(super) tick: u32,
    pub(super) next_sequence: u64,
    pub(super) game: Box<Game>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub(super) struct LabTimelineEntry {
    pub(super) sequence: u64,
    pub(super) tick: u32,
    pub(super) request_id: u32,
    pub(super) operator_id: u32,
    pub(super) kind: LabTimelineEntryKind,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub(super) enum LabTimelineEntryKind {
    LabOperation { op_kind: String, op: LabOp },
    IssueCommandAs { player_id: u32, command: Command },
}

impl LabTimeline {
    pub(super) const KEYFRAME_INTERVAL_TICKS: u32 = 2_000;
    pub(super) const MAX_KEYFRAMES: usize = 64;
    pub(super) const MAX_ENTRIES: usize = 50_000;

    pub(super) fn new(game: &Game) -> Self {
        let mut timeline = Self {
            keyframes: Vec::new(),
            entries: Vec::new(),
            next_sequence: 0,
        };
        timeline.push_keyframe(game);
        timeline
    }

    pub(super) fn reset(&mut self, game: &Game) {
        self.keyframes.clear();
        self.entries.clear();
        self.next_sequence = 0;
        self.push_keyframe(game);
    }

    pub(super) fn record_keyframe_if_due(&mut self, game: &Game) -> bool {
        let tick = game.tick_count();
        if tick == 0 || !tick.is_multiple_of(Self::KEYFRAME_INTERVAL_TICKS) {
            return false;
        }
        if self
            .keyframes
            .iter()
            .any(|keyframe| keyframe.tick == tick && keyframe.next_sequence == self.next_sequence)
        {
            return false;
        }
        self.push_keyframe(game);
        true
    }

    pub(super) fn record_lab_operation(
        &mut self,
        tick: u32,
        request_id: u32,
        operator_id: u32,
        op_kind: String,
        op: LabOp,
    ) {
        self.push_entry(
            tick,
            request_id,
            operator_id,
            LabTimelineEntryKind::LabOperation { op_kind, op },
        );
    }

    pub(super) fn record_issue_command_as(
        &mut self,
        tick: u32,
        request_id: u32,
        operator_id: u32,
        player_id: u32,
        command: Command,
    ) {
        self.push_entry(
            tick,
            request_id,
            operator_id,
            LabTimelineEntryKind::IssueCommandAs { player_id, command },
        );
    }

    pub(super) fn keyframe_ticks(&self) -> Vec<u32> {
        self.keyframes
            .iter()
            .map(|keyframe| keyframe.tick)
            .collect()
    }

    pub(super) fn duration_ticks(&self, current_tick: u32) -> u32 {
        self.keyframes
            .last()
            .map(|keyframe| current_tick.max(keyframe.tick))
            .unwrap_or(current_tick)
    }

    pub(super) fn is_entry_cap_reached(&self) -> bool {
        self.entries.len() >= Self::MAX_ENTRIES
    }

    #[cfg(test)]
    pub(super) fn entries(&self) -> &[LabTimelineEntry] {
        &self.entries
    }

    fn push_entry(
        &mut self,
        tick: u32,
        request_id: u32,
        operator_id: u32,
        kind: LabTimelineEntryKind,
    ) {
        self.entries.push(LabTimelineEntry {
            sequence: self.next_sequence,
            tick,
            request_id,
            operator_id,
            kind,
        });
        self.next_sequence = self.next_sequence.saturating_add(1);
        debug_assert!(self.entries.len() <= Self::MAX_ENTRIES);
    }

    fn push_keyframe(&mut self, game: &Game) {
        self.keyframes.push(LabTimelineKeyframe {
            tick: game.tick_count(),
            next_sequence: self.next_sequence,
            game: Box::new(game.clone_for_replay_keyframe()),
        });
        self.enforce_keyframe_cap();
    }

    fn enforce_keyframe_cap(&mut self) {
        if self.keyframes.len() <= Self::MAX_KEYFRAMES {
            return;
        }
        let remove_count = self.keyframes.len() - Self::MAX_KEYFRAMES;
        self.keyframes.drain(0..remove_count);
        self.prune_entries_before_first_keyframe();
    }

    fn prune_entries_before_first_keyframe(&mut self) {
        let Some(first_keyframe) = self.keyframes.first() else {
            return;
        };
        let first_needed_sequence = first_keyframe.next_sequence;
        let remove_count = self
            .entries
            .iter()
            .take_while(|entry| entry.sequence < first_needed_sequence)
            .count();
        if remove_count > 0 {
            self.entries.drain(0..remove_count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::DEFAULT_FACTION_ID;
    use rts_sim::game::PlayerInit;

    fn test_game() -> Game {
        let players = vec![
            PlayerInit {
                id: 1,
                team_id: 1,
                faction_id: DEFAULT_FACTION_ID.to_string(),
                name: "One".to_string(),
                color: "#0072b2".to_string(),
                is_ai: false,
            },
            PlayerInit {
                id: 2,
                team_id: 2,
                faction_id: DEFAULT_FACTION_ID.to_string(),
                name: "Two".to_string(),
                color: "#d55e00".to_string(),
                is_ai: false,
            },
        ];
        Game::new(&players, 1234)
    }

    #[test]
    fn lab_timeline_starts_with_baseline_keyframe() {
        let game = test_game();
        let timeline = LabTimeline::new(&game);

        assert_eq!(timeline.keyframe_ticks(), vec![0]);
        assert_eq!(timeline.duration_ticks(game.tick_count()), 0);
        assert!(timeline.entries().is_empty());
        assert_eq!(timeline.keyframes[0].next_sequence, 0);
    }

    #[test]
    fn lab_timeline_records_periodic_keyframes() {
        let mut game = test_game();
        let mut timeline = LabTimeline::new(&game);

        for _ in 0..LabTimeline::KEYFRAME_INTERVAL_TICKS {
            game.tick();
        }

        assert!(timeline.record_keyframe_if_due(&game));
        assert!(!timeline.record_keyframe_if_due(&game));
        assert_eq!(
            timeline.keyframe_ticks(),
            vec![0, LabTimeline::KEYFRAME_INTERVAL_TICKS]
        );
        assert_eq!(
            timeline.duration_ticks(game.tick_count()),
            LabTimeline::KEYFRAME_INTERVAL_TICKS
        );
    }
}
