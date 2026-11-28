mod common;

use {
    anchor_lang::{
        prelude::{system_program, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    common::{
        create_token_account, deserialize_account, dispute_pda, seed_agreement_with_vault,
        seed_arbiter, seed_client, seed_marketplace, seed_offer, seed_provider, send_ix, setup_svm,
        token_account_amount, warp_to_ts,
    },
    pactum_core::state::{ArbiterProfile, Client, DisputeMode as MarketDisputeMode, Provider},
    pactum_disputes::{
        constants::CPI_AUTHORITY_SEED,
        state::{Dispute, DisputeOutcome, DisputeStatus},
    },
    pactum_escrow::constants::CPI_AUTHORITY_SEED as ESCROW_CPI_SEED,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn resolve_by_arbiter_client_wins_pays_winner_and_arbiter_fee() {
    let mut env = setup_svm(true);
    let client_kp = Keypair::new();
    let provider_kp = Keypair::new();
    let arbiter_kp = Keypair::new();
    env.svm.airdrop(&client_kp.pubkey(), 1_000_000_000).unwrap();
    env.svm
        .airdrop(&arbiter_kp.pubkey(), 1_000_000_000)
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

    let client_ta = create_token_account(&mut env.svm, &env.usdc_mint, &client_kp.pubkey(), 0);
    let provider_ta = create_token_account(&mut env.svm, &env.usdc_mint, &provider_kp.pubkey(), 0);
    let arbiter_ta = create_token_account(&mut env.svm, &env.usdc_mint, &arbiter_kp.pubkey(), 0);

    let resolve_ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::ResolveByArbiter {
            outcome: DisputeOutcome::ClientWins,
        }
        .data(),
        pactum_disputes::accounts::ResolveByArbiter {
            dispute,
            dispute_config: env.dispute_config,
            arbiter_profile: arbiter,
            arbiter_authority: arbiter_kp.pubkey(),
            client,
            provider,
            agreement: Some(agreement),
            reservation: None,
            escrow_vault: vault,
            client_token_account: client_ta,
            provider_token_account: provider_ta,
            arbiter_token_account: arbiter_ta,
            platform_vault: env.platform_vault,
            usdc_mint: env.usdc_mint,
            rent_payer: client_kp.pubkey(),
            escrow_config: env.escrow_config,
            global_config: env.global_config,
            cpi_authority,
            escrow_cpi_authority,
            escrow_program: pactum_escrow::id(),
            core_program: pactum_core::id(),
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    send_ix(&mut env.svm, resolve_ix, &arbiter_kp, &[&arbiter_kp]).unwrap();

    // Client: 10M + 1M + 2M = 13M; Provider: 1M + (2M - 1.5M) = 1.5M; Arbiter: 1.5M
    assert_eq!(token_account_amount(&env.svm, &client_ta), 13_000_000);
    assert_eq!(token_account_amount(&env.svm, &provider_ta), 1_500_000);
    assert_eq!(token_account_amount(&env.svm, &arbiter_ta), 1_500_000);

    let d: Dispute = deserialize_account(&env.svm, &dispute);
    assert_eq!(d.status, DisputeStatus::Resolved);
    assert_eq!(d.outcome, Some(DisputeOutcome::ClientWins));

    let c: Client = deserialize_account(&env.svm, &client);
    let p: Provider = deserialize_account(&env.svm, &provider);
    assert_eq!(c.disputes_won, 1);
    assert_eq!(p.disputes_lost, 1);

    let arb: ArbiterProfile = deserialize_account(&env.svm, &arbiter);
    assert_eq!(arb.cases_resolved, 1);
    assert_eq!(arb.active_disputes, 0);
}
