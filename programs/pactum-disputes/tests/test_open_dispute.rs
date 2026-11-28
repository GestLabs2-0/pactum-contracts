mod common;

use {
    anchor_lang::{
        prelude::{system_program, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    common::{
        deserialize_account, dispute_pda, seed_agreement_with_vault, seed_client, seed_marketplace,
        seed_offer, seed_provider, send_ix, setup_svm,
    },
    pactum_core::state::{DisputeMode as MarketDisputeMode, DisputeParty},
    pactum_disputes::{
        constants::CPI_AUTHORITY_SEED,
        state::{Dispute, DisputeMode, DisputeStatus},
    },
    pactum_escrow::state::{Agreement, AgreementStatus},
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn open_dispute_freezes_agreement_and_routes_hybrid_mode() {
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
    let (agreement, _) = seed_agreement_with_vault(
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

    let ix = Instruction::new_with_bytes(
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
    send_ix(&mut env.svm, ix, &client_kp, &[&client_kp]).unwrap();

    let d: Dispute = deserialize_account(&env.svm, &dispute);
    assert_eq!(d.agreement, agreement);
    assert_eq!(d.opened_by, DisputeParty::Client);
    assert_eq!(d.mode, DisputeMode::DecentralizedJury);
    assert_eq!(d.status, DisputeStatus::OpenP2P);

    let ag: Agreement = deserialize_account(&env.svm, &agreement);
    assert_eq!(ag.status, AgreementStatus::Disputed);
    assert_eq!(ag.previous_status, Some(AgreementStatus::Funded));
}

#[test]
fn open_dispute_rejects_arbiter_only_without_arbiter() {
    let mut env = setup_svm(true);
    let client_kp = Keypair::new();
    let provider_kp = Keypair::new();
    env.svm.airdrop(&client_kp.pubkey(), 1_000_000_000).unwrap();

    let market = seed_marketplace(&mut env.svm, 2, MarketDisputeMode::ArbiterOnly);
    let client = seed_client(&mut env.svm, &market, &client_kp.pubkey());
    let provider = seed_provider(&mut env.svm, &market, &provider_kp.pubkey());
    let offer = seed_offer(&mut env.svm, &provider, 1);
    let (agreement, _) = seed_agreement_with_vault(
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

    let ix = Instruction::new_with_bytes(
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
    assert!(send_ix(&mut env.svm, ix, &client_kp, &[&client_kp]).is_err());
}
