pub mod access_control;
pub mod common;
pub mod community;
pub mod debug;
pub mod migrations;
mod notify;
pub mod proposal;
pub mod rfp;
pub mod stats;
pub mod web4;

use crate::access_control::members::ActionType;
use crate::access_control::members::Member;
use crate::access_control::AccessControl;
use community::*;

use common::*;
use devhub_shared::proposal::timeline::{
    TimelineStatus, TimelineStatusV1, VersionedTimelineStatus,
};
use devhub_shared::proposal::*;
use devhub_shared::rfp::timeline::TimelineStatus as RFPTimelineStatus;
use devhub_shared::rfp::*;

use devhub_common::{social_db_contract, SetReturnType};

use near_sdk::borsh::BorshDeserialize;
use near_sdk::collections::{LookupMap, UnorderedMap, Vector};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::serde_json::{json, Number, Value};
use near_sdk::store::Lazy;
use near_sdk::{env, near, require, AccountId, NearSchema, PanicOnDefault, Promise};
use web4::types::{Web4Request, Web4Response};

use std::collections::{HashMap, HashSet};
use std::convert::TryInto;

type PostId = u64;

/// An imaginary top post representing the landing page.
const ROOT_POST_ID: u64 = u64::MAX;

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Contract {
    pub posts: Vector<VersionedPost>,
    pub post_to_parent: LookupMap<PostId, PostId>,
    pub post_to_children: LookupMap<PostId, Vec<PostId>>,
    pub label_to_posts: UnorderedMap<String, HashSet<PostId>>,
    pub access_control: AccessControl,
    pub authors: UnorderedMap<AccountId, HashSet<PostId>>,
    pub proposals: Vector<VersionedProposal>,
    pub label_to_proposals: UnorderedMap<String, HashSet<ProposalId>>,
    pub author_proposals: UnorderedMap<AccountId, HashSet<ProposalId>>,
    pub proposal_categories: Vec<String>,
    pub rfps: Vector<VersionedRFP>,
    pub label_to_rfps: UnorderedMap<String, HashSet<RFPId>>,
    pub global_labels_info: Lazy<HashMap<String, LabelInfo>>,
    pub communities: UnorderedMap<CommunityHandle, Community>,
    pub featured_communities: Vec<FeaturedCommunity>,
    pub available_addons: UnorderedMap<AddOnId, AddOn>,
}

#[near]
impl Contract {
    #[init]
    pub fn new() -> Self {
        migrations::state_version_write(&migrations::StateVersion::V11);

        let mut contract = Self {
            posts: Vector::new(StorageKey::Posts),
            post_to_parent: LookupMap::new(StorageKey::PostToParent),
            post_to_children: LookupMap::new(StorageKey::PostToChildren),
            label_to_posts: UnorderedMap::new(StorageKey::LabelToPostsV2),
            access_control: AccessControl::default(),
            authors: UnorderedMap::new(StorageKey::AuthorToAuthorPosts),
            proposals: Vector::new(StorageKey::Proposals),
            label_to_proposals: UnorderedMap::new(StorageKey::LabelToProposals),
            author_proposals: UnorderedMap::new(StorageKey::AuthorProposals),
            proposal_categories: default_categories(),
            rfps: Vector::new(StorageKey::RFPs),
            label_to_rfps: UnorderedMap::new(StorageKey::LabelToRFPs),
            global_labels_info: Lazy::new(StorageKey::LabelInfo, HashMap::new()),
            communities: UnorderedMap::new(StorageKey::Communities),
            featured_communities: Vec::new(),
            available_addons: UnorderedMap::new(StorageKey::AddOns),
        };

        contract.post_to_children.insert(&ROOT_POST_ID, &Vec::new());
        contract
    }

    pub fn get_proposals(&self, ids: Option<Vec<ProposalId>>) -> Vec<VersionedProposal> {
        if let Some(ids) = ids {
            ids.into_iter().filter_map(|id| self.proposals.get(id.into())).collect()
        } else {
            self.proposals.to_vec()
        }
    }

    pub fn get_proposal(&self, proposal_id: ProposalId) -> VersionedProposal {
        self.proposals
            .get(proposal_id.into())
            .unwrap_or_else(|| panic!("Proposal id {} not found", proposal_id))
    }

    pub fn get_all_proposal_ids(&self) -> Vec<ProposalId> {
        (0..self.proposals.len().try_into().unwrap()).collect()
    }

    pub fn get_rfps(&self) -> Vec<VersionedRFP> {
        self.rfps.to_vec()
    }

    pub fn get_rfp(&self, rfp_id: RFPId) -> VersionedRFP {
        self.rfps.get(rfp_id.into()).unwrap_or_else(|| panic!("RFP id {} not found", rfp_id))
    }

