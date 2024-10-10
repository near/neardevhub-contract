use crate::notify::get_text_mentions;
use crate::Contract;
use devhub_shared::proposal::*;
use devhub_shared::rfp::*;
use near_sdk::{env, require, AccountId};
use std::collections::HashSet;

pub fn get_subscribers(proposal_body: &RFPBodyV0) -> Vec<String> {
    let result = [
        get_text_mentions(proposal_body.description.as_str()),
        get_text_mentions(proposal_body.summary.as_str()),
    ]
    .concat();
    result
}

pub fn rfp_repost_text(rfp: RFP) -> String {
    let rfp_link = format!("/devhub.near/widget/app?page=rfp&id={}", rfp.id);

    let body = rfp.snapshot.body.latest_version();

    let title = body.name;
    let summary = body.summary;

    let text = format!(
      "A new Request for Proposals is published.\n\n———\n\n**Title**: “{title}“\n\n**Summary**:\n\n{summary}\n\n———\n\nRead the full RFP and participate [here]({rfp_link})",
      rfp_link = rfp_link,
      title = title,
      summary = summary,
  );

    text
}

impl Contract {
    fn assert_can_link_unlink_rfp(&self, rfp_id: Option<RFPId>) {
        if let Some(rfp_id) = rfp_id {
            let rfp: RFP = self
                .rfps
                .get(rfp_id.into())
                .unwrap_or_else(|| panic!("RFP id {} not found", rfp_id))
                .into();
            require!(
              rfp.snapshot.body.latest_version().timeline.is_accepting_submissions() || self.is_allowed_to_write_rfps(env::predecessor_account_id()),
              format!("The RFP {} is not in the Accepting Submissions state, so you can't link or unlink to this RFP", rfp_id)
          );
        }
    }

    fn get_rfp_labels(&self, rfp_id: RFPId) -> HashSet<String> {
        let rfp: RFP = self
            .rfps
            .get(rfp_id.into())
            .unwrap_or_else(|| panic!("RFP id {} not found", rfp_id))
            .into();
        rfp.snapshot.labels
    }

    pub(crate) fn get_linked_proposals_in_rfp(&self, rfp_id: RFPId) -> HashSet<ProposalId> {
        let rfp: RFP = self.get_rfp(rfp_id).into();
        rfp.snapshot.linked_proposals
    }

    fn change_linked_proposal_in_rfp(
        &mut self,
        rfp_id: RFPId,
        proposal_id: ProposalId,
        operation: LinkedProposalChangeOperation,
    ) {
        let mut rfp: RFP = self.get_rfp(rfp_id).into();
        let mut linked_proposals = rfp.snapshot.linked_proposals.clone();
        match operation {
            LinkedProposalChangeOperation::Add => {
                linked_proposals.insert(proposal_id);
            }
            LinkedProposalChangeOperation::Remove => {
                linked_proposals.remove(&proposal_id);
            }
        }
        rfp.snapshot_history.push(rfp.snapshot.block_height);
        let new_snapshot = RFPSnapshot {
            editor_id: env::predecessor_account_id(),
            timestamp: env::block_timestamp(),
            block_height: env::block_height(),
            labels: rfp.snapshot.labels,
            body: rfp.snapshot.body,
            linked_proposals: linked_proposals,
        };
        rfp.snapshot = new_snapshot;
        self.rfps.replace(rfp_id.try_into().unwrap(), &rfp.clone().into());
    }

    fn add_linked_proposal_in_rfp(&mut self, rfp_id: RFPId, proposal_id: ProposalId) {
        self.change_linked_proposal_in_rfp(rfp_id, proposal_id, LinkedProposalChangeOperation::Add);
    }

    fn remove_linked_proposal_in_rfp(&mut self, rfp_id: RFPId, proposal_id: ProposalId) {
        self.change_linked_proposal_in_rfp(
            rfp_id,
            proposal_id,
            LinkedProposalChangeOperation::Remove,
        );
    }

