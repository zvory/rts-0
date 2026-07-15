//! Room-local lab timeline recording.
//!
//! The room task owns this alongside the authoritative lab `Game`. It records enough typed data
//! for a later seek/rebuild pass without making timeline state part of the simulation crate.

use crate::protocol::{
    Command, LabCheckpointScenarioV1, LabReplayOperation, LabReplayOperationEntry,
};
use rts_sim::game::lab::{LabCommandOptions, LabOp};
use rts_sim::game::Game;
use std::time::{Duration, Instant as StdInstant};

pub(super) struct LabTimeline {
    initial_setup: LabCheckpointScenarioV1,
    keyframes: Vec<LabTimelineKeyframe>,
    entries: Vec<LabTimelineEntry>,
    replay_entries: Vec<LabReplayOperationEntry>,
    next_sequence: u64,
    pending_entry_reservation: usize,
    last_seek_at: Option<StdInstant>,
}

pub(super) struct LabTimelineKeyframe {
    pub(super) tick: u32,
    pub(super) next_sequence: u64,
    pub(super) game: Box<Game>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct LabTimelineEntry {
    pub(super) sequence: u64,
    pub(super) tick: u32,
    pub(super) request_id: u32,
    pub(super) operator_id: u32,
    pub(super) kind: LabTimelineEntryKind,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum LabTimelineEntryKind {
    LabOperation {
        op_kind: String,
        op: LabOp,
    },
    IssueCommandAs {
        player_id: u32,
        command: Command,
        options: LabCommandOptions,
    },
}

pub(super) struct LabTimelineSeek {
    pub(super) target_tick: u32,
    pub(super) keyframe_tick: u32,
    pub(super) game: Game,
    pub(super) rebuild_ms: u128,
}

impl LabTimeline {
    pub(super) const KEYFRAME_INTERVAL_TICKS: u32 = 2_000;
    pub(super) const MAX_KEYFRAMES: usize = 64;
    pub(super) const MAX_ENTRIES: usize = 50_000;
    const SEEK_COOLDOWN: Duration = Duration::from_millis(500);

    pub(super) fn new(game: &Game, initial_setup: LabCheckpointScenarioV1) -> Self {
        let mut timeline = Self {
            initial_setup,
            keyframes: Vec::new(),
            entries: Vec::new(),
            replay_entries: Vec::new(),
            next_sequence: 0,
            pending_entry_reservation: 0,
            last_seek_at: None,
        };
        timeline.push_keyframe(game);
        timeline
    }

    pub(super) fn reset(&mut self, game: &Game, initial_setup: LabCheckpointScenarioV1) {
        self.initial_setup = initial_setup;
        self.keyframes.clear();
        self.entries.clear();
        self.replay_entries.clear();
        self.next_sequence = 0;
        self.pending_entry_reservation = 0;
        self.last_seek_at = None;
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
        replay_op: LabReplayOperation,
    ) {
        self.push_entry(
            tick,
            request_id,
            operator_id,
            LabTimelineEntryKind::LabOperation { op_kind, op },
            replay_op,
        );
    }

    pub(super) fn record_issue_command_as(
        &mut self,
        tick: u32,
        request_id: u32,
        operator_id: u32,
        player_id: u32,
        command: Command,
        options: LabCommandOptions,
    ) {
        self.push_entry(
            tick,
            request_id,
            operator_id,
            LabTimelineEntryKind::IssueCommandAs {
                player_id,
                command: command.clone(),
                options,
            },
            LabReplayOperation::IssueCommandAs {
                player_id,
                cmd: command,
                ignore_command_limits: options.ignore_command_limits,
            },
        );
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn record_replayed_entry(
        &mut self,
        replay_entry: LabReplayOperationEntry,
        kind: LabTimelineEntryKind,
    ) -> Result<(), String> {
        if replay_entry.sequence != self.next_sequence {
            return Err(format!(
                "Lab replay operation sequence mismatch: expected {}, got {}.",
                self.next_sequence, replay_entry.sequence
            ));
        }
        self.entries.push(LabTimelineEntry {
            sequence: replay_entry.sequence,
            tick: replay_entry.tick,
            request_id: replay_entry.request_id,
            operator_id: replay_entry.operator_id,
            kind,
        });
        self.replay_entries.push(replay_entry);
        self.next_sequence = self.next_sequence.saturating_add(1);
        debug_assert!(self.entries.len() <= Self::MAX_ENTRIES);
        debug_assert!(self.replay_entries.len() <= Self::MAX_ENTRIES);
        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn initial_setup(&self) -> &LabCheckpointScenarioV1 {
        &self.initial_setup
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn replay_entries(&self) -> &[LabReplayOperationEntry] {
        &self.replay_entries
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn replay_entry_count(&self) -> usize {
        self.replay_entries.len()
    }

    pub(super) fn keyframe_ticks(&self) -> Vec<u32> {
        self.keyframes
            .iter()
            .map(|keyframe| keyframe.tick)
            .collect()
    }

    pub(super) fn duration_ticks(&self, current_tick: u32) -> u32 {
        self.keyframes
            .iter()
            .map(|keyframe| keyframe.tick)
            .chain(self.entries.iter().map(|entry| entry.tick))
            .fold(current_tick, u32::max)
    }

    /// Keep one bounded scripted tick on the same side of an entry-cap rebase.
    pub(super) fn reserve_entries(&mut self, additional_entries: usize) {
        self.pending_entry_reservation = additional_entries;
    }

    pub(super) fn take_entry_cap_reset_required(&mut self) -> bool {
        let additional_entries = self.pending_entry_reservation.max(1);
        self.pending_entry_reservation = self.pending_entry_reservation.saturating_sub(1);
        self.entries.len().saturating_add(additional_entries) > Self::MAX_ENTRIES
            || self.replay_entries.len().saturating_add(additional_entries) > Self::MAX_ENTRIES
    }

    pub(super) fn truncate_future(&mut self, current_tick: u32) -> bool {
        let old_entry_count = self.entries.len();
        let old_replay_entry_count = self.replay_entries.len();
        let old_keyframe_count = self.keyframes.len();
        self.entries.retain(|entry| entry.tick <= current_tick);
        self.replay_entries
            .retain(|entry| entry.tick <= current_tick);
        self.keyframes
            .retain(|keyframe| keyframe.tick <= current_tick);
        if self.keyframes.is_empty() {
            return old_entry_count != self.entries.len()
                || old_replay_entry_count != self.replay_entries.len()
                || old_keyframe_count != self.keyframes.len();
        }
        self.recompute_next_sequence();
        old_entry_count != self.entries.len()
            || old_replay_entry_count != self.replay_entries.len()
            || old_keyframe_count != self.keyframes.len()
    }

    pub(super) fn seek_back(
        &mut self,
        current_tick: u32,
        ticks_back: u32,
        replay_entry: impl FnMut(&mut Game, &LabTimelineEntry) -> Result<(), String>,
    ) -> Result<LabTimelineSeek, String> {
        let target_tick = current_tick.saturating_sub(ticks_back);
        self.seek_to(current_tick, target_tick, replay_entry)
    }

    pub(super) fn seek_to(
        &mut self,
        current_tick: u32,
        target_tick: u32,
        mut replay_entry: impl FnMut(&mut Game, &LabTimelineEntry) -> Result<(), String>,
    ) -> Result<LabTimelineSeek, String> {
        if self
            .last_seek_at
            .is_some_and(|last_seek| last_seek.elapsed() < Self::SEEK_COOLDOWN)
        {
            return Err("Lab seek ignored; wait before seeking again.".to_string());
        }
        let target_tick = target_tick.min(self.duration_ticks(current_tick));
        let rebuild_start = StdInstant::now();
        let (keyframe_tick, keyframe_next_sequence, mut game) = self
            .keyframes
            .iter()
            .rev()
            .find(|keyframe| keyframe.tick <= target_tick)
            .map(|keyframe| {
                (
                    keyframe.tick,
                    keyframe.next_sequence,
                    keyframe.game.clone_for_replay_keyframe(),
                )
            })
            .ok_or_else(|| "Lab seek target is outside retained timeline history.".to_string())?;
        if target_tick.saturating_sub(keyframe_tick) > Self::KEYFRAME_INTERVAL_TICKS {
            return Err("Lab seek target is outside retained keyframe history.".to_string());
        }

        for entry in self
            .entries
            .iter()
            .filter(|entry| entry.sequence >= keyframe_next_sequence && entry.tick <= target_tick)
        {
            if entry.tick < game.tick_count() {
                return Err(format!(
                    "Lab timeline entry {} is out of order: tick {} before {}.",
                    entry.sequence,
                    entry.tick,
                    game.tick_count()
                ));
            }
            while game.tick_count() < entry.tick {
                game.tick();
            }
            replay_entry(&mut game, entry)?;
        }
        while game.tick_count() < target_tick {
            game.tick();
        }

        self.last_seek_at = Some(StdInstant::now());
        Ok(LabTimelineSeek {
            target_tick,
            keyframe_tick,
            game,
            rebuild_ms: rebuild_start.elapsed().as_millis(),
        })
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
        replay_op: LabReplayOperation,
    ) {
        let sequence = self.next_sequence;
        self.entries.push(LabTimelineEntry {
            sequence,
            tick,
            request_id,
            operator_id,
            kind,
        });
        self.replay_entries.push(LabReplayOperationEntry {
            sequence,
            tick,
            request_id,
            operator_id,
            op: replay_op,
        });
        self.next_sequence = self.next_sequence.saturating_add(1);
        debug_assert!(self.entries.len() <= Self::MAX_ENTRIES);
        debug_assert!(self.replay_entries.len() <= Self::MAX_ENTRIES);
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

    fn recompute_next_sequence(&mut self) {
        let entry_next = self
            .entries
            .last()
            .map(|entry| entry.sequence.saturating_add(1))
            .unwrap_or(0);
        let keyframe_next = self
            .keyframes
            .last()
            .map(|keyframe| keyframe.next_sequence)
            .unwrap_or(0);
        let replay_entry_next = self
            .replay_entries
            .last()
            .map(|entry| entry.sequence.saturating_add(1))
            .unwrap_or(0);
        self.next_sequence = entry_next.max(keyframe_next).max(replay_entry_next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab_scenarios::export_lab_checkpoint_scenario_for_protocol;
    use crate::protocol::{LabScenarioLabMetadata, LabVisionMode, DEFAULT_FACTION_ID};
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

    fn test_initial_setup(game: &Game) -> LabCheckpointScenarioV1 {
        export_lab_checkpoint_scenario_for_protocol(
            game,
            "timeline baseline".to_string(),
            LabScenarioLabMetadata {
                vision: LabVisionMode::All,
                god_mode_players: game.lab_god_mode_players(),
                initial_camera: None,
            },
            "test-build",
        )
        .expect("checkpoint baseline")
    }

    #[test]
    fn lab_timeline_starts_with_baseline_keyframe() {
        let game = test_game();
        let timeline = LabTimeline::new(&game, test_initial_setup(&game));

        assert_eq!(timeline.keyframe_ticks(), vec![0]);
        assert_eq!(timeline.duration_ticks(game.tick_count()), 0);
        assert!(timeline.entries().is_empty());
        assert!(timeline.replay_entries().is_empty());
        assert_eq!(timeline.keyframes[0].next_sequence, 0);
        assert_eq!(timeline.initial_setup().kind, "labCheckpointScenario");
    }

    #[test]
    fn lab_timeline_records_periodic_keyframes() {
        let mut game = test_game();
        let mut timeline = LabTimeline::new(&game, test_initial_setup(&game));

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
