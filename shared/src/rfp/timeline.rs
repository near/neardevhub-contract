use near_sdk::near;

#[near(serializers=[borsh, json])]
#[derive(Clone)]
#[serde(tag = "status", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimelineStatus {
    AcceptingSubmissions,
    Evaluation,
    ProposalSelected,
    Cancelled,
}

impl TimelineStatus {
    pub fn is_accepting_submissions(&self) -> bool {
        matches!(self, TimelineStatus::AcceptingSubmissions)
    }

    pub fn is_cancelled(&self) -> bool {
        matches!(self, TimelineStatus::Cancelled)
    }

    pub fn is_proposal_selected(&self) -> bool {
        matches!(self, TimelineStatus::ProposalSelected)
    }
}
