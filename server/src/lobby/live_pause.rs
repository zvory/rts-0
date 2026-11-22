use std::collections::HashMap;
use std::time::Duration;

use tokio::time::Instant as TokioInstant;

use super::room_task::helpers::{LIVE_PAUSE_LIMIT, MATCH_COUNTDOWN_WORDS};
use crate::protocol::{LivePauseState, LiveResumeCountdown};

const HOTKEY_SETUP_DURATION: Duration = Duration::from_secs(10);
const RESUME_DURATION: Duration = Duration::from_secs(3);

#[derive(Default)]
pub(super) struct LivePauseControl {
    paused: bool,
    paused_by: Option<u32>,
    resume_deadline: Option<TokioInstant>,
    setup_duration: Duration,
    counts: HashMap<u32, u8>,
}

impl LivePauseControl {
    pub(super) fn state_for(&self, actor_id: Option<u32>) -> LivePauseState {
        let resume_countdown = self.resume_deadline.map(|deadline| {
            let duration = self.setup_duration + RESUME_DURATION;
            LiveResumeCountdown {
                duration_ms: duration.as_millis() as u32,
                setup_duration_ms: self.setup_duration.as_millis() as u32,
                remaining_ms: deadline
                    .saturating_duration_since(TokioInstant::now())
                    .as_millis()
                    .clamp(1, u32::MAX as u128) as u32,
                words: MATCH_COUNTDOWN_WORDS
                    .iter()
                    .map(|word| (*word).into())
                    .collect(),
            }
        });
        let pauses_remaining = actor_id
            .map(|id| LIVE_PAUSE_LIMIT.saturating_sub(self.counts.get(&id).copied().unwrap_or(0)));
        LivePauseState {
            paused: self.paused,
            paused_by: self.paused_by,
            pauses_remaining,
            pause_limit: LIVE_PAUSE_LIMIT,
            can_pause: pauses_remaining.is_some_and(|remaining| !self.paused && remaining > 0),
            can_unpause: self.paused && resume_countdown.is_none() && actor_id.is_some(),
            resume_countdown,
        }
    }

    pub(super) fn pause(&mut self, actor_id: u32) -> bool {
        let used = self.counts.get(&actor_id).copied().unwrap_or(0);
        if self.paused || used >= LIVE_PAUSE_LIMIT {
            return false;
        }
        self.counts.insert(actor_id, used.saturating_add(1));
        self.paused = true;
        self.paused_by = Some(actor_id);
        self.resume_deadline = None;
        true
    }

    pub(super) fn start_resume(&mut self) -> bool {
        if !self.paused || self.resume_deadline.is_some() {
            return false;
        }
        self.setup_duration = Duration::ZERO;
        self.resume_deadline = Some(TokioInstant::now() + RESUME_DURATION);
        true
    }

    /// Initial replay takeover needs time to rebuild local control groups, without spending a pause.
    pub(super) fn start_replay_setup(&mut self) {
        self.paused = true;
        self.paused_by = None;
        self.setup_duration = HOTKEY_SETUP_DURATION;
        self.resume_deadline = Some(TokioInstant::now() + HOTKEY_SETUP_DURATION + RESUME_DURATION);
    }

    pub(super) fn finish_resume_if_due(&mut self) -> bool {
        if self
            .resume_deadline
            .is_none_or(|deadline| TokioInstant::now() < deadline)
        {
            return false;
        }
        self.resume_deadline = None;
        self.paused = false;
        self.paused_by = None;
        true
    }

    pub(super) fn is_paused(&self) -> bool {
        self.paused
    }

    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    #[cfg(test)]
    pub(super) fn paused_by(&self) -> Option<u32> {
        self.paused_by
    }

    #[cfg(test)]
    pub(super) fn used(&self, actor_id: u32) -> Option<&u8> {
        self.counts.get(&actor_id)
    }

    #[cfg(test)]
    pub(super) fn force_paused(&mut self) {
        self.paused = true;
    }

    #[cfg(test)]
    pub(super) fn force_resume_due(&mut self) {
        self.resume_deadline = Some(TokioInstant::now());
    }
}
