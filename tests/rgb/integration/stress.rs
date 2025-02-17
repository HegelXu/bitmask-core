#![cfg(not(target_arch = "wasm32"))]
use std::{collections::HashMap, str::FromStr};

use anyhow::Ok;
use bitcoin::psbt::PartiallySignedTransaction;
use bitmask_core::{
    bitcoin::{get_wallet, save_mnemonic, sign_psbt, sync_wallet},
    rgb::{
        clear_stock, clear_watcher, constants::RGB_DEFAULT_NAME, create_watcher, list_contracts,
    },
    structs::{IssueResponse, SecretString, WatcherRequest},
};
use psbt::Psbt;

use crate::rgb::integration::utils::{
    create_new_psbt, import_new_contract, issuer_issue_contract, send_coins, ISSUER_MNEMONIC,
};

#[allow(unused_variables)]
#[tokio::test]
async fn allow_issue_x_fungibles_in_one_utxo() -> anyhow::Result<()> {
    let stress: bool = std::env::var("STRESS_TEST")
        .unwrap_or("false".to_string())
        .parse()?;
    let network = std::env::var("BITCOIN_NETWORK").unwrap_or("bitcoin".to_string());
    if !stress && network != "regtest" {
        return Ok(());
    }

    let max = 150;
    let supply = 5;
    let issuer_keys = save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let issuer_sk = issuer_keys.private.nostr_prv.clone();
    clear_stock(&issuer_sk).await;
    clear_watcher(&issuer_sk, "default").await?;

    let iface = "RGB20";
    let watcher_pub = issuer_keys.public.watcher_xpub.clone();
    send_coins(iface, &watcher_pub).await?;

    let mut contracts = HashMap::new();
    for i in 0..max {
        let issuer_resp = issuer_issue_contract(iface, supply, false, false, None).await;
        let issuer = issuer_resp?;
        contracts.insert(issuer.contract_id.clone(), issuer.supply);
        // println!(
        //     "Contract #{i} ({}) : {} {}",
        //     issuer.contract_id, issuer.supply, issuer.issue_utxo
        // );
    }

    let contract_resp = list_contracts(&issuer_sk).await?;
    assert_eq!(contracts.len(), contract_resp.contracts.len());

    for i in 0..max {
        let contract = &contract_resp.contracts[i];
        // println!(
        //     "Contract #{i} ({}) : {} ({})",
        //     contract.contract_id,
        //     contract.balance,
        //     contract.allocations.len()
        // );
        assert_eq!(
            contracts.get(&contract.contract_id.to_string()).unwrap(),
            &contract.balance
        );
    }
    Ok(())
}

#[allow(unused_variables)]
#[tokio::test]
async fn allow_issue_x_fungibles_generate_utxos() -> anyhow::Result<()> {
    let stress: bool = std::env::var("STRESS_TEST")
        .unwrap_or("false".to_string())
        .parse()?;
    let network = std::env::var("BITCOIN_NETWORK").unwrap_or("bitcoin".to_string());
    if !stress && network != "regtest" {
        return Ok(());
    }

    let max = 26;
    let supply = 5;
    let issuer_keys = save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let issuer_sk = issuer_keys.private.nostr_prv.clone();
    clear_stock(&issuer_sk).await;
    clear_watcher(&issuer_sk, "default").await?;

    let iface = "RGB20";
    let mut contracts = HashMap::new();
    for i in 0..max {
        let issuer_resp = issuer_issue_contract(iface, supply, false, true, None).await;
        let issuer = issuer_resp?;
        contracts.insert(issuer.contract_id.clone(), issuer.supply);
        // println!(
        //     "Contract #{i} ({}) : {} ({})",
        //     issuer.contract_id, issuer.supply, issuer.issue_utxo
        // );
    }

    let contract_resp = list_contracts(&issuer_sk).await?;
    assert_eq!(contracts.len(), contract_resp.contracts.len());

    for i in 0..max {
        let contract = &contract_resp.contracts[i];
        // println!(
        //     "Contract #{i} ({}) : {} ({})",
        //     contract.contract_id, contract.balance, contract.allocations[0].utxo
        // );
        assert_eq!(
            contracts.get(&contract.contract_id.to_string()).unwrap(),
            &contract.balance
        );
    }
    Ok(())
}

