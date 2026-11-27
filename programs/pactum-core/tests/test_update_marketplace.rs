mod common;

use {
    anchor_lang::{
        prelude::{AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::{
        instructions::UpdateMarketplaceParams,
        state::{DisputeMode, Marketplace},
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

fn default_update_params() -> UpdateMarketplaceParams {
    UpdateMarketplaceParams {
        min_judge_stake: None,
        max_judge_stake: None,
        min_arbiter_stake: None,
        min_jury_fee: None,
        kyb_credential: None,
        requires_identity: None,
        is_active: None,
        new_committee: None,
    }
}

#[test]
fn update_marketplace_by_committee_updates_stake_limits() {
    let admin = Keypair::new();
    let committee = Keypair::new();
    let payer = Keypair::new();
    let (mut svm, _, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&committee.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let market_pda = common::create_marketplace_account(
        &mut svm,
        1,
        committee.pubkey(),
        DisputeMode::Hybrid,
        false,
        &payer,
    );

    let mut params = default_update_params();
    params.min_judge_stake = Some(250);
    params.max_judge_stake = Some(5_000);

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::UpdateMarketplace { params }.data(),
        pactum_core::accounts::UpdateMarketplace {
            marketplace: market_pda,
            committee: committee.pubkey(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &committee, &[&committee]).unwrap();

    let market =
        Marketplace::try_deserialize(&mut svm.get_account(&market_pda).unwrap().data.as_slice())
            .unwrap();
    assert_eq!(market.min_judge_stake, 250);
    assert_eq!(market.max_judge_stake, 5_000);
}

#[test]
fn update_marketplace_rejects_non_committee() {
    let admin = Keypair::new();
    let committee = Keypair::new();
    let attacker = Keypair::new();
    let (mut svm, _, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();

    let market_pda = common::create_marketplace_account(
        &mut svm,
        2,
        committee.pubkey(),
        DisputeMode::Hybrid,
        false,
        &attacker,
    );

    let mut params = default_update_params();
    params.max_judge_stake = Some(999_999);

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::UpdateMarketplace { params }.data(),
        pactum_core::accounts::UpdateMarketplace {
            marketplace: market_pda,
            committee: attacker.pubkey(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &attacker, &[&attacker]).is_err());
}

#[test]
fn update_marketplace_rejects_min_greater_than_max_stake() {
    let admin = Keypair::new();
    let committee = Keypair::new();
    let (mut svm, _, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&committee.pubkey(), 10_000_000_000).unwrap();

    let market_pda = common::create_marketplace_account(
        &mut svm,
        3,
        committee.pubkey(),
        DisputeMode::Hybrid,
        false,
        &committee,
    );

    let mut params = default_update_params();
    params.min_judge_stake = Some(2_000);
    params.max_judge_stake = Some(1_000);

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::UpdateMarketplace { params }.data(),
        pactum_core::accounts::UpdateMarketplace {
            marketplace: market_pda,
            committee: committee.pubkey(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &committee, &[&committee]).is_err());
}