    pub fn get_all_rfp_ids(&self) -> Vec<RFPId> {
        (0..self.rfps.len().try_into().unwrap()).collect()
    }

    #[payable]
    pub fn add_proposal(
        &mut self,
        body: VersionedProposalBody,
        labels: HashSet<String>,
        accepted_terms_and_conditions_version: Option<near_sdk::BlockHeight>,
    ) -> Promise {
        let id: ProposalId = self.proposals.len().try_into().unwrap();
        let author_id = env::predecessor_account_id();
        let editor_id = author_id.clone();

        if let Some(accepted_terms_and_conditions_version) = accepted_terms_and_conditions_version {
            require!(
                accepted_terms_and_conditions_version <= env::block_height(),
                "Terms and conditions version cannot be from the future"
            );
        } else {
            require!(
                env::current_account_id() != "devhub.near",
                "Accepted terms and conditions version is required"
            );
        }

        let proposal_body = body.clone().latest_version();

        if proposal_body.linked_rfp.is_some() {
            require!(labels.is_empty(), "Cannot add custom labels to this proposal. It inherits labels from the linked RFP. You should not add any labels to this proposal manually");
        }

        let labels = self.update_and_check_rfp_link(id, body.clone(), None, labels);

        require!(
            self.is_allowed_to_use_labels(
                Some(editor_id.clone()),
                labels.iter().cloned().collect()
            ),
            "Cannot use these labels"
        );

        require!(self.proposal_categories.contains(&proposal_body.category), "Unknown category");

        let timeline = proposal_body.timeline.clone().latest_version();

        require!(
            timeline.is_draft() || timeline.is_empty_review(),
            "Cannot create proposal which is not in a draft or a review state"
        );

        for label in &labels {
            let mut other_proposals = self.label_to_proposals.get(label).unwrap_or_default();
            other_proposals.insert(id);
            self.label_to_proposals.insert(label, &other_proposals);
        }

        let mut author_proposals = self.author_proposals.get(&author_id).unwrap_or_default();
        author_proposals.insert(id);
        self.author_proposals.insert(&author_id, &author_proposals);

        let proposal = Proposal {
            id: id,
            author_id: author_id.clone(),
            social_db_post_block_height: 0u64,
            snapshot: ProposalSnapshot {
                editor_id,
                timestamp: env::block_timestamp(),
                labels,
                body: body.clone(),
            },
            snapshot_history: vec![],
        };

        proposal::publish_to_socialdb_feed(
            Self::ext(env::current_account_id())
                .with_static_gas(env::prepaid_gas().saturating_div(4))
                .set_block_height_callback(proposal.clone()),
            proposal::proposal_repost_text(proposal.clone()),
        )
        .then(notify::notify_proposal_subscribers(&proposal))
    }

    #[payable]
    pub fn add_rfp(&mut self, body: VersionedRFPBody, labels: HashSet<String>) -> Promise {
        let id: RFPId = self.rfps.len().try_into().unwrap();
        let author_id = env::predecessor_account_id();
        let editor_id = author_id.clone();

        let rfp_body = body.clone().latest_version();

        require!(
            self.is_allowed_to_write_rfps(editor_id.clone()),
            "The account is not allowed to create RFPs"
        );

        require!(
            rfp_body.timeline.is_accepting_submissions(),
            "Cannot create proposal which is not in a Accepting Submissions state"
        );

        for label in &labels {
            require!(
                self.global_labels_info.get().get(label).is_some(),
                format!("Label {} is not registered", label)
            );

            let mut other_rfps = self.label_to_rfps.get(label).unwrap_or_default();
            other_rfps.insert(id);
            self.label_to_rfps.insert(label, &other_rfps);
        }

        let rfp = RFP {
            id: id,
            author_id: env::predecessor_account_id(),
            social_db_post_block_height: 0u64,
            snapshot: RFPSnapshot {
                editor_id: env::predecessor_account_id(),
                timestamp: env::block_timestamp(),
                block_height: env::block_height(),
                labels,
                body: body.clone(),
                linked_proposals: HashSet::new(),
            },
            snapshot_history: vec![],
        };

        proposal::publish_to_socialdb_feed(
            Self::ext(env::current_account_id())
                .with_static_gas(env::prepaid_gas().saturating_div(4))
                .set_rfp_block_height_callback(rfp.clone()),
            rfp::rfp_repost_text(rfp.clone()),
        )
        .then(notify::notify_rfp_subscribers(&rfp, self.get_moderators()))
    }

    #[private]
    pub fn set_block_height_callback(
        &mut self,
        #[allow(unused_mut)] mut proposal: Proposal,
        #[callback_unwrap] set_result: SetReturnType,
    ) -> BlockHeightCallbackRetValue {
        proposal.social_db_post_block_height = set_result.block_height.into();
        self.proposals.push(&proposal.clone().into());
        BlockHeightCallbackRetValue { proposal_id: proposal.id }
    }