#[allow(unused_variables)]
#[tokio::test]
async fn allow_issue_x_fungibles_witn_spend_utxos() -> anyhow::Result<()> {
    let stress: bool = std::env::var("STRESS_TEST")
        .unwrap_or("false".to_string())
        .parse()?;
    let network = std::env::var("BITCOIN_NETWORK").unwrap_or("bitcoin".to_string());
    if !stress && network != "regtest" {
        return Ok(());
    }

    let max = 26;
    let supply = 5;
    let issuer_keys = &save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;

    let issuer_sk = issuer_keys.private.nostr_prv.clone();
    clear_stock(&issuer_sk).await;
    clear_watcher(&issuer_sk, "default").await?;

    let iface = "RGB20";

    let mut issuer = IssueResponse::default();
    let mut contracts = HashMap::new();
    for i in 0..(max / 2) {
        let issuer_resp = issuer_issue_contract(iface, supply, false, true, None).await;
        issuer = issuer_resp?;
        contracts.insert(issuer.contract_id.clone(), issuer.supply);
        // println!(
        //     "Contract #{i} ({}) : {} ({})",
        //     issuer.contract_id, issuer.supply, issuer.issue_utxo
        // );
    }
    let psbt_resp = create_new_psbt(
        &issuer.contract_id,
        &issuer.iface,
        vec![issuer.issue_utxo],
        issuer_keys.clone(),
    )
    .await?;
    let original_psbt = Psbt::from_str(&psbt_resp.psbt)?;
    let final_psbt = PartiallySignedTransaction::from(original_psbt);

    let issuer_wallet = get_wallet(
        &SecretString(issuer_keys.private.rgb_assets_descriptor_xprv.clone()),
        None,
    )
    .await?;
    sync_wallet(&issuer_wallet).await?;

    let sign = sign_psbt(&issuer_wallet, final_psbt).await;
    assert!(sign.is_ok());

    for i in (max / 2)..max {
        let issuer_resp = issuer_issue_contract(iface, supply, false, true, None).await;
        let issuer = issuer_resp?;
        contracts.insert(issuer.contract_id.clone(), issuer.supply);
        // println!(
        //     "Contract #{i} ({}) : {} ({})",
        //     issuer.contract_id, issuer.supply, issuer.issue_utxo
        // );
    }

    let contract_resp = list_contracts(&issuer_sk).await?;
    assert_eq!(contracts.len(), contract_resp.contracts.len());

    for i in 0..max {
        let contract = &contract_resp.contracts[i];
        // println!(
        //     "Contract #{i} ({}) : {} ({})",
        //     contract.contract_id, contract.balance, contract.allocations[0].utxo
        // );
        assert_eq!(
            contracts.get(&contract.contract_id.to_string()).unwrap(),
            &contract.balance
        );
    }
    Ok(())
}

#[allow(unused_variables)]
#[tokio::test]
async fn allow_import_fungible_before_create_watcher() -> anyhow::Result<()> {
    let stress: bool = std::env::var("STRESS_TEST")
        .unwrap_or("false".to_string())
        .parse()?;
    let network = std::env::var("BITCOIN_NETWORK").unwrap_or("bitcoin".to_string());
    if !stress && network != "regtest" {
        return Ok(());
    }

    let supply = 5;
    let issuer_keys = &save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let issuer_sk = issuer_keys.private.nostr_prv.clone();
    clear_stock(&issuer_sk).await;
    clear_watcher(&issuer_sk, "default").await?;

    let iface = "RGB20";
    let watcher_pub = issuer_keys.public.watcher_xpub.clone();
    send_coins(iface, &watcher_pub).await?;

    let issuer_resp = issuer_issue_contract(iface, supply, false, false, None).await;
    let issuer = issuer_resp?;

    // Clean Watcher
    clear_watcher(&issuer_sk, "default").await?;
    let contract_resp = list_contracts(&issuer_sk).await?;

    for contract in contract_resp.contracts {
        assert_eq!(0, contract.balance);
    }

    // Clean Stock
    clear_stock(&issuer_sk).await;

    // Import Contract
    let import_resp = import_new_contract(issuer).await;
    assert!(import_resp.is_ok());

    // Clean Watcher
    clear_watcher(&issuer_sk, "default").await?;
    let contract_resp = list_contracts(&issuer_sk).await?;
    for contract in contract_resp.contracts {
        assert_eq!(0, contract.balance);
    }

    // Re-create Watcher
    let create_watch_req = WatcherRequest {
        name: RGB_DEFAULT_NAME.to_string(),
        xpub: watcher_pub,
        force: false,
    };

    let resp = create_watcher(&issuer_sk, create_watch_req).await;
    assert!(resp.is_ok());

    let contract_resp = list_contracts(&issuer_sk).await?;
    for contract in contract_resp.contracts {
        assert_eq!(supply, contract.balance);
    }

    Ok(())
}

