//! Public methods of data model/state migrations between the versions.
//! Should be invocable only by the owner and in most cases should be called only once though the
//! latter is not asserted.

use crate::Contract;
use crate::*;
use near_sdk::store::Lazy;
use near_sdk::{borsh::to_vec, env, near, NearToken, Promise};
use std::collections::{HashMap, HashSet};

#[near]
#[derive(PanicOnDefault)]
pub struct ContractV1 {
    pub posts: Vector<VersionedPost>,
    pub post_to_parent: LookupMap<PostId, PostId>,
    pub post_to_children: LookupMap<PostId, Vec<PostId>>,
    pub label_to_posts: UnorderedMap<String, HashSet<PostId>>,
}

// From ContractV1 to ContractV2
impl Contract {
    fn unsafe_add_acl() {
        let ContractV1 { posts, post_to_parent, post_to_children, label_to_posts } =
            env::state_read().unwrap();
        env::state_write(&ContractV2 {
            posts,
            post_to_parent,
            post_to_children,
            label_to_posts,
            access_control: Default::default(),
        });
    }
}

// // Fake vector purely for the sake of overriding initialization.
// #[derive(BorshSerialize, BorshDeserialize)]
// pub struct FakeVector {
//     len: u64,
//     prefix: Vec<u8>,
// }
//
// impl FakeVector {
//     pub fn new<S>(len: u64, prefix: S) -> Self
//     where
//         S: IntoStorageKey,
//     {
//         Self { len, prefix: prefix.into_storage_key() }
//     }
// }

#[near]
#[derive(PanicOnDefault)]
pub struct ContractV2 {
    pub posts: Vector<VersionedPost>,
    pub post_to_parent: LookupMap<PostId, PostId>,
    pub post_to_children: LookupMap<PostId, Vec<PostId>>,
    pub label_to_posts: UnorderedMap<String, HashSet<PostId>>,
    pub access_control: AccessControl,
}

// From ContractV2 to ContractV3
impl Contract {
    fn unsafe_add_post_authors() {
        let ContractV2 { posts, post_to_parent, post_to_children, label_to_posts, access_control } =
            env::state_read().unwrap();
        let authors = UnorderedMap::new(StorageKey::AuthorToAuthorPosts);

        env::state_write(&ContractV3 {
            posts,
            post_to_parent,
            post_to_children,
            label_to_posts,
            access_control,
            authors,
        });
    }
}

#[near]
#[derive(PanicOnDefault)]
pub struct ContractV3 {
    pub posts: Vector<VersionedPost>,
    pub post_to_parent: LookupMap<PostId, PostId>,
    pub post_to_children: LookupMap<PostId, Vec<PostId>>,
    pub label_to_posts: UnorderedMap<String, HashSet<PostId>>,
    pub access_control: AccessControl,
    pub authors: UnorderedMap<AccountId, HashSet<PostId>>,
}

// From ContractV3 to ContractV4
impl Contract {
    fn unsafe_add_communities() {
        let ContractV3 {
            posts,
            post_to_parent,
            post_to_children,
            label_to_posts,
            access_control,
            authors,
        } = env::state_read().unwrap();
        env::state_write(&ContractV4 {
            posts,
            post_to_parent,
            post_to_children,
            label_to_posts,
            access_control,
            authors,
            communities: UnorderedMap::new(StorageKey::Communities),
        });
    }
}

#[near]
#[derive(PanicOnDefault)]
pub struct ContractV4 {
    pub posts: Vector<VersionedPost>,
    pub post_to_parent: LookupMap<PostId, PostId>,
    pub post_to_children: LookupMap<PostId, Vec<PostId>>,
    pub label_to_posts: UnorderedMap<String, HashSet<PostId>>,
    pub access_control: AccessControl,
    pub authors: UnorderedMap<AccountId, HashSet<PostId>>,
    pub communities: UnorderedMap<String, CommunityV1>,
}

// From ContractV4 to ContractV5
impl Contract {
    fn unsafe_add_featured_communities() {
        let ContractV4 {
            posts,
            post_to_parent,
            post_to_children,
            label_to_posts,
            access_control,
            authors,
            communities,
        } = env::state_read().unwrap();
        env::state_write(&ContractV5 {
            posts,
            post_to_parent,
            post_to_children,
            label_to_posts,
            access_control,
            authors,
            communities,
            featured_communities: Vec::new(),
        });
    }
}

