mod common;

use {
    anchor_lang::{
        prelude::{AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::{
        instructions::UpdateOfferParams,
        state::{CancellationPolicy, DisputeMode, Offer},
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn update_offer_changes_base_price() {
    let admin = Keypair::new();
    let provider_auth = Keypair::new();
    let (mut svm, _, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&provider_auth.pubkey(), 10_000_000_000)
        .unwrap();

    let user_main_pda = common::create_user_main(&mut svm, &provider_auth, &provider_auth);
    let market_pda = common::create_marketplace_account(
        &mut svm,
        1,
        Pubkey::new_unique(),
        DisputeMode::Hybrid,
        false,
        &provider_auth,
    );
    let provider_pda = common::create_provider_account(
        &mut svm,
        market_pda,
        user_main_pda,
        &provider_auth,
        &provider_auth,
    );
    let offer_pda = common::create_offer_account(
        &mut svm,
        provider_pda,
        user_main_pda,
        market_pda,
        0,
        &provider_auth,
        &provider_auth,
    );

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::UpdateOffer {
            params: UpdateOfferParams {
                base_price: Some(Some(75_000_000)),
                bond: None,
                default_arbiter: None,
                cancellation_policy: None,
                content_ref: None,
                is_active: None,
            },
        }
        .data(),
        pactum_core::accounts::UpdateOffer {
            offer: offer_pda,
            provider: provider_pda,
            authority: provider_auth.pubkey(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &provider_auth, &[&provider_auth]).unwrap();

    let offer =
        Offer::try_deserialize(&mut svm.get_account(&offer_pda).unwrap().data.as_slice()).unwrap();
    assert_eq!(offer.base_price, Some(75_000_000));
}

#[test]
fn update_offer_updates_cancellation_policy_and_rejects_pct_over_100() {
    let admin = Keypair::new();
    let provider_auth = Keypair::new();
    let (mut svm, _, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&provider_auth.pubkey(), 10_000_000_000)
        .unwrap();

    let user_main_pda = common::create_user_main(&mut svm, &provider_auth, &provider_auth);
    let market_pda = common::create_marketplace_account(
        &mut svm,
        10,
        Pubkey::new_unique(),
        DisputeMode::Hybrid,
        false,
        &provider_auth,
    );
    let provider_pda = common::create_provider_account(
        &mut svm,
        market_pda,
        user_main_pda,
        &provider_auth,
        &provider_auth,
    );
    let offer_pda = common::create_offer_account(
        &mut svm,
        provider_pda,
        user_main_pda,
        market_pda,
        0,
        &provider_auth,
        &provider_auth,
    );

    let policy = CancellationPolicy {
        window_hours: 24,
        client_refund_pct: 80,
    };
    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::UpdateOffer {
            params: UpdateOfferParams {
                base_price: None,
                bond: None,
                default_arbiter: None,
                cancellation_policy: Some(Some(policy)),
                content_ref: None,
                is_active: None,
            },
        }
        .data(),
        pactum_core::accounts::UpdateOffer {
            offer: offer_pda,
            provider: provider_pda,
            authority: provider_auth.pubkey(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &provider_auth, &[&provider_auth]).unwrap();

    let offer =
        Offer::try_deserialize(&mut svm.get_account(&offer_pda).unwrap().data.as_slice()).unwrap();
    assert_eq!(offer.cancellation_policy, Some(policy));

    let bad_ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::UpdateOffer {
            params: UpdateOfferParams {
                base_price: None,
                bond: None,
                default_arbiter: None,
                cancellation_policy: Some(Some(CancellationPolicy {
                    window_hours: 24,
                    client_refund_pct: 101,
                })),
                content_ref: None,
                is_active: None,
            },
        }
        .data(),
        pactum_core::accounts::UpdateOffer {
            offer: offer_pda,
            provider: provider_pda,
            authority: provider_auth.pubkey(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, bad_ix, &provider_auth, &[&provider_auth]).is_err());
}