    pub fn set_rfp_block_height_callback(
        &mut self,
        #[allow(unused_mut)] mut rfp: RFP,
        #[callback_unwrap] set_result: SetReturnType,
    ) -> BlockHeightCallbackRetValue {
        let ret_value = BlockHeightCallbackRetValue { proposal_id: rfp.id };
        rfp.social_db_post_block_height = set_result.block_height.into();
        self.rfps.push(&rfp.into());
        ret_value
    }

    pub fn get_proposals_by_author(&self, author: AccountId) -> Vec<ProposalId> {
        self.author_proposals
            .get(&author)
            .map(|proposals| proposals.into_iter().collect())
            .unwrap_or_default()
    }

    pub fn get_proposals_by_label(&self, label: String) -> Vec<ProposalId> {
        let mut res: Vec<_> =
            self.label_to_proposals.get(&label).unwrap_or_default().into_iter().collect();
        res.sort();
        res
    }

    pub fn get_rfps_by_label(&self, label: String) -> Vec<RFPId> {
        let mut res: Vec<_> =
            self.label_to_rfps.get(&label).unwrap_or_default().into_iter().collect();
        res.sort();
        res
    }

    pub fn get_all_proposal_labels(&self) -> Vec<String> {
        let mut res: Vec<_> = self.label_to_proposals.keys().collect();
        res.sort();
        res
    }

    pub fn get_all_authors(&self) -> Vec<AccountId> {
        let mut res: Vec<_> = self.authors.keys().collect();
        res.sort();
        res
    }

    pub fn get_all_proposal_authors(&self) -> Vec<AccountId> {
        let mut res: Vec<_> = self.author_proposals.keys().collect();
        res.sort();
        res
    }

    pub fn is_allowed_to_edit_proposal(
        &self,
        proposal_id: ProposalId,
        editor: Option<AccountId>,
    ) -> bool {
        let proposal: Proposal = self
            .proposals
            .get(proposal_id.try_into().unwrap())
            .unwrap_or_else(|| panic!("Proposal id {} not found", proposal_id))
            .into();
        let editor = editor.unwrap_or_else(env::predecessor_account_id);
        // First check for simple cases.
        if editor == env::current_account_id() || editor == proposal.author_id {
            return true;
        }

        // Then check for complex case.
        self.access_control
            .members_list
            .check_permissions(editor, proposal.snapshot.labels.into_iter().collect::<Vec<_>>())
            .contains(&ActionType::EditPost)
    }

    pub fn is_allowed_to_write_rfps(&self, editor: AccountId) -> bool {
        editor == env::current_account_id() || self.has_moderator(editor)
    }

    pub fn is_allowed_to_use_labels(&self, editor: Option<AccountId>, labels: Vec<String>) -> bool {
        let editor = editor.unwrap_or_else(env::predecessor_account_id);
        // First check for simple cases.
        if editor == env::current_account_id() {
            return true;
        }
        let restricted_labels = self.access_control.rules_list.find_restricted(&labels);
        if restricted_labels.is_empty() {
            return true;
        }
        self.access_control
            .members_list
            .check_permissions(editor, labels)
            .contains(&ActionType::UseLabels)
    }

    fn filtered_labels<T>(
        &self,
        labels_to_t: &UnorderedMap<String, T>,
        editor: &AccountId,
    ) -> Vec<String>
    where
        T: near_sdk::borsh::BorshSerialize + near_sdk::borsh::BorshDeserialize,
    {
        let filtered: HashSet<String> = labels_to_t
            .keys()
            .filter(|label| {
                self.is_allowed_to_use_labels(Some(editor.clone()), vec![label.clone()])
            })
            .collect();
        let mut res: Vec<_> = filtered.into_iter().collect();
        res.sort();
        res
    }

    pub fn get_all_allowed_proposal_labels(&self, editor: AccountId) -> Vec<String> {
        self.filtered_labels(&self.label_to_proposals, &editor)
    }