#[near]
#[derive(Clone)]
pub struct CommunityV1 {
    pub handle: CommunityHandle,
    pub admins: Vec<AccountId>,
    pub name: String,
    pub description: String,
    pub bio_markdown: Option<String>,
    pub logo_url: String,
    pub banner_url: String,
    pub tag: String,
    pub github_handle: Option<String>,
    pub telegram_handle: Option<String>,
    pub twitter_handle: Option<String>,
    pub website_url: Option<String>,
    /// JSON string of github board configuration
    pub github: Option<String>,
    pub sponsorship: Option<bool>,
    pub wiki1: Option<WikiPage>,
    pub wiki2: Option<WikiPage>,
}

#[near]
#[derive(PanicOnDefault)]
pub struct ContractV5 {
    pub posts: Vector<VersionedPost>,
    pub post_to_parent: LookupMap<PostId, PostId>,
    pub post_to_children: LookupMap<PostId, Vec<PostId>>,
    pub label_to_posts: UnorderedMap<String, HashSet<PostId>>,
    pub access_control: AccessControl,
    pub authors: UnorderedMap<AccountId, HashSet<PostId>>,
    pub communities: UnorderedMap<String, CommunityV1>,
    pub featured_communities: Vec<FeaturedCommunity>,
}

// From ContractV5 to ContractV6
impl Contract {
    fn unsafe_multiple_telegrams() {
        let ContractV5 {
            posts,
            post_to_parent,
            post_to_children,
            label_to_posts,
            access_control,
            authors,
            mut communities,
            featured_communities,
        } = env::state_read().unwrap();

        let migrated_communities: Vec<(String, CommunityV2)> = communities
            .iter()
            .map(|(community_handle, community)| {
                (
                    community_handle,
                    CommunityV2 {
                        handle: community.handle,
                        admins: community.admins,
                        name: community.name,
                        description: community.description,
                        bio_markdown: community.bio_markdown,
                        logo_url: community.logo_url,
                        banner_url: community.banner_url,
                        tag: community.tag,
                        github_handle: community.github_handle,
                        telegram_handle: community.telegram_handle.iter().cloned().collect(),
                        twitter_handle: community.twitter_handle,
                        website_url: community.website_url,
                        github: community.github,
                        sponsorship: community.sponsorship,
                        wiki1: community.wiki1,
                        wiki2: community.wiki2,
                    },
                )
            })
            .collect();

        communities.clear();

        let mut communities_new = UnorderedMap::new(StorageKey::Communities);

        for (k, v) in migrated_communities {
            communities_new.insert(&k, &v);
        }

        env::state_write(&ContractV6 {
            posts,
            post_to_parent,
            post_to_children,
            label_to_posts,
            access_control,
            authors,
            communities: communities_new,
            featured_communities,
        });
    }
}

#[near]
#[derive(Clone)]
pub struct CommunityV2 {
    pub handle: CommunityHandle,
    pub admins: Vec<AccountId>,
    pub name: String,
    pub description: String,
    pub bio_markdown: Option<String>,
    pub logo_url: String,
    pub banner_url: String,
    pub tag: String,
    pub github_handle: Option<String>,
    pub telegram_handle: Vec<String>,
    pub twitter_handle: Option<String>,
    pub website_url: Option<String>,
    /// JSON string of github board configuration
    pub github: Option<String>,
    pub sponsorship: Option<bool>,
    pub wiki1: Option<WikiPage>,
    pub wiki2: Option<WikiPage>,
}

#[near]
#[derive(PanicOnDefault)]
pub struct ContractV6 {
    pub posts: Vector<VersionedPost>,
    pub post_to_parent: LookupMap<PostId, PostId>,
    pub post_to_children: LookupMap<PostId, Vec<PostId>>,
    pub label_to_posts: UnorderedMap<String, HashSet<PostId>>,
    pub access_control: AccessControl,
    pub authors: UnorderedMap<AccountId, HashSet<PostId>>,
    pub communities: UnorderedMap<String, CommunityV2>,
    pub featured_communities: Vec<FeaturedCommunity>,
}

