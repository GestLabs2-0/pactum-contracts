mod common;

use {
    anchor_lang::{
        prelude::{system_program, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    common::{
        arbiter_vault_pda, create_token_account, deserialize_account, dispute_pda,
        seed_agreement_with_vault, seed_arbiter, seed_client, seed_integrator, seed_marketplace,
        seed_offer, seed_provider, seed_treasury_vault, send_ix, send_ixs, set_cu_limit_ix,
        setup_svm, token_account_amount, warp_to_ts,
    },
    pactum_core::state::{ArbiterProfile, DisputeMode as MarketDisputeMode},
    pactum_disputes::{
        constants::CPI_AUTHORITY_SEED,
        state::{Dispute, DisputeMode, DisputeOutcome, DisputeStatus},
    },
    pactum_escrow::constants::CPI_AUTHORITY_SEED as ESCROW_CPI_SEED,
    pactum_treasury::constants::{ARBITER_VAULT_SEED, CPI_AUTHORITY_SEED as TREASURY_CPI_SEED},
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn fallback_to_integrator_slashes_arbiter_and_settles_with_integrator() {
    let mut env = setup_svm(true);
    let client_kp = Keypair::new();
    let provider_kp = Keypair::new();
    let arbiter_kp = Keypair::new();
    let integrator_kp = Keypair::new();
    env.svm.airdrop(&client_kp.pubkey(), 1_000_000_000).unwrap();
    env.svm
        .airdrop(&integrator_kp.pubkey(), 1_000_000_000)
        .unwrap();

    let market = seed_marketplace(&mut env.svm, 1, MarketDisputeMode::Hybrid);
    let client = seed_client(&mut env.svm, &market, &client_kp.pubkey());
    let provider = seed_provider(&mut env.svm, &market, &provider_kp.pubkey());
    let arbiter = seed_arbiter(
        &mut env.svm,
        &market,
        &arbiter_kp.pubkey(),
        1_500_000,
        10_000_000,
        86_400,
        true,
    );
    let integrator = seed_integrator(&mut env.svm, &market, &integrator_kp.pubkey(), true);
    seed_treasury_vault(
        &mut env.svm,
        ARBITER_VAULT_SEED,
        &market,
        &env.usdc_mint,
        10_000_000,
    );
    let offer = seed_offer(&mut env.svm, &provider, 1);
    let (agreement, vault) = seed_agreement_with_vault(
        &mut env.svm,
        &env.usdc_mint,
        &offer,
        &client,
        &provider,
        Some(arbiter_kp.pubkey()),
        Some(integrator),
        1,
        10_000_000,
        1_000_000,
        2_000_000,
        &client_kp.pubkey(),
    );

    let dispute = dispute_pda(&agreement);
    let (cpi_authority, _) =
        Pubkey::find_program_address(&[CPI_AUTHORITY_SEED.as_bytes()], &pactum_disputes::id());
    let (escrow_cpi_authority, _) =
        Pubkey::find_program_address(&[ESCROW_CPI_SEED.as_bytes()], &pactum_escrow::id());
    let (treasury_cpi_authority, _) =
        Pubkey::find_program_address(&[TREASURY_CPI_SEED.as_bytes()], &pactum_treasury::id());

    warp_to_ts(&mut env.svm, 1_000);
    let open_ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::OpenDispute {}.data(),
        pactum_disputes::accounts::OpenDispute {
            dispute,
            target_agreement: agreement,
            agreement: Some(agreement),
            reservation: None,
            marketplace: market,
            client,
            provider,
            dispute_config: env.dispute_config,
            escrow_config: env.escrow_config,
            global_config: env.global_config,
            cpi_authority,
            signer: client_kp.pubkey(),
            payer: client_kp.pubkey(),
            escrow_program: pactum_escrow::id(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    send_ix(&mut env.svm, open_ix, &client_kp, &[&client_kp]).unwrap();

    warp_to_ts(&mut env.svm, 1_000 + 172_800);
    let esc_ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::EscalateDispute {}.data(),
        pactum_disputes::accounts::EscalateDispute {
            dispute,
            dispute_config: env.dispute_config,
            client,
            provider,
            arbiter_profile: Some(arbiter),
            global_config: env.global_config,
            cpi_authority,
            signer: client_kp.pubkey(),
            counterparty_signer: None,
            core_program: pactum_core::id(),
        }
        .to_account_metas(None),
    );
    send_ix(&mut env.svm, esc_ix, &client_kp, &[&client_kp]).unwrap();

    warp_to_ts(&mut env.svm, 1_000 + 172_800 + 86_401);
    let client_ta = create_token_account(&mut env.svm, &env.usdc_mint, &client_kp.pubkey(), 0);
    let provider_ta = create_token_account(&mut env.svm, &env.usdc_mint, &provider_kp.pubkey(), 0);
    let integrator_ta =
        create_token_account(&mut env.svm, &env.usdc_mint, &integrator_kp.pubkey(), 0);

    let fallback_ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::FallbackToIntegrator {
            outcome: Some(DisputeOutcome::ProviderWins),
        }
        .data(),
        pactum_disputes::accounts::FallbackToIntegrator {
            dispute,
            dispute_config: env.dispute_config,
            marketplace: market,
            old_arbiter: arbiter,
            integrator_profile: Some(integrator),
            integrator_authority: Some(integrator_kp.pubkey()),
            integrator_token_account: Some(integrator_ta),
            arbiter_stake_vault: arbiter_vault_pda(&market),
            client,
            provider,
            agreement: Some(agreement),
            reservation: None,
            escrow_vault: vault,
            client_token_account: client_ta,
            provider_token_account: provider_ta,
            platform_vault: env.platform_vault,
            usdc_mint: env.usdc_mint,
            rent_payer: client_kp.pubkey(),
            escrow_config: env.escrow_config,
            treasury_config: env.treasury_config,
            global_config: env.global_config,
            cpi_authority,
            escrow_cpi_authority,
            treasury_cpi_authority,
            escrow_program: pactum_escrow::id(),
            treasury_program: pactum_treasury::id(),
            core_program: pactum_core::id(),
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    send_ixs(
        &mut env.svm,
        &[set_cu_limit_ix(400_000), fallback_ix],
        &integrator_kp,
        &[&integrator_kp],
    )
    .unwrap();

    assert_eq!(token_account_amount(&env.svm, &integrator_ta), 500_000);
    assert_eq!(token_account_amount(&env.svm, &provider_ta), 13_000_000);
    assert_eq!(token_account_amount(&env.svm, &client_ta), 3_000_000);

    let d: Dispute = deserialize_account(&env.svm, &dispute);
    assert_eq!(d.status, DisputeStatus::Resolved);
    assert_eq!(d.outcome, Some(DisputeOutcome::ProviderWins));

    let arb: ArbiterProfile = deserialize_account(&env.svm, &arbiter);
    assert_eq!(arb.active_disputes, 0);
}

#[test]
fn fallback_to_integrator_hybrid_without_integrator_transitions_to_jury() {
    let mut env = setup_svm(true);
    let client_kp = Keypair::new();
    let provider_kp = Keypair::new();
    let arbiter_kp = Keypair::new();
    env.svm.airdrop(&client_kp.pubkey(), 1_000_000_000).unwrap();

    let market = seed_marketplace(&mut env.svm, 2, MarketDisputeMode::Hybrid);
    let client = seed_client(&mut env.svm, &market, &client_kp.pubkey());
    let provider = seed_provider(&mut env.svm, &market, &provider_kp.pubkey());
    let arbiter = seed_arbiter(
        &mut env.svm,
        &market,
        &arbiter_kp.pubkey(),
        1_500_000,
        10_000_000,
        86_400,
        true,
    );
    let offer = seed_offer(&mut env.svm, &provider, 1);
    let (agreement, vault) = seed_agreement_with_vault(
        &mut env.svm,
        &env.usdc_mint,
        &offer,
        &client,
        &provider,
        Some(arbiter_kp.pubkey()),
        None,
        1,
        10_000_000,
        1_000_000,
        2_000_000,
        &client_kp.pubkey(),
    );

    let dispute = dispute_pda(&agreement);
    let (cpi_authority, _) =
        Pubkey::find_program_address(&[CPI_AUTHORITY_SEED.as_bytes()], &pactum_disputes::id());
    let (escrow_cpi_authority, _) =
        Pubkey::find_program_address(&[ESCROW_CPI_SEED.as_bytes()], &pactum_escrow::id());
    let (treasury_cpi_authority, _) =
        Pubkey::find_program_address(&[TREASURY_CPI_SEED.as_bytes()], &pactum_treasury::id());

    warp_to_ts(&mut env.svm, 1_000);
    let open_ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::OpenDispute {}.data(),
        pactum_disputes::accounts::OpenDispute {
            dispute,
            target_agreement: agreement,
            agreement: Some(agreement),
            reservation: None,
            marketplace: market,
            client,
            provider,
            dispute_config: env.dispute_config,
            escrow_config: env.escrow_config,
            global_config: env.global_config,
            cpi_authority,
            signer: client_kp.pubkey(),
            payer: client_kp.pubkey(),
            escrow_program: pactum_escrow::id(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    send_ix(&mut env.svm, open_ix, &client_kp, &[&client_kp]).unwrap();

    warp_to_ts(&mut env.svm, 1_000 + 172_800);
    let esc_ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::EscalateDispute {}.data(),
        pactum_disputes::accounts::EscalateDispute {
            dispute,
            dispute_config: env.dispute_config,
            client,
            provider,
            arbiter_profile: Some(arbiter),
            global_config: env.global_config,
            cpi_authority,
            signer: client_kp.pubkey(),
            counterparty_signer: None,
            core_program: pactum_core::id(),
        }
        .to_account_metas(None),
    );
    send_ix(&mut env.svm, esc_ix, &client_kp, &[&client_kp]).unwrap();

    warp_to_ts(&mut env.svm, 1_000 + 172_800 + 86_401);
    let client_ta = create_token_account(&mut env.svm, &env.usdc_mint, &client_kp.pubkey(), 0);
    let provider_ta = create_token_account(&mut env.svm, &env.usdc_mint, &provider_kp.pubkey(), 0);

    let fallback_ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::FallbackToIntegrator { outcome: None }.data(),
        pactum_disputes::accounts::FallbackToIntegrator {
            dispute,
            dispute_config: env.dispute_config,
            marketplace: market,
            old_arbiter: arbiter,
            integrator_profile: None,
            integrator_authority: None,
            integrator_token_account: None,
            arbiter_stake_vault: arbiter_vault_pda(&market),
            client,
            provider,
            agreement: Some(agreement),
            reservation: None,
            escrow_vault: vault,
            client_token_account: client_ta,
            provider_token_account: provider_ta,
            platform_vault: env.platform_vault,
            usdc_mint: env.usdc_mint,
            rent_payer: client_kp.pubkey(),
            escrow_config: env.escrow_config,
            treasury_config: env.treasury_config,
            global_config: env.global_config,
            cpi_authority,
            escrow_cpi_authority,
            treasury_cpi_authority,
            escrow_program: pactum_escrow::id(),
            treasury_program: pactum_treasury::id(),
            core_program: pactum_core::id(),
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    send_ix(&mut env.svm, fallback_ix, &client_kp, &[&client_kp]).unwrap();

    let d: Dispute = deserialize_account(&env.svm, &dispute);
    assert_eq!(d.mode, DisputeMode::DecentralizedJury);
    assert_eq!(d.status, DisputeStatus::VrfPending);
    assert_eq!(d.assigned_arbiter, None);
}
