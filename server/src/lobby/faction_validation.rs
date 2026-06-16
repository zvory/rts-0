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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FactionLifecycleStatus {
    Playable,
    TestFixtureOnly,
    UnsupportedCatalog,
}

#[derive(Debug, Clone, Copy)]
struct FactionContextPolicy {
    context: FactionRequestContext,
    missing: MissingFactionPolicy,
    playable: bool,
    test_fixture: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MissingFactionPolicy {
    Default,
    Reject,
}

const CONTEXT_POLICIES: &[FactionContextPolicy] = &[
    FactionContextPolicy {
        context: FactionRequestContext::NormalLobby,
        missing: MissingFactionPolicy::Default,
        playable: true,
        test_fixture: false,
    },
    FactionContextPolicy {
        context: FactionRequestContext::Quickstart,
        missing: MissingFactionPolicy::Default,
        playable: true,
        test_fixture: false,
    },
    FactionContextPolicy {
        context: FactionRequestContext::AiSeat,
        missing: MissingFactionPolicy::Default,
        playable: true,
        test_fixture: false,
    },
    FactionContextPolicy {
        context: FactionRequestContext::ReplayPlayback,
        missing: MissingFactionPolicy::Reject,
        playable: true,
        test_fixture: false,
    },
    FactionContextPolicy {
        context: FactionRequestContext::ReplayBranch,
        missing: MissingFactionPolicy::Reject,
        playable: true,
        test_fixture: false,
    },
    FactionContextPolicy {
        context: FactionRequestContext::DevScenario,
        missing: MissingFactionPolicy::Default,
        playable: true,
        test_fixture: false,
    },
    FactionContextPolicy {
        context: FactionRequestContext::SelfPlay,
        missing: MissingFactionPolicy::Default,
        playable: true,
        test_fixture: false,
    },
    FactionContextPolicy {
        context: FactionRequestContext::TestFixture,
        missing: MissingFactionPolicy::Default,
        playable: false,
        test_fixture: true,
    },
];

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
        return match policy_for(context).missing {
            MissingFactionPolicy::Default => FactionValidation::Defaulted {
                faction_id: DEFAULT_FACTION_ID.to_string(),
            },
            MissingFactionPolicy::Reject => FactionValidation::Rejected {
                requested: None,
                reason: FactionRejectReason::MissingFaction,
            },
        };
    };

    if catalog_for(faction_id).is_none() {
        return FactionValidation::Rejected {
            requested: Some(faction_id.to_string()),
            reason: FactionRejectReason::UnknownCatalog,
        };
    }

    let policy = policy_for(context);
    match lifecycle_status_for(faction_id) {
        FactionLifecycleStatus::Playable if policy.playable => {
            FactionValidation::AcceptedPlayable {
                faction_id: faction_id.to_string(),
            }
        }
        FactionLifecycleStatus::TestFixtureOnly if policy.test_fixture => {
            FactionValidation::AcceptedFixture {
                faction_id: faction_id.to_string(),
            }
        }
        FactionLifecycleStatus::TestFixtureOnly => FactionValidation::Rejected {
            requested: Some(faction_id.to_string()),
            reason: FactionRejectReason::FixtureNotAllowed,
        },
        FactionLifecycleStatus::Playable | FactionLifecycleStatus::UnsupportedCatalog => {
            FactionValidation::Rejected {
                requested: Some(faction_id.to_string()),
                reason: FactionRejectReason::FactionNotAllowedInContext,
            }
        }
    }
}

fn policy_for(context: FactionRequestContext) -> FactionContextPolicy {
    CONTEXT_POLICIES
        .iter()
        .copied()
        .find(|policy| policy.context == context)
        .expect("every faction request context must have a policy")
}

