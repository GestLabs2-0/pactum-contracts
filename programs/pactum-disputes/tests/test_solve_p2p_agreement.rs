mod common;

use {
    anchor_lang::{
        prelude::{system_program, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    common::{
        create_token_account, deserialize_account, dispute_pda, seed_agreement_with_vault,
        seed_client, seed_marketplace, seed_offer, seed_provider, send_ix, setup_svm,
        token_account_amount,
    },
    pactum_core::state::DisputeMode as MarketDisputeMode,
    pactum_disputes::{
        constants::CPI_AUTHORITY_SEED,
        state::{Dispute, DisputeOutcome, DisputeStatus},
        SolveP2pParams,
    },
    pactum_escrow::{constants::CPI_AUTHORITY_SEED as ESCROW_CPI_SEED, state::Agreement},
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn solve_p2p_agreement_settles_split_and_refunds_bonds_and_jury_fees() {
    let mut env = setup_svm(true);
    let client_kp = Keypair::new();
    let provider_kp = Keypair::new();
    env.svm.airdrop(&client_kp.pubkey(), 1_000_000_000).unwrap();
    env.svm
        .airdrop(&provider_kp.pubkey(), 1_000_000_000)
        .unwrap();

    let market = seed_marketplace(&mut env.svm, 1, MarketDisputeMode::Hybrid);
    let client = seed_client(&mut env.svm, &market, &client_kp.pubkey());
    let provider = seed_provider(&mut env.svm, &market, &provider_kp.pubkey());
    let offer = seed_offer(&mut env.svm, &provider, 1);
    let (agreement, vault) = seed_agreement_with_vault(
        &mut env.svm,
        &env.usdc_mint,
        &offer,
        &client,
        &provider,
        None,
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

    let client_ta = create_token_account(&mut env.svm, &env.usdc_mint, &client_kp.pubkey(), 0);
    let provider_ta = create_token_account(&mut env.svm, &env.usdc_mint, &provider_kp.pubkey(), 0);

    let solve_ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::SolveP2pAgreement {
            params: SolveP2pParams {
                restore_previous_status: false,
                client_principal_payout: 6_000_000,
                provider_principal_payout: 4_000_000,
                mark_completed: false,
            },
        }
        .data(),
        pactum_disputes::accounts::SolveP2pAgreement {
            dispute,
            client,
            provider,
            client_authority: client_kp.pubkey(),
            provider_authority: provider_kp.pubkey(),
            agreement: Some(agreement),
            reservation: None,
            escrow_vault: vault,
            client_token_account: client_ta,
            provider_token_account: provider_ta,
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
    send_ix(
        &mut env.svm,
        solve_ix,
        &client_kp,
        &[&client_kp, &provider_kp],
    )
    .unwrap();

    // 6M principal + 1M bond + 2M jury_fee = 9M; 4M + 1M + 2M = 7M
    assert_eq!(token_account_amount(&env.svm, &client_ta), 9_000_000);
    assert_eq!(token_account_amount(&env.svm, &provider_ta), 7_000_000);

    let d: Dispute = deserialize_account(&env.svm, &dispute);
    assert_eq!(d.status, DisputeStatus::Resolved);
    assert_eq!(d.outcome, Some(DisputeOutcome::P2PAgreement));

    let _ag: Agreement = deserialize_account(&env.svm, &agreement);
}
