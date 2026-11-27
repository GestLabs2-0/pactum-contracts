mod common;

use {
    anchor_lang::{
        prelude::Pubkey, solana_program::instruction::Instruction, InstructionData, ToAccountMetas,
    },
    pactum_core::state::DisputeMode,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn close_offer_rejects_wrong_rent_payer() {
    let admin = Keypair::new();
    let provider_auth = Keypair::new();
    let relayer = Keypair::new();
    let (mut svm, _, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&provider_auth.pubkey(), 10_000_000_000)
        .unwrap();
    svm.airdrop(&relayer.pubkey(), 10_000_000_000).unwrap();

    let user_main_pda = common::create_user_main(&mut svm, &provider_auth, &relayer);
    let market_pda = common::create_marketplace_account(
        &mut svm,
        1,
        Pubkey::new_unique(),
        DisputeMode::Hybrid,
        false,
        &relayer,
    );
    let provider_pda = common::create_provider_account(
        &mut svm,
        market_pda,
        user_main_pda,
        &provider_auth,
        &relayer,
    );
    let offer_pda = common::create_offer_account(
        &mut svm,
        provider_pda,
        user_main_pda,
        market_pda,
        0,
        &provider_auth,
        &relayer,
    );

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::CloseOffer {}.data(),
        pactum_core::accounts::CloseOffer {
            offer: offer_pda,
            provider: provider_pda,
            authority: provider_auth.pubkey(),
            rent_payer: provider_auth.pubkey(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &provider_auth, &[&provider_auth]).is_err());
}

#[test]
fn close_offer_refunds_lamports_to_original_rent_payer() {
    let admin = Keypair::new();
    let provider_auth = Keypair::new();
    let relayer = Keypair::new();
    let (mut svm, _, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&provider_auth.pubkey(), 10_000_000_000)
        .unwrap();
    svm.airdrop(&relayer.pubkey(), 10_000_000_000).unwrap();

    let user_main_pda = common::create_user_main(&mut svm, &provider_auth, &relayer);
    let market_pda = common::create_marketplace_account(
        &mut svm,
        2,
        Pubkey::new_unique(),
        DisputeMode::Hybrid,
        false,
        &relayer,
    );
    let provider_pda = common::create_provider_account(
        &mut svm,
        market_pda,
        user_main_pda,
        &provider_auth,
        &relayer,
    );
    let offer_pda = common::create_offer_account(
        &mut svm,
        provider_pda,
        user_main_pda,
        market_pda,
        0,
        &provider_auth,
        &relayer,
    );

    let relayer_before = svm.get_account(&relayer.pubkey()).unwrap().lamports;
    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::CloseOffer {}.data(),
        pactum_core::accounts::CloseOffer {
            offer: offer_pda,
            provider: provider_pda,
            authority: provider_auth.pubkey(),
            rent_payer: relayer.pubkey(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &provider_auth, &[&provider_auth]).unwrap();

    let relayer_after = svm.get_account(&relayer.pubkey()).unwrap().lamports;
    assert!(relayer_after > relayer_before);
    assert!(svm.get_account(&offer_pda).is_none());
}
