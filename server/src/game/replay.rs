//! Deterministic command-log replay for the simulation.
//!
//! A live [`Game`] records commands at the tick where they are applied, after AI controllers have
//! emitted their ordinary commands. Replays feed that exact log into a fresh game with AI thinking
//! disabled, so the log is the only source of player intent.

use serde::{Deserialize, Serialize};

use super::{Game, PlayerInit};
use crate::game::command::SimCommand;
use crate::protocol::{Command, Event, Snapshot};

/// One authoritative gameplay command, stamped with the simulation tick that applied it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandLogEntry {
    pub tick: u32,
    pub player_id: u32,
    pub command: Command,
}

/// One transient event emitted during replay, stamped with the tick that produced it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EventLogEntry {
    pub tick: u32,
    pub player_id: u32,
    pub event: Event,
}

/// Output from replaying a command log through a fresh [`Game`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReplayOutcome {
    pub ticks: u32,
    pub events: Vec<EventLogEntry>,
    pub final_snapshots: Vec<PlayerSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSnapshot {
    pub player_id: u32,
    pub snapshot: Snapshot,
}

/// Replay `commands` through tick `ticks`, preserving command order within each tick.
/// Used only by the test harness (`selfplay.rs`); kept alive for future replay UI.
#[allow(dead_code)]
pub fn replay_commands(
    players: &[PlayerInit],
    commands: &[CommandLogEntry],
    ticks: u32,
    seed: u32,
) -> Result<ReplayOutcome, ReplayError> {
    let mut replay = Game::new_for_replay(players, seed);
    let mut next_command = 0usize;
    let mut events = Vec::new();

    for tick in 1..=ticks {
        while let Some(entry) = commands.get(next_command) {
            if entry.tick < tick {
                return Err(ReplayError::OutOfOrder {
                    index: next_command,
                    tick: entry.tick,
                    previous_tick: tick,
                });
            }
            if entry.tick != tick {
                break;
            }
            replay.enqueue(
                entry.player_id,
                SimCommand::from_protocol(entry.command.clone()),
            );
            next_command += 1;
        }

        for (player_id, player_events) in replay.tick() {
            for event in player_events {
                events.push(EventLogEntry {
                    tick,
                    player_id,
                    event,
                });
            }
        }
    }

    if let Some(entry) = commands.get(next_command) {
        return Err(ReplayError::CommandAfterEnd {
            index: next_command,
            tick: entry.tick,
            replay_ticks: ticks,
        });
    }

    let final_snapshots = players
        .iter()
        .map(|p| PlayerSnapshot {
            player_id: p.id,
            snapshot: replay.snapshot_for(p.id),
        })
        .collect();

    Ok(ReplayOutcome {
        ticks,
        events,
        final_snapshots,
    })
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum ReplayError {
    OutOfOrder {
        index: usize,
        tick: u32,
        previous_tick: u32,
    },
    CommandAfterEnd {
        index: usize,
        tick: u32,
        replay_ticks: u32,
    },
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::OutOfOrder {
                index,
                tick,
                previous_tick,
            } => write!(
                f,
                "command log entry {index} has tick {tick}, before replay cursor {previous_tick}"
            ),
            ReplayError::CommandAfterEnd {
                index,
                tick,
                replay_ticks,
            } => write!(
                f,
                "command log entry {index} has tick {tick}, beyond replay length {replay_ticks}"
            ),
        }
    }
}

impl std::error::Error for ReplayError {}