    #[payable]
    pub fn create_community(
        &mut self,
        #[allow(unused_mut)] mut inputs: CommunityInputs,
    ) -> Promise {
        require!(
            self.get_community(inputs.handle.to_owned()).is_none(),
            "Community already exists"
        );

        require!(
            env::attached_deposit() >= CREATE_COMMUNITY_BALANCE,
            "Require 4 NEAR to create community"
        );

        require!(env::prepaid_gas() >= CREATE_COMMUNITY_GAS, "Require at least 200 Tgas");

        let mut new_community = Community {
            admins: vec![],
            handle: inputs.handle.clone(),
            name: inputs.name,
            tag: inputs.tag,
            description: inputs.description,
            logo_url: inputs.logo_url,
            banner_url: inputs.banner_url,
            bio_markdown: inputs.bio_markdown,
            github_handle: None,
            telegram_handle: None,
            twitter_handle: None,
            website_url: None,
            addons: vec![
                CommunityAddOn {
                    id: "announcements".to_string(),
                    addon_id: "announcements".to_string(),
                    display_name: "Announcements".to_string(),
                    enabled: true,
                    parameters: "".to_string(),
                },
                CommunityAddOn {
                    id: "discussions".to_string(),
                    addon_id: "discussions".to_string(),
                    display_name: "Discussions".to_string(),
                    enabled: true,
                    parameters: "".to_string(),
                },
            ],
        };

        new_community.validate();
        new_community.set_default_admin();
        self.communities.insert(&new_community.handle, &new_community);

        ext_devhub_community_factory::ext(get_devhub_community_factory())
            .with_unused_gas_weight(1)
            .with_attached_deposit(CREATE_COMMUNITY_BALANCE)
            .create_community_account(new_community.handle.clone())
    }

    #[payable]
    pub fn edit_proposal(
        &mut self,
        id: ProposalId,
        body: VersionedProposalBody,
        labels: HashSet<String>,
    ) -> ProposalId {
        let proposal_body = body.clone().latest_version();
        if proposal_body.linked_rfp.is_some() {
            require!(labels.len() == 0, "Cannot edit labels of a proposal linked to RFP. It inherits labels from the linked RFP.");
        }
        self.edit_proposal_internal(id, body.clone(), labels)
    }

    #[payable]
    pub fn edit_proposal_timeline(
        &mut self,
        id: ProposalId,
        timeline: TimelineStatusV1,
    ) -> ProposalId {
        let proposal: Proposal = self
            .proposals
            .get(id.into())
            .unwrap_or_else(|| panic!("Proposal id {} not found", id))
            .into();
        let mut body = proposal.snapshot.body.latest_version();
        body.timeline = timeline.into();

        self.edit_proposal_internal(id, body.into(), proposal.snapshot.labels)
    }

    #[payable]
    pub fn edit_proposal_versioned_timeline(
        &mut self,
        id: ProposalId,
        timeline: VersionedTimelineStatus,
    ) -> ProposalId {
        near_sdk::log!("edit_proposal_versioned_timeline");
        let proposal: Proposal = self
            .proposals
            .get(id.into())
            .unwrap_or_else(|| panic!("Proposal id {} not found", id))
            .into();
        let mut body = proposal.snapshot.body.latest_version();
        body.timeline = timeline.into();

        self.edit_proposal_internal(id, body.into(), proposal.snapshot.labels)
    }

    #[payable]
    pub fn edit_proposal_linked_rfp(
        &mut self,
        id: ProposalId,
        rfp_id: Option<RFPId>,
    ) -> ProposalId {
        let proposal: Proposal = self
            .proposals
            .get(id.into())
            .unwrap_or_else(|| panic!("Proposal id {} not found", id))
            .into();
        let mut body = proposal.snapshot.body.latest_version();
        body.linked_rfp = rfp_id;

        self.edit_proposal_internal(id, body.into(), proposal.snapshot.labels)
    }

    #[payable]
    pub fn edit_rfp(
        &mut self,
        id: RFPId,
        body: VersionedRFPBody,
        labels: HashSet<String>,
    ) -> RFPId {
        self.edit_rfp_internal(id, body.clone(), labels)
    }

    #[payable]
    pub fn cancel_rfp(
        &mut self,
        id: RFPId,
        proposals_to_cancel: Vec<ProposalId>,
        proposals_to_unlink: Vec<ProposalId>,
    ) -> RFPId {
        for proposal_id in proposals_to_cancel {
            let proposal: Proposal = self.get_proposal(proposal_id).into();
            let proposal_timeline = proposal.snapshot.body.latest_version().timeline;
            let review_status = proposal_timeline.latest_version().get_review_status().clone();
            self.edit_proposal_versioned_timeline(
                proposal_id,
                TimelineStatus::Cancelled(review_status).into(),
            );
        }

        for proposal_id in proposals_to_unlink {
            self.edit_proposal_linked_rfp(proposal_id, None);
        }

        self.edit_rfp_timeline(id, RFPTimelineStatus::Cancelled)
    }

    #[payable]
    pub fn edit_rfp_timeline(&mut self, id: RFPId, timeline: RFPTimelineStatus) -> ProposalId {
        let rfp: RFP = self.get_rfp(id).into();
        let mut body = rfp.snapshot.body.latest_version();
        body.timeline = timeline;

        self.edit_rfp_internal(id, body.into(), rfp.snapshot.labels)
    }