// From ContractV6 to ContractV7
impl Contract {
    fn unsafe_add_board_and_feature_flags() {
        let ContractV6 {
            posts,
            post_to_parent,
            post_to_children,
            label_to_posts,
            access_control,
            authors,
            mut communities,
            featured_communities,
        } = env::state_read().unwrap();

        let migrated_communities: Vec<(String, CommunityV3)> = communities
            .iter()
            .map(|(community_handle, community)| {
                (
                    community_handle,
                    CommunityV3 {
                        admins: community.admins,
                        handle: community.handle,
                        name: community.name,
                        tag: community.tag,
                        description: community.description,
                        logo_url: community.logo_url,
                        banner_url: community.banner_url,
                        bio_markdown: community.bio_markdown,
                        github_handle: community.github_handle,
                        telegram_handle: community.telegram_handle,
                        twitter_handle: community.twitter_handle,
                        website_url: community.website_url,
                        github: community.github,
                        board: None,
                        wiki1: community.wiki1,
                        wiki2: community.wiki2,

                        features: CommunityFeatureFlags {
                            telegram: true,
                            github: true,
                            board: true,
                            wiki: true,
                        },
                    },
                )
            })
            .collect();

        communities.clear();

        let mut communities_new = UnorderedMap::new(StorageKey::Communities);

        for (k, v) in migrated_communities {
            communities_new.insert(&k, &v);
        }

        env::state_write(&ContractV7 {
            posts,
            post_to_parent,
            post_to_children,
            label_to_posts,
            access_control,
            authors,
            communities: communities_new,
            featured_communities,
        });
    }
}

#[near]
#[derive(Clone)]
pub struct CommunityV3 {
    pub admins: Vec<AccountId>,
    pub handle: CommunityHandle,
    pub name: String,
    pub tag: String,
    pub description: String,
    pub logo_url: String,
    pub banner_url: String,
    pub bio_markdown: Option<String>,
    pub github_handle: Option<String>,
    pub telegram_handle: Vec<String>,
    pub twitter_handle: Option<String>,
    pub website_url: Option<String>,
    /// JSON string of github board configuration
    pub github: Option<String>,
    /// JSON string of kanban board configuration
    pub board: Option<String>,
    pub wiki1: Option<WikiPage>,
    pub wiki2: Option<WikiPage>,
    pub features: CommunityFeatureFlags,
}

#[near]
#[derive(PanicOnDefault)]
pub struct ContractV7 {
    pub posts: Vector<VersionedPost>,
    pub post_to_parent: LookupMap<PostId, PostId>,
    pub post_to_children: LookupMap<PostId, Vec<PostId>>,
    pub label_to_posts: UnorderedMap<String, HashSet<PostId>>,
    pub access_control: AccessControl,
    pub authors: UnorderedMap<AccountId, HashSet<PostId>>,
    pub communities: UnorderedMap<String, CommunityV3>,
    pub featured_communities: Vec<FeaturedCommunity>,
}

// From ContractV7 to ContractV8
impl Contract {
    fn unsafe_add_community_addons() {
        let ContractV7 {
            posts,
            post_to_parent,
            post_to_children,
            label_to_posts,
            access_control,
            authors,
            mut communities,
            featured_communities,
        } = env::state_read().unwrap();

        let migrated_communities: Vec<(String, CommunityV4)> = communities
            .iter()
            .map(|(community_handle, community)| {
                (
                    community_handle,
                    CommunityV4 {
                        admins: community.admins,
                        handle: community.handle,
                        name: community.name,
                        tag: community.tag,
                        description: community.description,
                        logo_url: community.logo_url,
                        banner_url: community.banner_url,
                        bio_markdown: community.bio_markdown,
                        github_handle: community.github_handle,
                        telegram_handle: community.telegram_handle,
                        twitter_handle: community.twitter_handle,
                        website_url: community.website_url,
                        github: community.github,
                        board: None,
                        wiki1: community.wiki1,
                        wiki2: community.wiki2,
                        features: community.features,
                        addons: Vec::new(),
                    },
                )
            })
            .collect();

        communities.clear();

        let mut communities_new = UnorderedMap::new(StorageKey::Communities);

        for (k, v) in migrated_communities {
            communities_new.insert(&k, &v);
        }

        env::state_write(&ContractV8 {
            posts,
            post_to_parent,
            post_to_children,
            label_to_posts,
            access_control,
            authors,
            communities: communities_new,
            featured_communities,
            available_addons: UnorderedMap::new(StorageKey::AddOns),
        });
    }
}

#[near]
pub struct CommunityV4 {
    pub admins: Vec<AccountId>,
    pub handle: CommunityHandle,
    pub name: String,
    pub tag: String,
    pub description: String,
    pub logo_url: String,
    pub banner_url: String,
    pub bio_markdown: Option<String>,
    pub github_handle: Option<String>,
    pub telegram_handle: Vec<String>,
    pub twitter_handle: Option<String>,
    pub website_url: Option<String>,
    /// JSON string of github board configuration
    pub github: Option<String>,
    /// JSON string of kanban board configuration
    pub board: Option<String>,
    pub wiki1: Option<WikiPage>,
    pub wiki2: Option<WikiPage>,
    pub features: CommunityFeatureFlags,
    pub addons: Vec<CommunityAddOn>,
}

