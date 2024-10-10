use near_sdk::NearToken;
use {crate::test_env::*, serde_json::json};

#[tokio::test]
async fn test_deploy_contract_self_upgrade() -> anyhow::Result<()> {
    // Test Flow:
    // 1. Deploy devhub and near social contract on sandbox
    // 2. Add all kinds of posts and add a community.
    // 3. Upgrade the contract.
    // 4. Get all the posts and community and check if migration was successful.

    // Initialize the devhub and near social contract on chain,
    // contract is devhub contract instance.
    let contract = init_contracts_from_mainnet().await?;

    let deposit_amount = NearToken::from_millinear(10000);

    // Add a community
    let create_community = contract
        .call("create_community")
        .args_json(json!({
            "inputs": {
                "handle": "gotham",
                "name": "Gotham",
                "tag": "some",
                "description": "This is a test community.",
                "bio_markdown": "This is a sample text about your community.\nYou can change it on the community configuration page.",
                "logo_url": "https://ipfs.near.social/ipfs/bafkreibysr2mkwhb4j36h2t7mqwhynqdy4vzjfygfkfg65kuspd2bawauu",
                "banner_url": "https://ipfs.near.social/ipfs/bafkreic4xgorjt6ha5z4s5e3hscjqrowe5ahd7hlfc5p4hb6kdfp6prgy4"
            }
        }))
        .deposit(NearToken::from_near(4))
        .max_gas()
        .transact()
        .await?;

    assert!(create_community.is_success());

    let _add_proposal = contract
        .call("add_proposal")
        .args_json(json!({
            "body": {
                "proposal_body_version": "V0",
                "name": "another post",
                "description": "some description",
                "category": "Marketing",
                "summary": "sum",
                "linked_proposals": [1, 3],
                "requested_sponsorship_usd_amount": "1000000000",
                "requested_sponsorship_paid_in_currency": "USDT",
                "receiver_account": "polyprogrammist.near",
                "supervisor": "frol.near",
                "requested_sponsor": "neardevdao.near",
                "timeline": {"status": "DRAFT"}
            },
            "labels": ["test1", "test2"],
            "accepted_terms_and_conditions_version": 0,
        }))
        .max_gas()
        .deposit(deposit_amount)
        .transact()
        .await?;

    assert!(_add_proposal.is_success());

    let _edit_proposal_timeline_review = contract
        .call("edit_proposal_timeline")
        .args_json(json!({
            "id": 0,
            "timeline": {"status": "REVIEW", "kyc_verified": false, "sponsor_requested_review": true, "reviewer_completed_attestation": false }
        }))
        .max_gas()
        .deposit(deposit_amount)
        .transact()
        .await?;

    assert!(_edit_proposal_timeline_review.is_success());

    // Call self upgrade with current branch code
    let mut contract_upgrade_result = contract
        .call("unsafe_self_upgrade")
        .args(crate::test_env::DEVHUB_CONTRACT_WASM.clone())
        .max_gas()
        .transact()
        .await?;

    while contract_upgrade_result.json::<String>()? == "needs-migration" {
        contract_upgrade_result =
            contract.call("unsafe_migrate").args_json(json!({})).max_gas().transact().await?;
    }

    let get_community: serde_json::Value = contract
        .call("get_community")
        .args_json(json!({
            "handle" : "gotham"
        }))
        .view()
        .await?
        .json()?;

    insta::assert_json_snapshot!(get_community);

    let get_proposal: serde_json::Value = contract
        .call("get_proposal")
        .args_json(json!({
            "proposal_id" : 0
        }))
        .view()
        .await?
        .json()?;

    insta::assert_json_snapshot!(get_proposal, {".snapshot.timestamp" => "[timestamp]", ".social_db_post_block_height" => "91", ".snapshot_history[0].timestamp" => "[timestamp]"});

    let _set_global_labels = contract
        .call("set_global_labels")
        .args_json(json!({
            "labels": [
                {
                    "value": "test1",
                    "title": "test1 description",
                    "color": [255, 0, 0]
                },
                {
                    "value": "test2",
                    "title": "test2 description",
                    "color": [0, 255, 0]
                },
                {
                    "value": "test3",
                    "title": "test3 description",
                    "color": [0, 0, 255]
                }
            ]
        }))
        .max_gas()
        .deposit(deposit_amount)
        .transact()
        .await?;

    let _add_rfp = contract
        .call("add_rfp")
        .args_json(json!({
            "body": {
                "rfp_body_version": "V0",
                "name": "Some RFP",
                "description": "some description",
                "summary": "sum",
                "timeline": {"status": "ACCEPTING_SUBMISSIONS"},
                "submission_deadline": "1707821848175250170"
            },
            "labels": ["test1", "test2"],
        }))
        .max_gas()
        .deposit(deposit_amount)
        .transact()
        .await?;

    println!("_add_rfp: {:?}", _add_rfp);

    let _add_proposal = contract
        .call("add_proposal")
        .args_json(json!({
            "body": {
                "proposal_body_version": "V0",
                "name": "another post",
                "description": "some description",
                "category": "Marketing",
                "summary": "sum",
                "linked_proposals": [1, 3],
                "requested_sponsorship_usd_amount": "1000000000",
                "requested_sponsorship_paid_in_currency": "USDT",
                "receiver_account": "polyprogrammist.near",
                "supervisor": "frol.near",
                "requested_sponsor": "neardevdao.near",
                "timeline": {"status": "DRAFT"}
            },
            "labels": ["test1", "test2"],
            "accepted_terms_and_conditions_version": 0
        }))
        .max_gas()
        .deposit(deposit_amount)
        .transact()
        .await?;

    assert!(_add_rfp.is_success());
    assert!(_add_proposal.is_success());

    Ok(())
}