    pub fn get_allowed_categories(&self) -> Vec<String> {
        self.proposal_categories.clone()
    }

    pub fn get_global_labels(&self) -> Vec<LabelInfoExtended> {
        let mut result: Vec<LabelInfoExtended> = self
            .global_labels_info
            .iter()
            .map(|(label, label_info)| LabelInfoExtended {
                value: label.clone(),
                title: label_info.title.clone(),
                color: label_info.color.clone(),
            })
            .collect();
        result.sort_by(|a, b| a.value.cmp(&b.value));
        result
    }

    pub fn get_rfp_linked_proposals(&self, rfp_id: RFPId) -> Vec<ProposalId> {
        self.get_linked_proposals_in_rfp(rfp_id).into_iter().collect()
    }

    #[payable]
    pub fn set_global_labels(&mut self, labels: Vec<LabelInfoExtended>) {
        let editor_id = env::predecessor_account_id();
        require!(
            self.has_moderator(editor_id.clone()) || editor_id.clone() == env::current_account_id(),
            "Only the admin and moderators can set labels"
        );

        self.global_labels_info.clear();

        for label in labels {
            let label_info = LabelInfo { title: label.title, color: label.color };
            (*self.global_labels_info).insert(label.value, label_info);
        }
    }

    #[payable]
    pub fn set_allowed_categories(&mut self, new_categories: Vec<String>) {
        let editor_id = env::predecessor_account_id();
        require!(
            self.has_moderator(editor_id.clone()) || editor_id.clone() == env::current_account_id(),
            "Only the admin and moderators can set categories"
        );
        self.proposal_categories = new_categories;
    }

    pub fn get_community(&self, handle: CommunityHandle) -> Option<Community> {
        self.communities.get(&handle)
    }

    pub fn get_community_metadata(&self, handle: CommunityHandle) -> Option<CommunityMetadata> {
        self.communities.get(&handle).map(|community| CommunityMetadata {
            admins: community.admins,
            handle: community.handle,
            name: community.name,
            tag: community.tag,
            description: community.description,
            logo_url: community.logo_url,
            banner_url: community.banner_url,
            bio_markdown: community.bio_markdown,
        })
    }

    pub fn get_account_community_permissions(
        &self,
        account_id: AccountId,
        community_handle: CommunityHandle,
    ) -> CommunityPermissions {
        let community = self.get_community(community_handle.to_owned()).expect(
            format!("Community with handle `{}` does not exist", community_handle).as_str(),
        );

        CommunityPermissions {
            can_configure: community.admins.contains(&account_id)
                || self.has_moderator(account_id.to_owned()),

            can_delete: self.has_moderator(account_id),
        }
    }

    pub fn get_all_communities_metadata(&self) -> Vec<CommunityMetadata> {
        self.communities
            .iter()
            .map(|(handle, community)| CommunityMetadata {
                admins: community.admins,
                handle,
                name: community.name,
                tag: community.tag,
                description: community.description,
                logo_url: community.logo_url,
                banner_url: community.banner_url,
                bio_markdown: community.bio_markdown,
            })
            .collect()
    }

    pub fn get_addon(&self, id: AddOnId) -> Option<AddOn> {
        self.available_addons.get(&id)
    }

    pub fn get_all_addons(&self) -> Vec<AddOn> {
        self.available_addons.iter().map(|(_id, add_on)| add_on).collect()
    }

    // Only the contract admin and DevHub moderators
    pub fn create_addon(&mut self, addon: AddOn) {
        if !self.has_moderator(env::predecessor_account_id())
            && env::predecessor_account_id() != env::current_account_id()
        {
            panic!("Only the admin and moderators can create new add-ons");
        }
        if self.get_addon(addon.id.to_owned()).is_some() {
            panic!("Add-on with this id already exists");
        }
        addon.validate();
        self.available_addons.insert(&addon.id.clone(), &addon);
    }

    // ONLY FOR TESTING
    pub fn delete_addon(&mut self, id: AddOnId) {
        // Also delete from communities
        if !self.has_moderator(env::predecessor_account_id())
            && env::predecessor_account_id() != env::current_account_id()
        {
            panic!("Only the admin and moderators can delete add-ons");
        }
        let addon = self
            .get_addon(id.clone())
            .expect(&format!("Add-on with id `{}` does not exist", id))
            .clone();

        self.available_addons.remove(&addon.id);
    }

    pub fn update_addon(&mut self, addon: AddOn) {
        if !self.has_moderator(env::predecessor_account_id())
            && env::predecessor_account_id() != env::current_account_id()
        {
            panic!("Only the admin and moderators can edit add-ons");
        }
        self.available_addons.insert(&addon.id.clone(), &addon);
    }