#[allow(unused_variables)]
#[tokio::test]
async fn allow_issue_x_uda_in_one_utxo() -> anyhow::Result<()> {
    let stress: bool = std::env::var("STRESS_TEST")
        .unwrap_or("false".to_string())
        .parse()?;
    let network = std::env::var("BITCOIN_NETWORK").unwrap_or("bitcoin".to_string());
    if !stress && network != "regtest" {
        return Ok(());
    }

    let max = 150;
    let supply = 1;
    let issuer_keys = &save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let issuer_sk = issuer_keys.private.nostr_prv.clone();
    clear_stock(&issuer_sk).await;
    clear_watcher(&issuer_sk, "default").await?;

    let iface = "RGB21";
    let watcher_pub = issuer_keys.public.watcher_xpub.clone();
    send_coins(iface, &watcher_pub).await?;

    let mut contracts = HashMap::new();
    for i in 0..max {
        let issuer_resp = issuer_issue_contract(iface, supply, false, false, None).await;
        let issuer = issuer_resp?;
        contracts.insert(issuer.contract_id.clone(), issuer.supply);
        // println!(
        //     "Contract #{i} ({}) : {} {}",
        //     issuer.contract_id, issuer.supply, issuer.issue_utxo
        // );
    }

    let contract_resp = list_contracts(&issuer_sk).await?;
    assert_eq!(contracts.len(), contract_resp.contracts.len());

    for i in 0..max {
        let contract = &contract_resp.contracts[i];
        // println!(
        //     "Contract #{i} ({}) : {} ({})",
        //     contract.contract_id,
        //     contract.balance,
        //     contract.allocations.len()
        // );
        assert_eq!(
            contracts.get(&contract.contract_id.to_string()).unwrap(),
            &contract.balance
        );
    }
    Ok(())
}

#[allow(unused_variables)]
#[tokio::test]
async fn allow_issue_x_uda_generate_utxos() -> anyhow::Result<()> {
    let stress: bool = std::env::var("STRESS_TEST")
        .unwrap_or("false".to_string())
        .parse()?;
    let network = std::env::var("BITCOIN_NETWORK").unwrap_or("bitcoin".to_string());
    if !stress && network != "regtest" {
        return Ok(());
    }

    let max = 26;
    let supply = 1;
    let issuer_keys = &save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let issuer_sk = issuer_keys.private.nostr_prv.clone();
    clear_stock(&issuer_sk).await;
    clear_watcher(&issuer_sk, "default").await?;

    let iface = "RGB21";
    let mut contracts = HashMap::new();
    for i in 0..max {
        let issuer_resp = issuer_issue_contract(iface, supply, false, true, None).await;
        let issuer = issuer_resp?;
        contracts.insert(issuer.contract_id.clone(), issuer.supply);
        // println!(
        //     "Contract #{i} ({}) : {} ({})",
        //     issuer.contract_id, issuer.supply, issuer.issue_utxo
        // );
    }

    let contract_resp = list_contracts(&issuer_sk).await?;
    assert_eq!(contracts.len(), contract_resp.contracts.len());

    for i in 0..max {
        let contract = &contract_resp.contracts[i];
        assert_eq!(
            contracts.get(&contract.contract_id.to_string()).unwrap(),
            &contract.balance
        );
    }
    Ok(())
}

