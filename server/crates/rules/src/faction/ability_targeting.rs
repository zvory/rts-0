#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityTargetMode {
    SelfTarget,
    WorldPoint,
    FriendlyUnit,
}

impl AbilityTargetMode {
    pub fn stable_id(self) -> &'static str {
        match self {
            AbilityTargetMode::SelfTarget => "self",
            AbilityTargetMode::WorldPoint => "worldPoint",
            AbilityTargetMode::FriendlyUnit => "friendlyUnit",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityQueuePolicy {
    NotQueueable,
    QueueSkipIfNotReady,
    QueueWaitUntilReady,
}

impl AbilityQueuePolicy {
    pub fn stable_id(self) -> &'static str {
        match self {
            AbilityQueuePolicy::NotQueueable => "notQueueable",
            AbilityQueuePolicy::QueueSkipIfNotReady => "skipIfNotReady",
            AbilityQueuePolicy::QueueWaitUntilReady => "waitUntilReady",
        }
    }

    pub fn may_queue(self) -> bool {
        !matches!(self, AbilityQueuePolicy::NotQueueable)
    }
}
