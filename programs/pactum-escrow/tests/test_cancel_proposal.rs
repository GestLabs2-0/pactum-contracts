mod common;

use {
    anchor_lang::{
        prelude::{system_program, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::state::PricingType,
    pactum_escrow::{
        constants::{AGREEMENT_SEED, ESCROW_VAULT_SEED},
        instructions::ProposeAgreementParams,
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn cancel_proposal_by_client_refunds_provider_bond_and_closes_accounts() {
    let authority = Keypair::new();
    let client_auth = Keypair::new();
    let provider_auth = Keypair::new();
    let stranger = Keypair::new();

    let (mut svm, global_config_pda, escrow_config_pda, usdc_mint, _) =
        common::setup_svm(&authority);
    svm.airdrop(&provider_auth.pubkey(), 10_000_000_000)
        .unwrap();
    svm.airdrop(&client_auth.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&stranger.pubkey(), 10_000_000_000).unwrap();

    let market_pda = common::seed_marketplace(&mut svm, 1);
    let (client_pda, _client_user_main_pda) =
        common::seed_client(&mut svm, market_pda, &client_auth.pubkey());
    let (provider_pda, _provider_user_main_pda) =
        common::seed_provider(&mut svm, market_pda, &provider_auth.pubkey());
    let _stranger_user_main_pda = common::seed_user_main(&mut svm, &stranger.pubkey());
    let offer_pda = common::seed_offer(
        &mut svm,
        provider_pda,
        0,
        common::SeedOfferOpts {
            pricing_type: PricingType::Custom,
            base_price: None,
            bond: 15_000_000,
            cancellation_policy: None,
        },
    );
    let provider_token_account =
        common::create_token_account(&mut svm, &usdc_mint, &provider_auth.pubkey(), 50_000_000);

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

    let propose_ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::ProposeAgreement {
            params: ProposeAgreementParams {
                nonce,
                milestone_amounts: vec![40_000_000, 60_000_000],
                step_duration_secs: 86_400,
                deal_contract_hash: None,
                arbiter: None,
            },
        }
        .data(),
        pactum_escrow::accounts::ProposeAgreement {
            provider_authority: provider_auth.pubkey(),
            payer: provider_auth.pubkey(),
            escrow_config: escrow_config_pda,
            global_config: global_config_pda,
            marketplace: market_pda,
            provider: provider_pda,
            client: client_pda,
            offer: offer_pda,
            integrator: None,
            agreement: agreement_pda,
            escrow_vault: escrow_vault_pda,
            provider_token_account,
            usdc_mint,
            token_program: common::TOKEN_PROGRAM_ID,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, propose_ix, &provider_auth, &[&provider_auth]).unwrap();

    // Non-party cannot cancel proposal.
    let bad_ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::CancelProposal {}.data(),
        pactum_escrow::accounts::CancelProposal {
            caller: stranger.pubkey(),
            escrow_config: escrow_config_pda,
            global_config: global_config_pda,
            provider: provider_pda,
            client: client_pda,
            agreement: agreement_pda,
            escrow_vault: escrow_vault_pda,
            provider_token_account,
            usdc_mint,
            rent_payer: provider_auth.pubkey(),
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, bad_ix, &stranger, &[&stranger]).is_err());

    // Client cancels proposal -> provider bond refunded, accounts closed.
    let cancel_ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::CancelProposal {}.data(),
        pactum_escrow::accounts::CancelProposal {
            caller: client_auth.pubkey(),
            escrow_config: escrow_config_pda,
            global_config: global_config_pda,
            provider: provider_pda,
            client: client_pda,
            agreement: agreement_pda,
            escrow_vault: escrow_vault_pda,
            provider_token_account,
            usdc_mint,
            rent_payer: provider_auth.pubkey(),
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, cancel_ix, &client_auth, &[&client_auth]).unwrap();

    assert_eq!(
        common::token_account_amount(&svm, &provider_token_account),
        50_000_000
    );
    assert!(svm.get_account(&agreement_pda).is_none());
    assert!(svm.get_account(&escrow_vault_pda).is_none());
}