fn lifecycle_status_for(faction_id: &str) -> FactionLifecycleStatus {
    if matches!(faction_id, DEFAULT_FACTION_ID | EKAT_FACTION_ID) {
        FactionLifecycleStatus::Playable
    } else if faction_id == EMPTY_FIXTURE_FACTION_ID {
        FactionLifecycleStatus::TestFixtureOnly
    } else {
        FactionLifecycleStatus::UnsupportedCatalog
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_CONTEXTS: &[FactionRequestContext] = &[
        FactionRequestContext::NormalLobby,
        FactionRequestContext::Quickstart,
        FactionRequestContext::AiSeat,
        FactionRequestContext::ReplayPlayback,
        FactionRequestContext::ReplayBranch,
        FactionRequestContext::DevScenario,
        FactionRequestContext::SelfPlay,
        FactionRequestContext::TestFixture,
    ];

    #[test]
    fn lifecycle_policy_table_covers_every_context() {
        for context in ALL_CONTEXTS {
            assert_eq!(policy_for(*context).context, *context);
        }
        assert_eq!(CONTEXT_POLICIES.len(), ALL_CONTEXTS.len());
    }

    #[test]
    fn context_policy_validates_missing_playable_fixture_and_unknown_ids() {
        for context in ALL_CONTEXTS {
            let policy = policy_for(*context);

            let missing = validate_faction_request(*context, None);
            if policy.missing == MissingFactionPolicy::Default {
                assert_eq!(
                    missing,
                    FactionValidation::Defaulted {
                        faction_id: DEFAULT_FACTION_ID.to_string()
                    },
                    "missing faction should default in {context:?}"
                );
                assert_eq!(default_faction_id_for(*context), DEFAULT_FACTION_ID);
            } else {
                assert_eq!(
                    missing,
                    FactionValidation::Rejected {
                        requested: None,
                        reason: FactionRejectReason::MissingFaction,
                    },
                    "missing faction should reject in {context:?}"
                );
                assert_eq!(default_faction_id_for(*context), DEFAULT_FACTION_ID);
            }

            for faction_id in [DEFAULT_FACTION_ID, EKAT_FACTION_ID] {
                let validation = validate_faction_request(*context, Some(faction_id));
                if policy.playable {
                    assert_eq!(
                        validation,
                        FactionValidation::AcceptedPlayable {
                            faction_id: faction_id.to_string()
                        },
                        "playable faction {faction_id:?} should be accepted in {context:?}"
                    );
                } else {
                    assert_eq!(
                        validation,
                        FactionValidation::Rejected {
                            requested: Some(faction_id.to_string()),
                            reason: FactionRejectReason::FactionNotAllowedInContext,
                        },
                        "playable faction {faction_id:?} should reject in {context:?}"
                    );
                }
            }

            let fixture = validate_faction_request(*context, Some(EMPTY_FIXTURE_FACTION_ID));
            if policy.test_fixture {
                assert_eq!(
                    fixture,
                    FactionValidation::AcceptedFixture {
                        faction_id: EMPTY_FIXTURE_FACTION_ID.to_string()
                    },
                    "fixture faction should be accepted in {context:?}"
                );
            } else {
                assert_eq!(
                    fixture,
                    FactionValidation::Rejected {
                        requested: Some(EMPTY_FIXTURE_FACTION_ID.to_string()),
                        reason: FactionRejectReason::FixtureNotAllowed,
                    },
                    "fixture faction should reject in {context:?}"
                );
            }

            assert_eq!(
                validate_faction_request(*context, Some("unknown_faction")),
                FactionValidation::Rejected {
                    requested: Some("unknown_faction".to_string()),
                    reason: FactionRejectReason::UnknownCatalog,
                },
                "unknown catalog should reject in {context:?}"
            );
        }
    }

    #[test]
    fn catalog_existence_does_not_define_runtime_admission() {
        assert!(catalog_for(EMPTY_FIXTURE_FACTION_ID).is_some());
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
            validate_faction_request(
                FactionRequestContext::AiSeat,
                Some(EMPTY_FIXTURE_FACTION_ID)
            ),
            FactionValidation::Rejected {
                requested: Some(EMPTY_FIXTURE_FACTION_ID.to_string()),
                reason: FactionRejectReason::FixtureNotAllowed,
            }
        );
        assert_eq!(
            validate_faction_request(
                FactionRequestContext::ReplayBranch,
                Some(EMPTY_FIXTURE_FACTION_ID)
            ),
            FactionValidation::Rejected {
                requested: Some(EMPTY_FIXTURE_FACTION_ID.to_string()),
                reason: FactionRejectReason::FixtureNotAllowed,
            }
        );
    }
}
