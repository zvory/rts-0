use super::session_policy::ClockPolicy;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ReplayPlaybackClock {
    pub(super) speed: f32,
    pub(super) paused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TickMode {
    RoomTicker,
    LiveMatch,
    ReplayPlayback,
    ReplayPaused,
    DevWatch,
    DevWatchPaused,
    BranchStaging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScheduledTickAction {
    Noop,
    Countdown,
    LiveMatch,
    ReplayPlayback,
    DevWatch,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum DevWatchSpeed {
    Paused,
    Running(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TickControl {
    mode: TickMode,
    countdown_active: bool,
    speed_multiplier: f32,
}

impl TickControl {
    pub(super) fn new(
        clock: ClockPolicy,
        replay: Option<ReplayPlaybackClock>,
        dev_watch_paused: bool,
        fallback_speed: f32,
        countdown_active: bool,
    ) -> Self {
        let (mode, speed_multiplier) = match clock {
            ClockPolicy::ReplayPlayback => match replay {
                Some(replay) if replay.paused => (TickMode::ReplayPaused, 1.0),
                Some(replay) => (TickMode::ReplayPlayback, replay.speed),
                None => (TickMode::ReplayPlayback, fallback_speed),
            },
            ClockPolicy::DevWatch if dev_watch_paused => (TickMode::DevWatchPaused, 1.0),
            ClockPolicy::DevWatch => (TickMode::DevWatch, fallback_speed),
            ClockPolicy::BranchStaging => (TickMode::BranchStaging, 1.0),
            ClockPolicy::LiveMatch => (TickMode::LiveMatch, fallback_speed),
            ClockPolicy::RoomTicker => (TickMode::RoomTicker, fallback_speed),
        };
        Self {
            mode,
            countdown_active,
            speed_multiplier,
        }
    }

    pub(super) fn tick_interval(self, base: Duration) -> Duration {
        base.div_f32(self.speed_multiplier)
    }

    pub(super) fn speed_multiplier(self) -> f32 {
        self.speed_multiplier
    }

    pub(super) fn scheduled_action(self) -> ScheduledTickAction {
        match self.mode {
            TickMode::ReplayPaused | TickMode::DevWatchPaused => ScheduledTickAction::Noop,
            _ if self.countdown_active => ScheduledTickAction::Countdown,
            TickMode::LiveMatch => ScheduledTickAction::LiveMatch,
            TickMode::ReplayPlayback => ScheduledTickAction::ReplayPlayback,
            TickMode::DevWatch => ScheduledTickAction::DevWatch,
            TickMode::RoomTicker | TickMode::BranchStaging => ScheduledTickAction::Noop,
        }
    }

    pub(super) fn can_step_dev_tick(self, player_in_room: bool) -> bool {
        player_in_room && matches!(self.mode, TickMode::DevWatchPaused)
    }

    pub(super) fn dev_watch_speed(speed: f32) -> DevWatchSpeed {
        if speed == 0.0 {
            DevWatchSpeed::Paused
        } else {
            DevWatchSpeed::Running(speed.clamp(0.125, 8.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_duration_close(actual: Duration, expected: Duration) {
        assert!(
            actual.abs_diff(expected) <= Duration::from_micros(10),
            "expected {actual:?} to be within 10us of {expected:?}"
        );
    }

    #[test]
    fn tick_control_names_interval_decisions() {
        let base = Duration::from_millis(90);

        let normal = TickControl::new(ClockPolicy::RoomTicker, None, false, 1.0, false);
        assert_duration_close(normal.tick_interval(base), Duration::from_millis(90));
        assert_eq!(normal.scheduled_action(), ScheduledTickAction::Noop);

        let replay = TickControl::new(
            ClockPolicy::ReplayPlayback,
            Some(ReplayPlaybackClock {
                speed: 2.0,
                paused: false,
            }),
            false,
            1.0,
            false,
        );
        assert_duration_close(replay.tick_interval(base), Duration::from_millis(45));
        assert_eq!(
            replay.scheduled_action(),
            ScheduledTickAction::ReplayPlayback
        );

        let paused_replay = TickControl::new(
            ClockPolicy::ReplayPlayback,
            Some(ReplayPlaybackClock {
                speed: 0.0,
                paused: true,
            }),
            false,
            1.0,
            false,
        );
        assert_duration_close(paused_replay.tick_interval(base), Duration::from_millis(90));
        assert_eq!(paused_replay.scheduled_action(), ScheduledTickAction::Noop);

        let dev_watch = TickControl::new(ClockPolicy::DevWatch, None, false, 3.0, false);
        assert_duration_close(dev_watch.tick_interval(base), Duration::from_millis(30));
        assert_eq!(dev_watch.scheduled_action(), ScheduledTickAction::DevWatch);

        let paused_dev_watch = TickControl::new(ClockPolicy::DevWatch, None, true, 3.0, false);
        assert_duration_close(
            paused_dev_watch.tick_interval(base),
            Duration::from_millis(90),
        );
        assert_eq!(
            paused_dev_watch.scheduled_action(),
            ScheduledTickAction::Noop
        );
        assert!(paused_dev_watch.can_step_dev_tick(true));

        let branch_staging = TickControl::new(ClockPolicy::BranchStaging, None, false, 3.0, false);
        assert_duration_close(
            branch_staging.tick_interval(base),
            Duration::from_millis(90),
        );
        assert_eq!(branch_staging.scheduled_action(), ScheduledTickAction::Noop);

        let countdown = TickControl::new(ClockPolicy::RoomTicker, None, false, 1.0, true);
        assert_eq!(countdown.scheduled_action(), ScheduledTickAction::Countdown);
    }

    #[test]
    fn dev_watch_speed_preserves_pause_and_clamps_running_speed() {
        assert_eq!(TickControl::dev_watch_speed(0.0), DevWatchSpeed::Paused);
        assert_eq!(
            TickControl::dev_watch_speed(0.01),
            DevWatchSpeed::Running(0.125)
        );
        assert_eq!(
            TickControl::dev_watch_speed(12.0),
            DevWatchSpeed::Running(8.0)
        );
    }
}