    pub(crate) fn update_and_check_rfp_link(
        &mut self,
        proposal_id: ProposalId,
        new_proposal_body: VersionedProposalBody,
        old_proposal_body: Option<VersionedProposalBody>,
        labels: HashSet<String>,
    ) -> HashSet<String> {
        let mut labels = labels;
        let new_body = new_proposal_body.clone().latest_version();
        let old_rfp_id =
            old_proposal_body.clone().map(|old| old.latest_version().linked_rfp).flatten();
        if new_body.linked_rfp != old_rfp_id {
            self.assert_can_link_unlink_rfp(new_body.linked_rfp);
            self.assert_can_link_unlink_rfp(old_rfp_id);
            if let Some(old_rfp_id) = old_rfp_id {
                self.remove_linked_proposal_in_rfp(old_rfp_id, proposal_id);
            }
            if let Some(new_rfp_id) = new_body.linked_rfp {
                self.add_linked_proposal_in_rfp(new_rfp_id, proposal_id);
            }
        }
        if let Some(new_rfp_id) = new_body.linked_rfp {
            labels = self.get_rfp_labels(new_rfp_id);
        }
        labels
    }

    pub(crate) fn edit_rfp_internal(
        &mut self,
        id: RFPId,
        body: VersionedRFPBody,
        labels: HashSet<String>,
    ) -> RFPId {
        let editor_id: AccountId = env::predecessor_account_id();
        require!(
            self.is_allowed_to_write_rfps(editor_id.clone()),
            "The account is not allowed to edit RFPs"
        );

        let mut rfp: RFP = self.get_rfp(id).into();

        let rfp_body = body.clone().latest_version();

        if rfp_body.timeline.is_proposal_selected() {
            let has_approved_proposal = self
                .get_rfp_linked_proposals(id)
                .into_iter()
                .filter_map(|proposal_id| self.proposals.get(proposal_id.into()))
                .any(|proposal| {
                    Into::<Proposal>::into(proposal)
                        .snapshot
                        .body
                        .latest_version()
                        .timeline
                        .latest_version()
                        .was_approved()
                });
            require!(has_approved_proposal, "Cannot change RFP status to Proposal Selected without an approved proposal linked to this RFP");
        }

        let old_snapshot = rfp.snapshot.clone();
        let old_labels_set = old_snapshot.labels.clone();
        let new_labels = labels;
        rfp.snapshot_history.push(rfp.snapshot.block_height);
        let new_snapshot = RFPSnapshot {
            editor_id: env::predecessor_account_id(),
            timestamp: env::block_timestamp(),
            block_height: env::block_height(),
            labels: new_labels.clone(),
            body: body,
            linked_proposals: old_snapshot.linked_proposals.clone(),
        };
        rfp.snapshot = new_snapshot;
        self.rfps.replace(id.try_into().unwrap(), &rfp.clone().into());

        // Update labels index.
        let new_labels_set = new_labels;

        if old_labels_set != new_labels_set {
            for proposal_id in self.get_rfp_linked_proposals(id) {
                self.update_proposal_labels(proposal_id, new_labels_set.clone());
            }
        }

        let labels_to_remove = &old_labels_set - &new_labels_set;
        let labels_to_add: HashSet<String> = &new_labels_set - &old_labels_set;
        for label_to_remove in labels_to_remove {
            let mut rfps = self.label_to_rfps.get(&label_to_remove).unwrap();
            rfps.remove(&id);
            self.label_to_rfps.insert(&label_to_remove, &rfps);
        }

        for label_to_add in labels_to_add {
            let mut rfps = self.label_to_rfps.get(&label_to_add).unwrap_or_default();
            rfps.insert(id);
            self.label_to_rfps.insert(&label_to_add, &rfps);
        }

        crate::notify::notify_rfp_subscribers(&rfp, self.get_moderators());
        id
    }
}
