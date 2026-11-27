mod common;

use {
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::state::{Client, PricingType, Provider},
    pactum_escrow::{
        constants::{AGREEMENT_SEED, CPI_AUTHORITY_SEED, ESCROW_VAULT_SEED},
        instructions::CreateFixedAgreementParams,
        state::Agreement,
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn review_counterparty_updates_core_reputation_and_blocks_duplicate() {
    let authority = Keypair::new();
    let client_auth = Keypair::new();
    let provider_auth = Keypair::new();

    let (mut svm, global_config_pda, escrow_config_pda, usdc_mint, platform_vault) =
        common::setup_svm(&authority);
    svm.airdrop(&client_auth.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&provider_auth.pubkey(), 10_000_000_000)
        .unwrap();
    common::warp_to_ts(&mut svm, 1_000_000);

    let market_pda = common::seed_marketplace(&mut svm, 1);
    let (client_pda, client_user_main_pda) =
        common::seed_client(&mut svm, market_pda, &client_auth.pubkey());
    let (provider_pda, provider_user_main_pda) =
        common::seed_provider(&mut svm, market_pda, &provider_auth.pubkey());
    let offer_pda = common::seed_offer(
        &mut svm,
        provider_pda,
        0,
        common::SeedOfferOpts {
            pricing_type: PricingType::Fixed,
            base_price: Some(100_000_000),
            provider_bond: 0,
            required_client_bond: 0,
            cancellation_policy: None,
        },
    );
    let client_token_account =
        common::create_token_account(&mut svm, &usdc_mint, &client_auth.pubkey(), 200_000_000);
    let provider_token_account =
        common::create_token_account(&mut svm, &usdc_mint, &provider_auth.pubkey(), 0);

    let nonce = 1u64;
    let (agreement_pda, _) = Pubkey::find_program_address(
        &[
            AGREEMENT_SEED.as_bytes(),
            offer_pda.as_ref(),
            client_pda.as_ref(),
            nonce.to_le_bytes().as_ref(),
        ],
        &pactum_escrow::id(),
    );
    let (escrow_vault_pda, _) = Pubkey::find_program_address(
        &[ESCROW_VAULT_SEED.as_bytes(), agreement_pda.as_ref()],
        &pactum_escrow::id(),
    );
    let (cpi_authority_pda, _) =
        Pubkey::find_program_address(&[CPI_AUTHORITY_SEED.as_bytes()], &pactum_escrow::id());

    let create_ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::CreateFixedAgreement {
            params: CreateFixedAgreementParams {
                nonce,
                step_duration_secs: 86_400,
                deal_contract_hash: None,
                arbiter: None,
            },
        }
        .data(),
        pactum_escrow::accounts::CreateFixedAgreement {
            client_authority: client_auth.pubkey(),
            payer: client_auth.pubkey(),
            escrow_config: escrow_config_pda,
            global_config: global_config_pda,
            marketplace: market_pda,
            client_user_main: client_user_main_pda,
            client: client_pda,
            provider_user_main: provider_user_main_pda,
            provider: provider_pda,
            offer: offer_pda,
            integrator: None,
            agreement: agreement_pda,
            escrow_vault: escrow_vault_pda,
            client_token_account,
            usdc_mint,
            token_program: common::TOKEN_PROGRAM_ID,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, create_ix, &client_auth, &[&client_auth]).unwrap();

    let accept_ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::ProviderAcceptAgreement {
            expected_contract_hash: None,
        }
        .data(),
        pactum_escrow::accounts::ProviderAcceptAgreement {
            provider_authority: provider_auth.pubkey(),
            escrow_config: escrow_config_pda,
            global_config: global_config_pda,
            provider_user_main: provider_user_main_pda,
            provider: provider_pda,
            agreement: agreement_pda,
            escrow_vault: escrow_vault_pda,
            provider_token_account,
            usdc_mint,
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, accept_ix, &provider_auth, &[&provider_auth]).unwrap();

    let submit_ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::SubmitMilestone {
            deliverable_hash: [1u8; 32],
        }
        .data(),
        pactum_escrow::accounts::SubmitMilestone {
            provider_authority: provider_auth.pubkey(),
            provider_user_main: provider_user_main_pda,
            provider: provider_pda,
            agreement: agreement_pda,
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, submit_ix, &provider_auth, &[&provider_auth]).unwrap();

    let approve_ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::ApproveMilestone {}.data(),
        pactum_escrow::accounts::ApproveMilestone {
            client_authority: client_auth.pubkey(),
            escrow_config: escrow_config_pda,
            global_config: global_config_pda,
            client_user_main: client_user_main_pda,
            client: client_pda,
            provider_user_main: provider_user_main_pda,
            provider: provider_pda,
            integrator: None,
            agreement: agreement_pda,
            escrow_vault: escrow_vault_pda,
            platform_vault,
            provider_token_account,
            client_token_account,
            integrator_token_account: None,
            usdc_mint,
            rent_payer: client_auth.pubkey(),
            cpi_authority: cpi_authority_pda,
            core_program: pactum_core::id(),
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, approve_ix, &client_auth, &[&client_auth]).unwrap();

    // Client reviews Provider (5 stars).
    let client_review_ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::ReviewCounterparty { rating: 5 }.data(),
        pactum_escrow::accounts::ReviewCounterparty {
            reviewer: client_auth.pubkey(),
            escrow_config: escrow_config_pda,
            global_config: global_config_pda,
            reviewer_user_main: client_user_main_pda,
            client: client_pda,
            provider: provider_pda,
            agreement: Some(agreement_pda),
            reservation: None,
            cpi_authority: cpi_authority_pda,
            core_program: pactum_core::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, client_review_ix, &client_auth, &[&client_auth]).unwrap();

    // Duplicate review by client rejected.
    let dup_ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::ReviewCounterparty { rating: 4 }.data(),
        pactum_escrow::accounts::ReviewCounterparty {
            reviewer: client_auth.pubkey(),
            escrow_config: escrow_config_pda,
            global_config: global_config_pda,
            reviewer_user_main: client_user_main_pda,
            client: client_pda,
            provider: provider_pda,
            agreement: Some(agreement_pda),
            reservation: None,
            cpi_authority: cpi_authority_pda,
            core_program: pactum_core::id(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, dup_ix, &client_auth, &[&client_auth]).is_err());

    // Provider reviews Client (4 stars).
    let provider_review_ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::ReviewCounterparty { rating: 4 }.data(),
        pactum_escrow::accounts::ReviewCounterparty {
            reviewer: provider_auth.pubkey(),
            escrow_config: escrow_config_pda,
            global_config: global_config_pda,
            reviewer_user_main: provider_user_main_pda,
            client: client_pda,
            provider: provider_pda,
            agreement: Some(agreement_pda),
            reservation: None,
            cpi_authority: cpi_authority_pda,
            core_program: pactum_core::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(
        &mut svm,
        provider_review_ix,
        &provider_auth,
        &[&provider_auth],
    )
    .unwrap();

    let agreement =
        Agreement::try_deserialize(&mut svm.get_account(&agreement_pda).unwrap().data.as_slice())
            .unwrap();
    assert!(agreement.client_reviewed);
    assert!(agreement.provider_reviewed);

    let provider_acc =
        Provider::try_deserialize(&mut svm.get_account(&provider_pda).unwrap().data.as_slice())
            .unwrap();
    assert_eq!(provider_acc.total_reviews, 1);
    assert_eq!(provider_acc.total_score, 5);

    let client_acc =
        Client::try_deserialize(&mut svm.get_account(&client_pda).unwrap().data.as_slice())
            .unwrap();
    assert_eq!(client_acc.total_reviews, 1);
    assert_eq!(client_acc.total_score, 4);
}
