use std::collections::HashSet;

use crate::{
    proposal::get_subscribers, rfp::get_subscribers as get_rfp_subscribers, Proposal, ProposalId,
    RFP,
};
use devhub_common::social_db_contract;
use near_sdk::serde_json::json;
use near_sdk::{env, AccountId, Promise};

pub fn get_text_mentions(text: &str) -> Vec<String> {
    let mut mentions = Vec::new();
    let mut mention = String::new();
    let mut recording = false;

    for ch in text.chars() {
        if recording {
            if ch.is_alphanumeric() || ch == '.' {
                mention.push(ch);
            } else {
                if !mention.is_empty() {
                    mentions.push(mention.clone());
                    mention.clear();
                }
                recording = false;
            }
        }

        if ch == '@' {
            recording = true;
        }
    }

    // Push the last mention if it wasn't pushed yet
    if recording && !mention.is_empty() {
        mentions.push(mention);
    }

    mentions
}

pub fn notify_accounts(
    notifier: AccountId,
    accounts: Vec<String>,
    notify_value: serde_json::Value,
) -> Promise {
    if !accounts.is_empty() {
        let mut notify_values = Vec::new();

        for account in accounts {
            notify_values.push(json!({
                "key": account,
                "value": notify_value,
            }));
        }

        social_db_contract()
            .with_static_gas(env::prepaid_gas().saturating_div(4))
            .with_attached_deposit(env::attached_deposit())
            .set(json!({
                notifier : {
                    "index": {
                        "notify": json!(notify_values).to_string()
                    }
                }
            }))
    } else {
        Promise::new(env::current_account_id())
    }
}

pub fn notify_proposal_subscribers(proposal: &Proposal) -> Promise {
    let accounts = get_subscribers(&proposal.snapshot.body.clone().latest_version());

    notify_accounts(
        env::current_account_id(),
        accounts,
        json!({
            "type": "proposal/mention",
            "proposal": proposal.id,
            "widgetAccountId": env::current_account_id(),
            "notifier": env::predecessor_account_id(),
        }),
    )
}

pub fn notify_rfp_subscribers(rfp: &RFP, additional_accounts: HashSet<AccountId>) -> Promise {
    let accounts = [
        get_rfp_subscribers(&rfp.snapshot.body.clone().latest_version()),
        additional_accounts.iter().map(|x| x.to_string()).collect::<Vec<_>>(),
    ]
    .concat();

    notify_accounts(
        env::current_account_id(),
        accounts,
        json!({
            "type": "rfp/mention",
            "rfp": rfp.id,
            "widgetAccountId": env::current_account_id(),
            "notifier": env::current_account_id(),
        }),
    )
}

pub fn notify_edit_proposal(proposal_id: ProposalId, post_author: AccountId) -> Promise {
    notify(
        env::current_account_id(),
        post_author,
        json!({
            "type": "proposal/edit",
            "proposal": proposal_id,
            "widgetAccountId": env::current_account_id(),
            "notifier": env::predecessor_account_id(),
        }),
    )
}

fn notify(notifier: AccountId, post_author: AccountId, notify_value: serde_json::Value) -> Promise {
    social_db_contract()
        .with_static_gas(env::prepaid_gas().saturating_div(4))
        .with_attached_deposit(env::attached_deposit())
        .set(json!({
            notifier : {
                "index": {
                    "notify": json!({
                        "key": post_author,
                        "value": notify_value,
                    }).to_string()
                }
            }
        }))
}
