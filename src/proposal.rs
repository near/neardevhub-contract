use crate::Contract;
use std::collections::HashSet;

use near_sdk::serde_json::json;
use near_sdk::{env, require, AccountId, Promise};

use devhub_common::social_db_contract;

use crate::notify::get_text_mentions;
use devhub_shared::proposal::{
    Proposal, ProposalBodyV2, ProposalId, ProposalSnapshot, VersionedProposalBody,
};

pub fn proposal_repost_text(proposal: Proposal) -> String {
    let proposal_link = format!("/devhub.near/widget/app?page=proposal&id={}", proposal.id);

    let title = proposal.snapshot.body.clone().latest_version().name;
    let summary = proposal.snapshot.body.clone().latest_version().summary;
    let category = proposal.snapshot.body.clone().latest_version().category;

    let text = format!(
        "We have just received a new *{category}* proposal.\n\n———\n\n**By**: @{author}\n\n**Title**: “{title}“\n\n**Summary**:\n\n{summary}\n\n———\n\nRead the full proposal and share your feedback on [DevHub]({proposal_link})",
        author = proposal.author_id,
        proposal_link = proposal_link,
        title = title,
        summary = summary,
        category = category
    );

    text
}

fn repost_internal(text: String, contract_address: AccountId) -> near_sdk::serde_json::Value {
    let main_value = json!({
        "type": "md",
        "text": text
    });

    json!({
        contract_address: {
            "post": {
                "main": main_value.to_string(),
            },
            "index": {
                "post": "{\"key\":\"main\",\"value\":{\"type\":\"md\"}}",
            }
        }
    })
}

pub fn publish_to_socialdb_feed(callback: Promise, text: String) -> Promise {
    social_db_contract()
        .with_static_gas(env::prepaid_gas().saturating_div(3))
        .with_attached_deposit(env::attached_deposit())
        .set(repost_internal(text, env::current_account_id()))
        .then(callback)
}

pub fn get_subscribers(proposal_body: &ProposalBodyV2) -> Vec<String> {
    let mut result = [
        get_text_mentions(proposal_body.description.as_str()),
        get_text_mentions(proposal_body.summary.as_str()),
    ]
    .concat();
    if let Some(supervisor) = proposal_body.supervisor.clone() {
        result.push(supervisor.to_string());
    }
    result.push(proposal_body.requested_sponsor.to_string());
    result
}

impl Contract {
    pub(crate) fn update_proposal_labels(
        &mut self,
        proposal_id: ProposalId,
        new_labels: HashSet<String>,
    ) -> ProposalId {
        let proposal: Proposal = self
            .proposals
            .get(proposal_id.into())
            .unwrap_or_else(|| panic!("Proposal id {} not found", proposal_id))
            .into();

        self.edit_proposal_internal(proposal_id, proposal.snapshot.body, new_labels)
    }

    pub(crate) fn edit_proposal_internal(
        &mut self,
        id: ProposalId,
        body: VersionedProposalBody,
        labels: HashSet<String>,
    ) -> ProposalId {
        require!(
            self.is_allowed_to_edit_proposal(id, Option::None),
            "The account is not allowed to edit this proposal"
        );
        let editor_id = env::predecessor_account_id();
        let mut proposal: Proposal = self
            .proposals
            .get(id.into())
            .unwrap_or_else(|| panic!("Proposal id {} not found", id))
            .into();

        let proposal_body = body.clone().latest_version();

        let old_body = proposal.snapshot.body.clone();
        let labels =
            self.update_and_check_rfp_link(id, body.clone(), Some(old_body.clone()), labels);

        let current_timeline = old_body.latest_version().timeline.latest_version();
        let new_timeline = proposal_body.timeline.latest_version();

        require!(
            self.has_moderator(editor_id.clone())
                || editor_id.clone() == env::current_account_id()
                || current_timeline.is_draft()
                    && (new_timeline.is_empty_review() || new_timeline.is_draft())
                || current_timeline.can_be_cancelled() && new_timeline.is_cancelled(),
            "This account is only allowed to change proposal status from DRAFT to REVIEW"
        );

        require!(
          new_timeline.is_draft() ||  new_timeline.is_review() || new_timeline.is_cancelled() || proposal_body.supervisor.is_some(),
          "You can't change the timeline of the proposal to this status without adding a supervisor"
      );

        require!(self.proposal_categories.contains(&proposal_body.category), "Unknown category");

        let old_snapshot = proposal.snapshot.clone();
        let old_labels_set = old_snapshot.labels.clone();
        let new_labels = labels;
        let new_snapshot = ProposalSnapshot {
            editor_id: editor_id.clone(),
            timestamp: env::block_timestamp(),
            labels: new_labels.clone(),
            body: body,
        };
        proposal.snapshot = new_snapshot;
        proposal.snapshot_history.push(old_snapshot);
        let proposal_author = proposal.author_id.clone();
        self.proposals.replace(id.try_into().unwrap(), &proposal.into());

        // Update labels index.
        let new_labels_set = new_labels;
        let labels_to_remove = &old_labels_set - &new_labels_set;
        let labels_to_add = &new_labels_set - &old_labels_set;
        require!(
            self.is_allowed_to_use_labels(
                Some(editor_id.clone()),
                labels_to_remove.iter().cloned().collect()
            ),
            "Not allowed to remove these labels"
        );
        require!(
            self.is_allowed_to_use_labels(
                Some(editor_id.clone()),
                labels_to_add.iter().cloned().collect()
            ),
            "Not allowed to add these labels"
        );

        for label_to_remove in labels_to_remove {
            let mut proposals = self.label_to_proposals.get(&label_to_remove).unwrap();
            proposals.remove(&id);
            self.label_to_proposals.insert(&label_to_remove, &proposals);
        }

        for label_to_add in labels_to_add {
            let mut proposals = self.label_to_proposals.get(&label_to_add).unwrap_or_default();
            proposals.insert(id);
            self.label_to_proposals.insert(&label_to_add, &proposals);
        }

        crate::notify::notify_edit_proposal(id, proposal_author);
        id
    }
}
