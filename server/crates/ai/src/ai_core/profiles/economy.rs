use super::AiProfile;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EconomyPolicy {
    Direct,
    ProposalManager,
}

impl AiProfile {
    pub(crate) fn uses_proposal_economy_manager(&self) -> bool {
        self.economy == EconomyPolicy::ProposalManager
    }
}