#[allow(unused_variables)]
#[tokio::test]
async fn allow_issue_x_uda_witn_spend_utxos() -> anyhow::Result<()> {
    let stress: bool = std::env::var("STRESS_TEST")
        .unwrap_or("false".to_string())
        .parse()?;
    let network = std::env::var("BITCOIN_NETWORK").unwrap_or("bitcoin".to_string());
    if !stress && network != "regtest" {
        return Ok(());
    }

    let max = 26;
    let supply = 1;
    let issuer_keys = &save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let issuer_sk = issuer_keys.private.nostr_prv.clone();
    clear_stock(&issuer_sk).await;
    clear_watcher(&issuer_sk, "default").await?;

    let iface = "RGB21";

    let mut issuer = IssueResponse::default();
    let mut contracts = HashMap::new();
    for i in 0..(max / 2) {
        let issuer_resp = issuer_issue_contract(iface, supply, false, true, None).await;
        issuer = issuer_resp?;
        contracts.insert(issuer.contract_id.clone(), issuer.supply);
    }
    let psbt_resp = create_new_psbt(
        &issuer.contract_id,
        &issuer.iface,
        vec![issuer.issue_utxo],
        issuer_keys.clone(),
    )
    .await?;
    let original_psbt = Psbt::from_str(&psbt_resp.psbt)?;
    let final_psbt = PartiallySignedTransaction::from(original_psbt);

    let issuer_wallet = get_wallet(
        &SecretString(issuer_keys.private.rgb_assets_descriptor_xprv.clone()),
        None,
    )
    .await?;
    sync_wallet(&issuer_wallet).await?;

    let sign = sign_psbt(&issuer_wallet, final_psbt).await;
    assert!(sign.is_ok());

    for i in (max / 2)..max {
        let issuer_resp = issuer_issue_contract(iface, supply, false, true, None).await;
        let issuer = issuer_resp?;
        contracts.insert(issuer.contract_id.clone(), issuer.supply);
        // println!(
        //     "Contract #{i} ({}) : {} ({})",
        //     issuer.contract_id, issuer.supply, issuer.issue_utxo
        // );
    }

    let contract_resp = list_contracts(&issuer_sk).await?;
    assert_eq!(contracts.len(), contract_resp.contracts.len());

    for i in 0..max {
        let contract = &contract_resp.contracts[i];
        // println!(
        //     "Contract #{i} ({}) : {} ({})",
        //     contract.contract_id, contract.balance, contract.allocations[0].utxo
        // );
        assert_eq!(
            contracts.get(&contract.contract_id.to_string()).unwrap(),
            &contract.balance
        );
    }
    Ok(())
}

#[allow(unused_variables)]
#[tokio::test]
async fn allow_import_uda_before_create_watcher() -> anyhow::Result<()> {
    let stress: bool = std::env::var("STRESS_TEST")
        .unwrap_or("false".to_string())
        .parse()?;
    let network = std::env::var("BITCOIN_NETWORK").unwrap_or("bitcoin".to_string());
    if !stress && network != "regtest" {
        return Ok(());
    }

    let supply = 1;
    let issuer_keys = &save_mnemonic(
        &SecretString(ISSUER_MNEMONIC.to_string()),
        &SecretString("".to_string()),
    )
    .await?;
    let issuer_sk = issuer_keys.private.nostr_prv.clone();
    clear_stock(&issuer_sk).await;
    clear_watcher(&issuer_sk, "default").await?;

    let iface = "RGB21";
    let watcher_pub = issuer_keys.public.watcher_xpub.clone();
    send_coins(iface, &watcher_pub).await?;

    let issuer_resp = issuer_issue_contract(iface, supply, false, false, None).await;
    let issuer = issuer_resp?;

    // Clean Watcher
    clear_watcher(&issuer_sk, "default").await?;
    let contract_resp = list_contracts(&issuer_sk).await?;

    for contract in contract_resp.contracts {
        assert_eq!(0, contract.balance);
    }

    // Clean Stock
    clear_stock(&issuer_sk).await;

    // Import Contract
    let import_resp = import_new_contract(issuer).await;
    assert!(import_resp.is_ok());

    // Clean Watcher
    clear_watcher(&issuer_sk, "default").await?;
    let contract_resp = list_contracts(&issuer_sk).await?;
    for contract in contract_resp.contracts {
        assert_eq!(0, contract.balance);
    }

    // Re-create Watcher
    let create_watch_req = WatcherRequest {
        name: RGB_DEFAULT_NAME.to_string(),
        xpub: watcher_pub,
        force: false,
    };

    let resp = create_watcher(&issuer_sk, create_watch_req).await;
    assert!(resp.is_ok());

    let contract_resp = list_contracts(&issuer_sk).await?;
    for contract in contract_resp.contracts {
        assert_eq!(supply, contract.balance);
    }

    Ok(())
}
