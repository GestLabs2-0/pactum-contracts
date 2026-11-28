mod common;

use {
    anchor_lang::{
        prelude::{AccountDeserialize, Pubkey},
        solana_program::{instruction::Instruction, system_program},
        InstructionData, ToAccountMetas,
    },
    pactum_core::{
        constants::OFFER_SEED,
        instructions::CreateOfferParams,
        state::{CancellationPolicy, DisputeMode, Offer, PricingType, Provider},
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn create_offer_increments_provider_count_and_sets_rent_payer() {
    let admin = Keypair::new();
    let provider_auth = Keypair::new();
    let relayer = Keypair::new();
    let (mut svm, _, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
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

    let offer =
        Offer::try_deserialize(&mut svm.get_account(&offer_pda).unwrap().data.as_slice()).unwrap();
    assert_eq!(offer.offer_id, 0);
    assert_eq!(offer.rent_payer, relayer.pubkey());
    assert!(offer.is_active);

    let provider =
        Provider::try_deserialize(&mut svm.get_account(&provider_pda).unwrap().data.as_slice())
            .unwrap();
    assert_eq!(provider.offer_count, 1);
}

#[test]
fn create_offer_sets_cancellation_policy_and_rejects_pct_over_100() {
    let admin = Keypair::new();
    let provider_auth = Keypair::new();
    let (mut svm, _, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&provider_auth.pubkey(), 10_000_000_000)
        .unwrap();

    let user_main_pda = common::create_user_main(&mut svm, &provider_auth, &provider_auth);
    let market_pda = common::create_marketplace_account(
        &mut svm,
        2,
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

    let (offer_pda, _) = Pubkey::find_program_address(
        &[
            OFFER_SEED.as_bytes(),
            provider_pda.as_ref(),
            0u32.to_le_bytes().as_ref(),
        ],
        &pactum_core::id(),
    );
    let policy = CancellationPolicy {
        window_hours: 48,
        client_refund_pct: 50,
    };
    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::CreateOffer {
            params: CreateOfferParams {
                base_price: Some(100_000_000),
                pricing_type: PricingType::DateRange,
                approval_window_secs: 86_400,
                bond: 0,
                jury_fee: 2_500_000,
                default_arbiter: None,
                cancellation_policy: Some(policy),
                content_ref: [1u8; 32],
            },
        }
        .data(),
        pactum_core::accounts::CreateOffer {
            offer: offer_pda,
            provider: provider_pda,
            marketplace: market_pda,
            authority: provider_auth.pubkey(),
            payer: provider_auth.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &provider_auth, &[&provider_auth]).unwrap();

    let offer =
        Offer::try_deserialize(&mut svm.get_account(&offer_pda).unwrap().data.as_slice()).unwrap();
    assert_eq!(offer.cancellation_policy, Some(policy));

    let (bad_offer_pda, _) = Pubkey::find_program_address(
        &[
            OFFER_SEED.as_bytes(),
            provider_pda.as_ref(),
            1u32.to_le_bytes().as_ref(),
        ],
        &pactum_core::id(),
    );
    let bad_ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::CreateOffer {
            params: CreateOfferParams {
                base_price: Some(100_000_000),
                pricing_type: PricingType::DateRange,
                approval_window_secs: 86_400,
                bond: 0,
                jury_fee: 2_500_000,
                default_arbiter: None,
                cancellation_policy: Some(CancellationPolicy {
                    window_hours: 48,
                    client_refund_pct: 101,
                }),
                content_ref: [1u8; 32],
            },
        }
        .data(),
        pactum_core::accounts::CreateOffer {
            offer: bad_offer_pda,
            provider: provider_pda,
            marketplace: market_pda,
            authority: provider_auth.pubkey(),
            payer: provider_auth.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, bad_ix, &provider_auth, &[&provider_auth]).is_err());
}