    pub fn get_community_addons(&self, handle: CommunityHandle) -> Vec<CommunityAddOn> {
        let community = self
            .get_community(handle.clone())
            .expect(format!("Community not found with handle `{}`", handle).as_str());
        return community.addons;
    }

    pub fn set_community_addons(
        &mut self,
        handle: CommunityHandle,
        addons: Vec<CommunityAddOn>,
    ) -> Promise {
        let mut community = self
            .get_community(handle.clone())
            .expect(format!("Community not found with handle `{}`", handle).as_str());
        community.addons = addons;
        self.update_community(handle, community)
    }

    // To add or update parameters set by the configurator widget
    pub fn set_community_addon(
        &mut self,
        handle: CommunityHandle,
        community_addon: CommunityAddOn,
    ) -> Promise {
        let mut community = self
            .get_community(handle.clone())
            .expect(format!("Community not found with handle `{}`", handle).as_str());
        if let Some(existing_addon) =
            community.addons.iter_mut().find(|current| current.id == community_addon.id)
        {
            *existing_addon = community_addon;
        } else {
            community.addons.push(community_addon);
        }
        self.update_community(handle, community)
    }

    fn get_editable_community(&self, handle: &CommunityHandle) -> Option<Community> {
        if self
            .get_account_community_permissions(env::predecessor_account_id(), handle.to_owned())
            .can_configure
        {
            return self.get_community(handle.to_owned());
        } else {
            return None;
        };
    }

    pub fn update_community(
        &mut self,
        handle: CommunityHandle,
        #[allow(unused_mut)] mut community: Community,
    ) -> Promise {
        let _ = self
            .get_editable_community(&handle)
            .expect("Only community admins and hub moderators can configure communities");

        community.validate();
        community.set_default_admin();

        require!(community.handle == handle, "Community handle cannot be changed");
        require!(env::prepaid_gas() >= UPDATE_COMMUNITY_GAS, "Require at least 30 Tgas");
        self.communities.insert(&handle, &community);
        let community_page_link =
            format!("/devhub.near/widget/app?page=community&handle={}", community.handle);
        social_db_contract().with_unused_gas_weight(1).set(json!({
            get_devhub_community_account(&community.handle): {
                "profile": {
                    "name": community.name,
                    "image": {
                        "url": community.logo_url,
                    },
                    "linktree": {
                        "twitter": community.twitter_handle,
                        "github": community.github_handle,
                        "telegram": community.telegram_handle,
                        "website": format!("near.social{community_page_link}"),
                    },
                    "description": format!(
                        "{}\n\nLearn more about our community [on DevHub]({}).",
                        community.bio_markdown.as_ref().unwrap_or(&community.description),
                        community_page_link
                    ),
                    "backgroundImage": {
                        "url": community.banner_url,
                    },
                    "tags": {
                        "community": "",
                        "announcements": "",
                        &community.handle: "",
                    }
                }
            },
            get_devhub_discussions_account(&community.handle):  {
                "profile": {
                    "name": format!("{} (Community Discussions)", community.name),
                    "image": {
                        "url": community.logo_url,
                    },
                    "linktree": {
                        "twitter": community.twitter_handle,
                        "github": community.github_handle,
                        "telegram": community.telegram_handle,
                        "website": format!("near.social{community_page_link}"),
                    },
                    "description": format!("{}\n\nLearn more about our community [on DevHub]({}).", community.description, community_page_link),
                    "backgroundImage": {
                        "url": community.banner_url,
                    },
                    "tags": {
                        "community": "",
                        "discussions": "",
                        &community.handle: "",
                    }
                }
            }
        }))
    }

    pub fn set_community_socialdb(&mut self, handle: CommunityHandle, data: Value) -> Promise {
        let _ = self
            .get_editable_community(&handle)
            .expect("Only community admins and hub moderators can set community Social DB");

        require!(env::prepaid_gas() >= SET_COMMUNITY_SOCIALDB_GAS, "Require at least 30 Tgas");
        social_db_contract()
            .with_unused_gas_weight(1)
            .set(json!({ get_devhub_community_account(&handle): data }))
    }

    pub fn create_discussion(&mut self, handle: CommunityHandle, block_height: Number) -> Promise {
        require!(env::prepaid_gas() >= CREATE_DISCUSSION_GAS, "Require at least 30 Tgas");

        let post_initiator = env::predecessor_account_id();
        let repost = format!("[{{\"key\":\"main\",\"value\":{{\"type\":\"repost\",\"item\":{{\"type\":\"social\",\"path\":\"{}/post/main\",\"blockHeight\":{}}}}}}},{{\"key\":{{\"type\":\"social\",\"path\":\"{}/post/main\",\"blockHeight\":{}}},\"value\":{{\"type\":\"repost\"}}}}]", post_initiator, block_height, post_initiator, block_height);
        let notify = format!("{{\"key\":\"{}\",\"value\":{{\"type\":\"repost\",\"item\":{{\"type\":\"social\",\"path\":\"{}/post/main\",\"blockHeight\":{}}}}}}}", post_initiator, post_initiator, block_height);
        social_db_contract().with_unused_gas_weight(1).set(
            json!({ get_devhub_discussions_account(&handle): {
              "index": {
                "repost": repost,
                "notify": notify
              }
            } }),
        )
    }