#[near]
#[derive(PanicOnDefault)]
pub struct ContractV8 {
    pub posts: Vector<VersionedPost>,
    pub post_to_parent: LookupMap<PostId, PostId>,
    pub post_to_children: LookupMap<PostId, Vec<PostId>>,
    pub label_to_posts: UnorderedMap<String, HashSet<PostId>>,
    pub access_control: AccessControl,
    pub authors: UnorderedMap<AccountId, HashSet<PostId>>,
    pub communities: UnorderedMap<String, CommunityV4>,
    pub featured_communities: Vec<FeaturedCommunity>,
    pub available_addons: UnorderedMap<AddOnId, AddOn>,
}

// From ContractV8 to ContractV9
impl Contract {
    fn unsafe_clean_up_community() {
        let ContractV8 {
            posts,
            post_to_parent,
            post_to_children,
            label_to_posts,
            access_control,
            authors,
            mut communities,
            featured_communities,
            available_addons,
        } = env::state_read().unwrap();
        let migrated_communities: Vec<(String, CommunityV5)> = communities
            .iter()
            .map(|(community_handle, community)| {
                (
                    community_handle,
                    CommunityV5 {
                        admins: community.admins,
                        handle: community.handle,
                        name: community.name,
                        tag: community.tag,
                        description: community.description,
                        logo_url: community.logo_url,
                        banner_url: community.banner_url,
                        bio_markdown: community.bio_markdown,
                        github_handle: community.github_handle,
                        telegram_handle: community.telegram_handle.first().cloned(),
                        twitter_handle: community.twitter_handle,
                        website_url: community.website_url,
                        addons: community.addons,
                    },
                )
            })
            .collect();

        communities.clear();

        let mut communities_new = UnorderedMap::new(StorageKey::Communities);

        for (k, v) in migrated_communities {
            communities_new.insert(&k, &v);
        }

        env::state_write(&ContractV9 {
            posts,
            post_to_parent,
            post_to_children,
            label_to_posts,
            access_control,
            authors,
            communities: communities_new,
            featured_communities,
            available_addons,
        });
    }
}

#[near]
pub struct CommunityV5 {
    pub admins: Vec<AccountId>,
    pub handle: CommunityHandle,
    pub name: String,
    pub tag: String,
    pub description: String,
    pub logo_url: String,
    pub banner_url: String,
    pub bio_markdown: Option<String>,
    pub github_handle: Option<String>,
    pub telegram_handle: Option<String>,
    pub twitter_handle: Option<String>,
    pub website_url: Option<String>,
    pub addons: Vec<CommunityAddOn>,
}

#[near]
#[derive(PanicOnDefault)]
pub struct ContractV9 {
    pub posts: Vector<VersionedPost>,
    pub post_to_parent: LookupMap<PostId, PostId>,
    pub post_to_children: LookupMap<PostId, Vec<PostId>>,
    pub label_to_posts: UnorderedMap<String, HashSet<PostId>>,
    pub access_control: AccessControl,
    pub authors: UnorderedMap<AccountId, HashSet<PostId>>,
    pub communities: UnorderedMap<String, CommunityV5>,
    pub featured_communities: Vec<FeaturedCommunity>,
    pub available_addons: UnorderedMap<AddOnId, AddOn>,
}

// From ContractV9 to ContractV10
impl Contract {
    fn unsafe_add_proposals() {
        let ContractV9 {
            posts,
            post_to_parent,
            post_to_children,
            label_to_posts,
            access_control,
            authors,
            communities,
            featured_communities,
            available_addons,
        } = env::state_read().unwrap();

        env::state_write(&ContractV10 {
            posts,
            post_to_parent,
            post_to_children,
            label_to_posts,
            access_control,
            authors,
            proposals: Vector::new(StorageKey::Proposals),
            label_to_proposals: UnorderedMap::new(StorageKey::LabelToProposals),
            author_proposals: UnorderedMap::new(StorageKey::AuthorProposals),
            proposal_categories: default_categories(),
            communities,
            featured_communities,
            available_addons,
        });
    }
}

#[near]
#[derive(PanicOnDefault)]
pub struct ContractV10 {
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
    pub communities: UnorderedMap<CommunityHandle, CommunityV5>,
    pub featured_communities: Vec<FeaturedCommunity>,
    pub available_addons: UnorderedMap<AddOnId, AddOn>,
}

