use rts_rules::faction::{
    catalog_for, DEFAULT_FACTION_ID, EKAT_FACTION_ID, EMPTY_FIXTURE_FACTION_ID,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FactionRequestContext {
    NormalLobby,
    Quickstart,
    AiSeat,
    ReplayPlayback,
    ReplayBranch,
    DevScenario,
    SelfPlay,
    TestFixture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FactionRejectReason {
    MissingFaction,
    UnknownCatalog,
    FixtureNotAllowed,
    FactionNotAllowedInContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FactionValidation {
    Defaulted {
        faction_id: String,
    },
    AcceptedPlayable {
        faction_id: String,
    },
    AcceptedFixture {
        faction_id: String,
    },
    Rejected {
        requested: Option<String>,
        reason: FactionRejectReason,
    },
}

pub(super) fn default_faction_id_for(context: FactionRequestContext) -> String {
    match validate_faction_request(context, None) {
        FactionValidation::Defaulted { faction_id }
        | FactionValidation::AcceptedPlayable { faction_id }
        | FactionValidation::AcceptedFixture { faction_id } => faction_id,
        FactionValidation::Rejected { .. } => DEFAULT_FACTION_ID.to_string(),
    }
}

pub(super) fn validate_faction_request(
    context: FactionRequestContext,
    requested: Option<&str>,
) -> FactionValidation {
    let requested = requested.and_then(|id| {
        let trimmed = id.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    });

    let Some(faction_id) = requested else {
        return match context {
            FactionRequestContext::ReplayPlayback | FactionRequestContext::ReplayBranch => {
                FactionValidation::Rejected {
                    requested: None,
                    reason: FactionRejectReason::MissingFaction,
                }
            }
            _ => FactionValidation::Defaulted {
                faction_id: DEFAULT_FACTION_ID.to_string(),
            },
        };
    };

    if catalog_for(faction_id).is_none() {
        return FactionValidation::Rejected {
            requested: Some(faction_id.to_string()),
            reason: FactionRejectReason::UnknownCatalog,
        };
    }

    if faction_id == EMPTY_FIXTURE_FACTION_ID {
        return if context == FactionRequestContext::TestFixture {
            FactionValidation::AcceptedFixture {
                faction_id: faction_id.to_string(),
            }
        } else {
            FactionValidation::Rejected {
                requested: Some(faction_id.to_string()),
                reason: FactionRejectReason::FixtureNotAllowed,
            }
        };
    }

    if matches!(faction_id, DEFAULT_FACTION_ID | EKAT_FACTION_ID) {
        return FactionValidation::AcceptedPlayable {
            faction_id: faction_id.to_string(),
        };
    }

    FactionValidation::Rejected {
        requested: Some(faction_id.to_string()),
        reason: FactionRejectReason::FactionNotAllowedInContext,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_lobby_defaults_to_kriegsia() {
        assert_eq!(
            validate_faction_request(FactionRequestContext::NormalLobby, None),
            FactionValidation::Defaulted {
                faction_id: "kriegsia".to_string()
            }
        );
    }

    #[test]
    fn normal_lobby_rejects_fixture_or_unknown_reserved_ids() {
        assert_eq!(
            validate_faction_request(
                FactionRequestContext::NormalLobby,
                Some(EMPTY_FIXTURE_FACTION_ID)
            ),
            FactionValidation::Rejected {
                requested: Some(EMPTY_FIXTURE_FACTION_ID.to_string()),
                reason: FactionRejectReason::FixtureNotAllowed,
            }
        );
        assert_eq!(
            validate_faction_request(FactionRequestContext::NormalLobby, Some("unknown_faction")),
            FactionValidation::Rejected {
                requested: Some("unknown_faction".to_string()),
                reason: FactionRejectReason::UnknownCatalog,
            }
        );
    }

    #[test]
    fn ekat_is_a_playable_faction() {
        assert_eq!(
            validate_faction_request(FactionRequestContext::NormalLobby, Some(EKAT_FACTION_ID)),
            FactionValidation::AcceptedPlayable {
                faction_id: EKAT_FACTION_ID.to_string()
            }
        );
    }

    #[test]
    fn ai_and_replay_contexts_accept_only_explicit_current_playable_policy() {
        assert_eq!(
            validate_faction_request(FactionRequestContext::AiSeat, Some(DEFAULT_FACTION_ID)),
            FactionValidation::AcceptedPlayable {
                faction_id: DEFAULT_FACTION_ID.to_string()
            }
        );
        assert_eq!(
            validate_faction_request(FactionRequestContext::ReplayPlayback, None),
            FactionValidation::Rejected {
                requested: None,
                reason: FactionRejectReason::MissingFaction,
            }
        );
        assert_eq!(
            validate_faction_request(
                FactionRequestContext::ReplayBranch,
                Some(DEFAULT_FACTION_ID)
            ),
            FactionValidation::AcceptedPlayable {
                faction_id: DEFAULT_FACTION_ID.to_string()
            }
        );
    }

    #[test]
    fn fixture_faction_is_test_fixture_only() {
        assert_eq!(
            validate_faction_request(
                FactionRequestContext::TestFixture,
                Some(EMPTY_FIXTURE_FACTION_ID)
            ),
            FactionValidation::AcceptedFixture {
                faction_id: EMPTY_FIXTURE_FACTION_ID.to_string()
            }
        );
        assert_eq!(
            validate_faction_request(
                FactionRequestContext::DevScenario,
                Some(EMPTY_FIXTURE_FACTION_ID)
            ),
            FactionValidation::Rejected {
                requested: Some(EMPTY_FIXTURE_FACTION_ID.to_string()),
                reason: FactionRejectReason::FixtureNotAllowed,
            }
        );
    }
}
