mod common;

use {
    anchor_lang::{
        prelude::{system_program, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    common::{
        deserialize_account, dispute_pda, seed_agreement_with_vault, seed_arbiter, seed_client,
        seed_marketplace, seed_offer, seed_provider, send_ix, setup_svm, warp_to_ts,
    },
    pactum_core::state::{DisputeMode as MarketDisputeMode, DisputeParty},
    pactum_disputes::{constants::CPI_AUTHORITY_SEED, state::Dispute},
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn propose_arbiter_replacement_succeeds_after_sla_deadline() {
    let mut env = setup_svm(true);
    let client_kp = Keypair::new();
    let provider_kp = Keypair::new();
    let arbiter_kp = Keypair::new();
    let new_arbiter_kp = Keypair::new();
    env.svm.airdrop(&client_kp.pubkey(), 1_000_000_000).unwrap();

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
    let new_arbiter = seed_arbiter(
        &mut env.svm,
        &market,
        &new_arbiter_kp.pubkey(),
        1_500_000,
        10_000_000,
        86_400,
        true,
    );
    let offer = seed_offer(&mut env.svm, &provider, 1);
    let (agreement, _) = seed_agreement_with_vault(
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

    // Fails before SLA deadline.
    let early_ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::ProposeArbiterReplacement {}.data(),
        pactum_disputes::accounts::ProposeArbiterReplacement {
            dispute,
            client,
            provider,
            current_arbiter: arbiter,
            new_arbiter,
            signer: client_kp.pubkey(),
        }
        .to_account_metas(None),
    );
    assert!(send_ix(&mut env.svm, early_ix, &client_kp, &[&client_kp]).is_err());

    warp_to_ts(&mut env.svm, 1_000 + 172_800 + 86_401);
    let prop_ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::ProposeArbiterReplacement {}.data(),
        pactum_disputes::accounts::ProposeArbiterReplacement {
            dispute,
            client,
            provider,
            current_arbiter: arbiter,
            new_arbiter,
            signer: client_kp.pubkey(),
        }
        .to_account_metas(None),
    );
    send_ix(&mut env.svm, prop_ix, &client_kp, &[&client_kp]).unwrap();

    let d: Dispute = deserialize_account(&env.svm, &dispute);
    assert_eq!(
        d.proposed_replacement_arbiter,
        Some(new_arbiter_kp.pubkey())
    );
    assert_eq!(d.proposed_by, Some(DisputeParty::Client));
}