    pub fn delete_community(&mut self, handle: CommunityHandle) -> Promise {
        require!(
            self.has_moderator(env::predecessor_account_id()),
            "Only moderators can delete community"
        );

        let community = self
            .get_community(handle.clone())
            .expect(&format!("Community with handle `{}` does not exist", handle));

        self.communities.remove(&community.handle);

        require!(env::prepaid_gas() >= DELETE_COMMUNITY_GAS, "Require at least 30 Tgas");
        ext_devhub_community::ext(get_devhub_community_account(&community.handle).parse().unwrap())
            .with_unused_gas_weight(1)
            .destroy()
    }

    pub fn set_featured_communities(&mut self, handles: Vec<CommunityHandle>) {
        require!(
            self.has_moderator(env::predecessor_account_id()),
            "Only moderators can add featured communities"
        );

        // Check if every handle corresponds to an existing community
        for handle in &handles {
            require!(self.communities.get(&handle).is_some(), "Community does not exist.");
        }

        // Replace the existing featured communities with the new ones
        self.featured_communities =
            handles.into_iter().map(|handle| FeaturedCommunity { handle }).collect();
    }

    pub fn get_featured_communities(&self) -> Vec<Community> {
        self.featured_communities
            .iter()
            .filter_map(|fc| self.get_community(fc.handle.clone()))
            .collect()
    }

    fn get_moderators(&self) -> HashSet<AccountId> {
        let mut moderators: HashSet<AccountId> = HashSet::new();
        for m in self.access_control.members_list.get_moderators() {
            if let Member::Account(account_id) = m {
                moderators.insert(account_id.clone());
            }
        }
        moderators.insert(env::current_account_id());
        moderators
    }

    pub fn has_moderator(&self, account_id: AccountId) -> bool {
        let moderators = self.access_control.members_list.get_moderators();
        moderators.contains(&Member::Account(account_id))
    }

    pub fn web4_get(&self, request: Web4Request) -> Web4Response {
        web4::handler::web4_get(self, request)
    }

    pub fn set_social_db_profile_description(&self, description: String) -> Promise {
        let editor = env::predecessor_account_id();
        require!(
            editor == env::current_account_id() || self.has_moderator(editor),
            "Permission denied"
        );
        social_db_contract()
            .with_static_gas(env::prepaid_gas().saturating_div(3))
            .with_attached_deposit(env::attached_deposit())
            .set(json!({
                env::current_account_id(): {
                    "profile": {
                        "description": description
                    }
                }
            }))
    }
}

#[near]
pub struct LabelInfo {
    title: Option<String>,
    color: Option<(u8, u8, u8)>,
}

#[near(serializers=[borsh, json])]
pub struct LabelInfoExtended {
    value: String,
    title: Option<String>,
    color: Option<(u8, u8, u8)>,
}

#[derive(Copy, Clone, Serialize, Deserialize, NearSchema)]
#[serde(crate = "near_sdk::serde")]
pub struct BlockHeightCallbackRetValue {
    proposal_id: ProposalId,
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use crate::community::AddOn;

    use crate::{ProposalBodyV0, VersionedProposalBody};

    use near_sdk::test_utils::{get_created_receipts, VMContextBuilder};
    use near_sdk::{testing_env, VMContext};
    use serde_json::json;
    use std::collections::HashSet;
    use std::convert::TryInto;

    use super::Contract;

    fn get_context(is_view: bool) -> VMContext {
        get_context_with_signer(is_view, "bob.near".to_string())
    }

    fn get_context_with_signer(is_view: bool, signer: String) -> VMContext {
        VMContextBuilder::new()
            .signer_account_id(signer.clone().try_into().unwrap())
            .current_account_id(signer.try_into().unwrap())
            .is_view(is_view)
            .build()
    }

    fn get_context_with_current(is_view: bool, signer: String) -> VMContext {
        VMContextBuilder::new()
            .current_account_id(signer.try_into().unwrap())
            .is_view(is_view)
            .build()
    }

    #[allow(dead_code)]
    fn get_context_with_predecessor(is_view: bool, signer: String) -> VMContext {
        VMContextBuilder::new()
            .predecessor_account_id(signer.try_into().unwrap())
            .is_view(is_view)
            .build()
    }

