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
        instructions::ProposeAgreementParams,
        state::{Agreement, AgreementStatus},
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn approve_milestone_loops_and_settles_final_milestone_with_fees_bonds_and_outcomes() {
    let authority = Keypair::new();
    let client_auth = Keypair::new();
    let provider_auth = Keypair::new();

    let (mut svm, global_config_pda, escrow_config_pda, usdc_mint, platform_vault) =
        common::setup_svm(&authority);
    svm.airdrop(&provider_auth.pubkey(), 10_000_000_000)
        .unwrap();
    svm.airdrop(&client_auth.pubkey(), 10_000_000_000).unwrap();
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
            pricing_type: PricingType::Custom,
            base_price: None,
            bond: 10_000_000,
            cancellation_policy: None,
        },
    );
    let provider_token_account =
        common::create_token_account(&mut svm, &usdc_mint, &provider_auth.pubkey(), 50_000_000);
    let client_token_account =
        common::create_token_account(&mut svm, &usdc_mint, &client_auth.pubkey(), 200_000_000);

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

    // Propose 2 milestones: 40_000_000 + 60_000_000 = 100_000_000.
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

    // Fund.
    let fund_ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::FundAgreement {
            expected_contract_hash: None,
        }
        .data(),
        pactum_escrow::accounts::FundAgreement {
            client_authority: client_auth.pubkey(),
            escrow_config: escrow_config_pda,
            global_config: global_config_pda,
            client: client_pda,
            agreement: agreement_pda,
            escrow_vault: escrow_vault_pda,
            client_token_account,
            usdc_mint,
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, fund_ix, &client_auth, &[&client_auth]).unwrap();

    // Submit & approve milestone 1 (40_000_000 -> 1% fee = 400_000, net = 39_600_000).
    let submit1 = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::SubmitMilestone {
            deliverable_hash: [1u8; 32],
        }
        .data(),
        pactum_escrow::accounts::SubmitMilestone {
            provider_authority: provider_auth.pubkey(),
            provider: provider_pda,
            agreement: agreement_pda,
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, submit1, &provider_auth, &[&provider_auth]).unwrap();

    let approve1 = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::ApproveMilestone {}.data(),
        pactum_escrow::accounts::ApproveMilestone {
            client_authority: client_auth.pubkey(),
            escrow_config: escrow_config_pda,
            global_config: global_config_pda,
            client: client_pda,
            provider: provider_pda,
            integrator: None,
            agreement: agreement_pda,
            escrow_vault: escrow_vault_pda,
            platform_vault,
            provider_token_account,
            client_token_account,
            integrator_token_account: None,
            usdc_mint,
            rent_payer: provider_auth.pubkey(),
            cpi_authority: cpi_authority_pda,
            core_program: pactum_core::id(),
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, approve1, &client_auth, &[&client_auth]).unwrap();

    let mid =
        Agreement::try_deserialize(&mut svm.get_account(&agreement_pda).unwrap().data.as_slice())
            .unwrap();
    assert_eq!(mid.status, AgreementStatus::Funded);
    assert_eq!(mid.completed_milestones, 1);
    assert_eq!(mid.released_amount, 40_000_000);
    assert_eq!(mid.current_deadline_ts, 1_000_000 + 2 * 86_400);

    // Submit & approve milestone 2 (60_000_000 -> 1% fee = 600_000, net = 59_400_000 + 10M bond).
    let submit2 = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::SubmitMilestone {
            deliverable_hash: [2u8; 32],
        }
        .data(),
        pactum_escrow::accounts::SubmitMilestone {
            provider_authority: provider_auth.pubkey(),
            provider: provider_pda,
            agreement: agreement_pda,
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, submit2, &provider_auth, &[&provider_auth]).unwrap();

    let approve2 = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::ApproveMilestone {}.data(),
        pactum_escrow::accounts::ApproveMilestone {
            client_authority: client_auth.pubkey(),
            escrow_config: escrow_config_pda,
            global_config: global_config_pda,
            client: client_pda,
            provider: provider_pda,
            integrator: None,
            agreement: agreement_pda,
            escrow_vault: escrow_vault_pda,
            platform_vault,
            provider_token_account,
            client_token_account,
            integrator_token_account: None,
            usdc_mint,
            rent_payer: provider_auth.pubkey(),
            cpi_authority: cpi_authority_pda,
            core_program: pactum_core::id(),
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, approve2, &client_auth, &[&client_auth]).unwrap();

    let done =
        Agreement::try_deserialize(&mut svm.get_account(&agreement_pda).unwrap().data.as_slice())
            .unwrap();
    assert_eq!(done.status, AgreementStatus::Completed);
    assert_eq!(done.completed_milestones, 2);
    assert_eq!(done.released_amount, 100_000_000);
    assert!(svm.get_account(&escrow_vault_pda).is_none());
    assert_eq!(
        common::token_account_amount(&svm, &platform_vault),
        1_000_000
    );
    // Provider started with 50M, paid 10M bond, received 39.6M + 59.4M + 10M bond = 149M.
    assert_eq!(
        common::token_account_amount(&svm, &provider_token_account),
        149_000_000
    );
    // Client started with 200M, paid 120M (100M + 20M bond), received 20M bond back = 100M.
    assert_eq!(
        common::token_account_amount(&svm, &client_token_account),
        100_000_000
    );

    let client_acc =
        Client::try_deserialize(&mut svm.get_account(&client_pda).unwrap().data.as_slice())
            .unwrap();
    let provider_acc =
        Provider::try_deserialize(&mut svm.get_account(&provider_pda).unwrap().data.as_slice())
            .unwrap();
    assert_eq!(client_acc.completed_agreements, 1);
    assert_eq!(provider_acc.completed_agreements, 1);
}
