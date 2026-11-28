mod common;

use {
    anchor_lang::{
        prelude::{system_program, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    common::{
        arbiter_vault_pda, create_token_account, deserialize_account, dispute_pda,
        seed_agreement_with_vault, seed_arbiter, seed_client, seed_marketplace, seed_offer,
        seed_provider, seed_treasury_vault, send_ix, setup_svm, token_account_amount, warp_to_ts,
    },
    pactum_core::state::{ArbiterProfile, DisputeMode as MarketDisputeMode},
    pactum_disputes::{constants::CPI_AUTHORITY_SEED, state::Dispute},
    pactum_treasury::constants::{ARBITER_VAULT_SEED, CPI_AUTHORITY_SEED as TREASURY_CPI_SEED},
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn accept_arbiter_replacement_slashes_old_arbiter_and_updates_assignment() {
    let mut env = setup_svm(true);
    let client_kp = Keypair::new();
    let provider_kp = Keypair::new();
    let old_arbiter_kp = Keypair::new();
    let new_arbiter_kp = Keypair::new();
    env.svm.airdrop(&client_kp.pubkey(), 1_000_000_000).unwrap();
    env.svm
        .airdrop(&provider_kp.pubkey(), 1_000_000_000)
        .unwrap();

    let market = seed_marketplace(&mut env.svm, 1, MarketDisputeMode::Hybrid);
    let client = seed_client(&mut env.svm, &market, &client_kp.pubkey());
    let provider = seed_provider(&mut env.svm, &market, &provider_kp.pubkey());
    let old_arbiter = seed_arbiter(
        &mut env.svm,
        &market,
        &old_arbiter_kp.pubkey(),
        1_500_000,
        10_000_000,
        86_400,
        true,
    );
    let new_arbiter = seed_arbiter(
        &mut env.svm,
        &market,
        &new_arbiter_kp.pubkey(),
        1_500_000,
        10_000_000,
        86_400,
        true,
    );
    seed_treasury_vault(
        &mut env.svm,
        ARBITER_VAULT_SEED,
        &market,
        &env.usdc_mint,
        10_000_000,
    );
    let offer = seed_offer(&mut env.svm, &provider, 1);
    let (agreement, _) = seed_agreement_with_vault(
        &mut env.svm,
        &env.usdc_mint,
        &offer,
        &client,
        &provider,
        Some(old_arbiter_kp.pubkey()),
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
            arbiter_profile: Some(old_arbiter),
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
    let prop_ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::ProposeArbiterReplacement {}.data(),
        pactum_disputes::accounts::ProposeArbiterReplacement {
            dispute,
            client,
            provider,
            current_arbiter: old_arbiter,
            new_arbiter,
            signer: client_kp.pubkey(),
        }
        .to_account_metas(None),
    );
    send_ix(&mut env.svm, prop_ix, &client_kp, &[&client_kp]).unwrap();

    let new_arbiter_ta =
        create_token_account(&mut env.svm, &env.usdc_mint, &new_arbiter_kp.pubkey(), 0);
    let arb_vault = arbiter_vault_pda(&market);

    let accept_ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::AcceptArbiterReplacement {}.data(),
        pactum_disputes::accounts::AcceptArbiterReplacement {
            dispute,
            dispute_config: env.dispute_config,
            marketplace: market,
            client,
            provider,
            old_arbiter,
            new_arbiter,
            arbiter_stake_vault: arb_vault,
            new_arbiter_token_account: new_arbiter_ta,
            usdc_mint: env.usdc_mint,
            treasury_config: env.treasury_config,
            global_config: env.global_config,
            cpi_authority,
            treasury_cpi_authority,
            signer: provider_kp.pubkey(),
            treasury_program: pactum_treasury::id(),
            core_program: pactum_core::id(),
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    send_ix(&mut env.svm, accept_ix, &provider_kp, &[&provider_kp]).unwrap();

    // 5% of 10_000_000 = 500_000 transferred to replacement arbiter.
    assert_eq!(token_account_amount(&env.svm, &new_arbiter_ta), 500_000);

    let old_arb: ArbiterProfile = deserialize_account(&env.svm, &old_arbiter);
    let new_arb: ArbiterProfile = deserialize_account(&env.svm, &new_arbiter);
    assert_eq!(old_arb.active_disputes, 0);
    assert_eq!(old_arb.staked_amount, 9_500_000);
    assert_eq!(new_arb.active_disputes, 1);

    let d: Dispute = deserialize_account(&env.svm, &dispute);
    assert_eq!(d.assigned_arbiter, Some(new_arbiter_kp.pubkey()));
    assert_eq!(d.proposed_replacement_arbiter, None);
}