    #[test]
    pub fn test_add_proposal() {
        let context = get_context(false);
        testing_env!(context);
        let mut contract = Contract::new();

        let body: ProposalBodyV0 = near_sdk::serde_json::from_value(json!({
            "proposal_body_version": "V0",
            "name": "another post",
            "description": "Hello to @petersalomonsen.near and @psalomo.near. This is an idea with mentions.",            "category": "Marketing",
            "summary": "sum",
            "linked_proposals": [1, 3],
            "requested_sponsorship_usd_amount": "1000000000",
            "requested_sponsorship_paid_in_currency": "USDT",
            "receiver_account": "polyprogrammist.near",
            "supervisor": "frol.near",
            "requested_sponsor": "neardevdao.near",
            "payouts": [],
            "timeline": {"status": "DRAFT"}
        })).unwrap();
        contract.add_proposal(VersionedProposalBody::V0(body), HashSet::new(), Some(0));
        let receipts = get_created_receipts();
        assert_eq!(3, receipts.len());

        if let near_sdk::mock::MockAction::FunctionCallWeight { method_name, args, .. } =
            &receipts[2].actions[0]
        {
            assert_eq!(method_name, b"set");
            assert_eq!(args, b"{\"data\":{\"bob.near\":{\"index\":{\"notify\":\"[{\\\"key\\\":\\\"petersalomonsen.near\\\",\\\"value\\\":{\\\"type\\\":\\\"proposal/mention\\\",\\\"proposal\\\":0,\\\"widgetAccountId\\\":\\\"bob.near\\\",\\\"notifier\\\":\\\"bob.near\\\"}},{\\\"key\\\":\\\"psalomo.near.\\\",\\\"value\\\":{\\\"type\\\":\\\"proposal/mention\\\",\\\"proposal\\\":0,\\\"widgetAccountId\\\":\\\"bob.near\\\",\\\"notifier\\\":\\\"bob.near\\\"}},{\\\"key\\\":\\\"frol.near\\\",\\\"value\\\":{\\\"type\\\":\\\"proposal/mention\\\",\\\"proposal\\\":0,\\\"widgetAccountId\\\":\\\"bob.near\\\",\\\"notifier\\\":\\\"bob.near\\\"}},{\\\"key\\\":\\\"neardevdao.near\\\",\\\"value\\\":{\\\"type\\\":\\\"proposal/mention\\\",\\\"proposal\\\":0,\\\"widgetAccountId\\\":\\\"bob.near\\\",\\\"notifier\\\":\\\"bob.near\\\"}}]\"}}}}");
        } else {
            assert!(false, "Expected a function call ...")
        }
    }

    #[test]
    pub fn test_create_addon() {
        let context = get_context_with_current(false, "bob.near".to_string());
        testing_env!(context);

        let mut contract = Contract::new();
        let input = fake_addon("CommunityAddOnId".to_string());
        contract.create_addon(input.to_owned());

        let addon = contract.get_addon("CommunityAddOnId".to_owned());

        assert_eq!(addon, Some(input))
    }

    pub fn fake_addon(id: String) -> AddOn {
        let input = AddOn {
            id: id.to_owned(),
            title: "GitHub AddOn".to_owned(),
            description: "Current status of NEARCORE repo".to_owned(),
            view_widget: "custom-viewer-widget".to_owned(),
            configurator_widget: "github-configurator".to_owned(),
            icon: "bi bi-github".to_owned(),
        };
        return input;
    }

    #[test]
    pub fn test_get_all_addons() {
        let context = get_context_with_current(false, "bob.near".to_string());
        testing_env!(context);
        let mut contract = Contract::new();
        let input = fake_addon("CommunityAddOnId".to_string());
        contract.create_addon(input.to_owned());

        let addons = contract.get_all_addons();

        assert_eq!(addons[0], input)
    }

    #[test]
    pub fn test_get_addon() {
        let context = get_context_with_current(false, "bob.near".to_string());
        testing_env!(context);
        let mut contract = Contract::new();
        let input = fake_addon("CommunityAddOnId".to_string());
        contract.create_addon(input.to_owned());

        let addon = contract.get_addon("CommunityAddOnId".to_owned());

        assert_eq!(addon, Some(input))
    }

    #[test]
    pub fn test_update_addon() {
        let context = get_context(false);
        testing_env!(context);
        let mut contract = Contract::new();
        let input = fake_addon("test".to_owned());
        contract.create_addon(input.to_owned());

        contract.update_addon(AddOn { title: "Telegram AddOn".to_owned(), ..input });

        let addons = contract.get_all_addons();

        assert_eq!(addons[0].title, "Telegram AddOn".to_owned());
    }
}
