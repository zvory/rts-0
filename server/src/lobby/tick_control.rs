use super::session_policy::{
    ClockCapability, ClockTickSource, RoomTimeOperation, RoomTimeOperations, RoomTimeSource,
};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct RoomTimeClock {
    pub(super) speed: f32,
    pub(super) paused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TickMode {
    Idle,
    LiveMatch,
    RoomControlled(RoomTimeSource),
    RoomControlledPaused(RoomTimeSource),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScheduledTickAction {
    Noop,
    Countdown,
    LiveMatch,
    RoomControlled(RoomTimeSource),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum RoomTimeSpeed {
    Paused,
    Running(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TickControl {
    mode: TickMode,
    room_time_operations: RoomTimeOperations,
    countdown_active: bool,
    speed_multiplier: f32,
}

impl TickControl {
    pub(super) fn new(
        clock: ClockCapability,
        room_time: Option<RoomTimeClock>,
        fallback_speed: f32,
        countdown_active: bool,
    ) -> Self {
        let mut room_time_operations = RoomTimeOperations::NONE;
        let (mode, speed_multiplier) = match clock {
            ClockCapability::RoomControlled(capability) => {
                room_time_operations = capability.operations;
                let room_time = room_time.unwrap_or(RoomTimeClock {
                    speed: fallback_speed,
                    paused: false,
                });
                if room_time.paused {
                    (TickMode::RoomControlledPaused(capability.source), 1.0)
                } else {
                    (TickMode::RoomControlled(capability.source), room_time.speed)
                }
            }
            ClockCapability::FixedRealtime(source) => match source {
                ClockTickSource::RoomTicker => (TickMode::Idle, fallback_speed),
                ClockTickSource::LiveMatch => (TickMode::LiveMatch, fallback_speed),
                ClockTickSource::BranchStaging => (TickMode::Idle, 1.0),
            },
        };
        Self {
            mode,
            room_time_operations,
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
            TickMode::RoomControlledPaused(_) => ScheduledTickAction::Noop,
            _ if self.countdown_active => ScheduledTickAction::Countdown,
            TickMode::LiveMatch => ScheduledTickAction::LiveMatch,
            TickMode::RoomControlled(source) => ScheduledTickAction::RoomControlled(source),
            TickMode::Idle => ScheduledTickAction::Noop,
        }
    }

    pub(super) fn allows_room_time_operation(
        self,
        operation: RoomTimeOperation,
        player_in_room: bool,
    ) -> bool {
        player_in_room && self.room_time_operations.allows(operation)
    }

    pub(super) fn can_step_room_time(self, player_in_room: bool) -> bool {
        self.allows_room_time_operation(RoomTimeOperation::Step, player_in_room)
            && matches!(self.mode, TickMode::RoomControlledPaused(_))
    }

    pub(super) fn room_time_speed(speed: f32) -> RoomTimeSpeed {
        if speed == 0.0 {
            RoomTimeSpeed::Paused
        } else {
            RoomTimeSpeed::Running(speed.clamp(0.125, 8.0))
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

        let normal = TickControl::new(ClockCapability::ROOM_TICKER, None, 1.0, false);
        assert_duration_close(normal.tick_interval(base), Duration::from_millis(90));
        assert_eq!(normal.scheduled_action(), ScheduledTickAction::Noop);

        let replay = TickControl::new(
            ClockCapability::REPLAY_PLAYBACK,
            Some(RoomTimeClock {
                speed: 2.0,
                paused: false,
            }),
            1.0,
            false,
        );
        assert_duration_close(replay.tick_interval(base), Duration::from_millis(45));
        assert_eq!(
            replay.scheduled_action(),
            ScheduledTickAction::RoomControlled(RoomTimeSource::ReplayPlayback)
        );
        assert!(replay.allows_room_time_operation(RoomTimeOperation::SetSpeed, true));
        assert!(replay.allows_room_time_operation(RoomTimeOperation::SeekRelative, true));
        assert!(replay.allows_room_time_operation(RoomTimeOperation::SeekAbsolute, true));
        assert!(!replay.allows_room_time_operation(RoomTimeOperation::Step, true));

        let paused_replay = TickControl::new(
            ClockCapability::REPLAY_PLAYBACK,
            Some(RoomTimeClock {
                speed: 0.0,
                paused: true,
            }),
            1.0,
            false,
        );
        assert_duration_close(paused_replay.tick_interval(base), Duration::from_millis(90));
        assert_eq!(paused_replay.scheduled_action(), ScheduledTickAction::Noop);

        let dev_watch = TickControl::new(
            ClockCapability::DEV_SCENARIO,
            Some(RoomTimeClock {
                speed: 3.0,
                paused: false,
            }),
            1.0,
            false,
        );
        assert_duration_close(dev_watch.tick_interval(base), Duration::from_millis(30));
        assert_eq!(
            dev_watch.scheduled_action(),
            ScheduledTickAction::RoomControlled(RoomTimeSource::DevScenario)
        );
        assert!(dev_watch.allows_room_time_operation(RoomTimeOperation::SetSpeed, true));
        assert!(dev_watch.allows_room_time_operation(RoomTimeOperation::Step, true));
        assert!(!dev_watch.allows_room_time_operation(RoomTimeOperation::SeekRelative, true));

        let paused_dev_watch = TickControl::new(
            ClockCapability::DEV_SCENARIO,
            Some(RoomTimeClock {
                speed: 3.0,
                paused: true,
            }),
            1.0,
            false,
        );
        assert_duration_close(
            paused_dev_watch.tick_interval(base),
            Duration::from_millis(90),
        );
        assert_eq!(
            paused_dev_watch.scheduled_action(),
            ScheduledTickAction::Noop
        );
        assert!(paused_dev_watch.can_step_room_time(true));

        let lab = TickControl::new(
            ClockCapability::LAB,
            Some(RoomTimeClock {
                speed: 2.0,
                paused: false,
            }),
            1.0,
            false,
        );
        assert_duration_close(lab.tick_interval(base), Duration::from_millis(45));
        assert_eq!(
            lab.scheduled_action(),
            ScheduledTickAction::RoomControlled(RoomTimeSource::Lab)
        );
        assert!(lab.allows_room_time_operation(RoomTimeOperation::SetSpeed, true));
        assert!(lab.allows_room_time_operation(RoomTimeOperation::Step, true));
        assert!(!lab.allows_room_time_operation(RoomTimeOperation::SeekRelative, true));
        assert!(!lab.allows_room_time_operation(RoomTimeOperation::SeekAbsolute, true));

        let paused_lab = TickControl::new(
            ClockCapability::LAB,
            Some(RoomTimeClock {
                speed: 2.0,
                paused: true,
            }),
            1.0,
            false,
        );
        assert_eq!(paused_lab.scheduled_action(), ScheduledTickAction::Noop);
        assert!(paused_lab.can_step_room_time(true));

        let branch_staging = TickControl::new(ClockCapability::BRANCH_STAGING, None, 3.0, false);
        assert_duration_close(
            branch_staging.tick_interval(base),
            Duration::from_millis(90),
        );
        assert_eq!(branch_staging.scheduled_action(), ScheduledTickAction::Noop);

        let countdown = TickControl::new(ClockCapability::ROOM_TICKER, None, 1.0, true);
        assert_eq!(countdown.scheduled_action(), ScheduledTickAction::Countdown);
    }

    #[test]
    fn room_time_speed_preserves_pause_and_clamps_running_speed() {
        assert_eq!(TickControl::room_time_speed(0.0), RoomTimeSpeed::Paused);
        assert_eq!(
            TickControl::room_time_speed(0.01),
            RoomTimeSpeed::Running(0.125)
        );
        assert_eq!(
            TickControl::room_time_speed(12.0),
            RoomTimeSpeed::Running(8.0)
        );
    }
}
