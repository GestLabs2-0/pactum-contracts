mod common;

use {
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::state::PricingType,
    pactum_escrow::{
        constants::{AGREEMENT_SEED, CPI_AUTHORITY_SEED, ESCROW_VAULT_SEED},
        instructions::{CreateFixedAgreementParams, SettleDisputedParams},
        state::{Agreement, AgreementStatus},
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn cpi_settle_disputed_agreement_rejects_random_wallet() {
    let authority = Keypair::new();
    let client_auth = Keypair::new();
    let provider_auth = Keypair::new();
    let attacker = Keypair::new();

    let (mut svm, global_config_pda, escrow_config_pda, usdc_mint, platform_vault) =
        common::setup_svm(&authority);
    svm.airdrop(&client_auth.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&provider_auth.pubkey(), 10_000_000_000)
        .unwrap();
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();
    common::warp_to_ts(&mut svm, 1_000_000);

    let market_pda = common::seed_marketplace(&mut svm, 1);
    let (client_pda, _client_user_main_pda) =
        common::seed_client(&mut svm, market_pda, &client_auth.pubkey());
    let (provider_pda, _provider_user_main_pda) =
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
                milestone_amounts: vec![100_000_000],
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
            client: client_pda,
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

    // Random wallet cannot settle.
    let settle_ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::CpiSettleDisputedAgreement {
            params: SettleDisputedParams {
                restore_previous_status: false,
                client_payout: 100_000_000,
                provider_payout: 0,
                fee_payout: 0,
                mark_completed: false,
            },
        }
        .data(),
        pactum_escrow::accounts::CpiSettleDisputedAgreement {
            escrow_config: escrow_config_pda,
            global_config: global_config_pda,
            cpi_authority: attacker.pubkey(),
            client: client_pda,
            provider: provider_pda,
            agreement: Some(agreement_pda),
            reservation: None,
            escrow_vault: escrow_vault_pda,
            client_token_account,
            provider_token_account,
            platform_vault,
            fee_destination: None,
            usdc_mint,
            rent_payer: client_auth.pubkey(),
            escrow_cpi_authority: cpi_authority_pda,
            core_program: pactum_core::id(),
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, settle_ix, &attacker, &[&attacker]).is_err());

    let agreement =
        Agreement::try_deserialize(&mut svm.get_account(&agreement_pda).unwrap().data.as_slice())
            .unwrap();
    assert_eq!(agreement.status, AgreementStatus::Funded);
}