// From ContractV10 to ContractV11
impl Contract {
    fn unsafe_add_rfp() {
        let ContractV10 {
            posts,
            post_to_parent,
            post_to_children,
            label_to_posts,
            access_control,
            authors,
            proposals,
            label_to_proposals,
            author_proposals,
            proposal_categories,
            communities,
            featured_communities,
            available_addons,
        } = env::state_read().unwrap();

        env::state_write(&ContractV11 {
            posts,
            post_to_parent,
            post_to_children,
            label_to_posts,
            access_control,
            authors,
            proposals,
            label_to_proposals,
            author_proposals,
            proposal_categories,
            rfps: Vector::new(StorageKey::RFPs),
            label_to_rfps: UnorderedMap::new(StorageKey::LabelToRFPs),
            global_labels_info: Lazy::new(StorageKey::LabelInfo, HashMap::new()),
            communities,
            featured_communities,
            available_addons,
        });
    }
}

#[near]
#[derive(PanicOnDefault)]
pub struct ContractV11 {
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
    pub communities: UnorderedMap<CommunityHandle, CommunityV5>,
    pub featured_communities: Vec<FeaturedCommunity>,
    pub available_addons: UnorderedMap<AddOnId, AddOn>,
}

#[near]
#[derive(Debug)]
pub(crate) enum StateVersion {
    V1,
    V2,
    V3 { done: bool, migrated_count: u64 },
    V4,
    V5,
    V6,
    V7,
    V8,
    V9,
    V10,
    V11,
}

const VERSION_KEY: &[u8] = b"VERSION";

fn state_version_read() -> StateVersion {
    env::storage_read(VERSION_KEY)
        .map(|data| {
            StateVersion::try_from_slice(&data).expect("Cannot deserialize the contract state.")
        })
        .unwrap_or(StateVersion::V2) // StateVersion is introduced in production contract with V2 State.
}

pub(crate) fn state_version_write(version: &StateVersion) {
    let data = to_vec(&version).expect("Cannot serialize the contract state.");
    env::storage_write(VERSION_KEY, &data);
    near_sdk::log!("Migrated to version: {:?}", version);
}

#[near]
impl Contract {
    pub fn unsafe_self_upgrade() {
        near_sdk::assert_self();

        let contract = env::input().expect("No contract code is attached in input");
        Promise::new(env::current_account_id())
            .deploy_contract(contract)
            .then(Promise::new(env::current_account_id()).function_call(
                "unsafe_migrate".to_string(),
                Vec::new(),
                NearToken::from_near(0),
                env::prepaid_gas().saturating_sub(near_sdk::Gas::from_tgas(100)),
            ))
            .as_return();
    }

    fn migration_done() {
        near_sdk::log!("Migration done.");
        env::value_return(b"\"done\"");
    }

    fn needs_migration() {
        env::value_return(b"\"needs-migration\"");
    }

    pub fn unsafe_migrate() {
        near_sdk::assert_self();
        let current_version = state_version_read();
        near_sdk::log!("Migrating from version: {:?}", current_version);
        match current_version {
            StateVersion::V1 => {
                Contract::unsafe_add_acl();
                state_version_write(&StateVersion::V2);
            }
            StateVersion::V2 => {
                Contract::unsafe_add_post_authors();
                state_version_write(&StateVersion::V3 { done: false, migrated_count: 0 })
            }
            StateVersion::V3 { done: false, .. } => {
                unimplemented!();
            }
            StateVersion::V3 { done: true, migrated_count: _ } => {
                Contract::unsafe_add_communities();
                state_version_write(&StateVersion::V4);
            }
            StateVersion::V4 => {
                Contract::unsafe_add_featured_communities();
                state_version_write(&StateVersion::V5);
            }
            StateVersion::V5 => {
                Contract::unsafe_multiple_telegrams();
                state_version_write(&StateVersion::V6);
            }
            StateVersion::V6 => {
                Contract::unsafe_add_board_and_feature_flags();
                state_version_write(&StateVersion::V7);
            }
            StateVersion::V7 => {
                Contract::unsafe_add_community_addons();
                state_version_write(&StateVersion::V8);
            }
            StateVersion::V8 => {
                Contract::unsafe_clean_up_community();
                state_version_write(&StateVersion::V9);
            }
            StateVersion::V9 => {
                Contract::unsafe_add_proposals();
                state_version_write(&StateVersion::V10);
            }
            StateVersion::V10 => {
                Contract::unsafe_add_rfp();
                state_version_write(&StateVersion::V11);
            }
            _ => {
                return Contract::migration_done();
            }
        }
        Contract::needs_migration();
    }
}
