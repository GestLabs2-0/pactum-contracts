mod common;

use {
    anchor_lang::{
        prelude::{AccountMeta, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    common::{
        create_token_account, deserialize_account, dispute_pda, judge_vault_pda,
        seed_agreement_with_vault, seed_client, seed_judge, seed_marketplace, seed_offer,
        seed_provider, seed_treasury_vault, send_ixs, serialize_account, set_cu_limit_ix,
        setup_svm, token_account_amount, vote_record_pda, warp_to_ts,
    },
    pactum_core::state::{DisputeMode as MarketDisputeMode, DisputeParty, Judge},
    pactum_disputes::{
        constants::{
            CPI_AUTHORITY_SEED, DISPUTE_SEED, JURY_PANEL_SIZE, ROUND_INITIAL, ROUND_RUNOFF,
        },
        state::{Dispute, DisputeMode, DisputeOutcome, DisputeStatus, VoteChoice, VoteRecord},
    },
    pactum_escrow::constants::CPI_AUTHORITY_SEED as ESCROW_CPI_SEED,
    pactum_treasury::constants::{CPI_AUTHORITY_SEED as TREASURY_CPI_SEED, JUDGE_VAULT_SEED},
    solana_account::Account,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

fn seed_voting_dispute(
    svm: &mut litesvm::LiteSVM,
    agreement: Pubkey,
    marketplace: Pubkey,
    parties: (Pubkey, Pubkey, Pubkey),
    leaves: [u32; JURY_PANEL_SIZE],
    panel_size: u8,
    phase_started_at: i64,
) -> Pubkey {
    let (client, provider, rent_payer) = parties;
    let (dispute, bump) = Pubkey::find_program_address(
        &[DISPUTE_SEED.as_bytes(), agreement.as_ref()],
        &pactum_disputes::id(),
    );
    let acc = Dispute {
        agreement,
        marketplace,
        client,
        provider,
        opened_by: DisputeParty::Client,
        mode: DisputeMode::DecentralizedJury,
        assigned_arbiter: None,
        proposed_replacement_arbiter: None,
        proposed_by: None,
        selected_leaves: leaves,
        panel_size,
        round: ROUND_INITIAL,
        runoff_choices: None,
        client_evidence: None,
        provider_evidence: None,
        status: DisputeStatus::Voting,
        outcome: None,
        is_reservation: false,
        created_at: 100,
        phase_started_at,
        sla_deadline: 0,
        rent_payer,
        bump,
    };
    use anchor_lang::Space;
    let mut data = serialize_account(&acc);
    data.resize(8 + Dispute::INIT_SPACE, 0);
    svm.set_account(
        dispute,
        Account {
            lamports: 1_000_000_000,
            data,
            owner: pactum_disputes::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    dispute
}

fn seed_vote_record(
    svm: &mut litesvm::LiteSVM,
    dispute: Pubkey,
    round: u8,
    leaf_index: u32,
    choice: VoteChoice,
    rent_payer: Pubkey,
) -> Pubkey {
    let vr_pda = vote_record_pda(&dispute, round, leaf_index);
    let bump = Pubkey::find_program_address(
        &[
            pactum_disputes::constants::VOTE_RECORD_SEED.as_bytes(),
            dispute.as_ref(),
            &[round],
            &leaf_index.to_le_bytes(),
        ],
        &pactum_disputes::id(),
    )
    .1;
    let acc = VoteRecord {
        dispute,
        round,
        leaf_index,
        commit_hash: [1u8; 32],
        revealed_choice: Some(choice),
        has_revealed: true,
        rent_payer,
        bump,
    };
    svm.set_account(
        vr_pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&acc),
            owner: pactum_disputes::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    vr_pda
}

#[test]
fn tally_or_trigger_runoff_settles_majority_and_slashes_outlier() {
    let mut env = setup_svm(true);
    let client_kp = Keypair::new();
    let provider_kp = Keypair::new();
    let j1_kp = Keypair::new();
    let j2_kp = Keypair::new();
    let j3_kp = Keypair::new();
    env.svm.airdrop(&client_kp.pubkey(), 1_000_000_000).unwrap();

    let market = seed_marketplace(&mut env.svm, 1, MarketDisputeMode::DecentralizedJury);
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
    // Freeze agreement status to Disputed
    let mut ag: pactum_escrow::state::Agreement = deserialize_account(&env.svm, &agreement);
    ag.status = pactum_escrow::state::AgreementStatus::Disputed;
    env.svm
        .set_account(
            agreement,
            Account {
                lamports: 1_000_000_000,
                data: serialize_account(&ag),
                owner: pactum_escrow::id(),
                executable: false,
                rent_epoch: 0,
            },
        )
        .unwrap();

    let j1 = seed_judge(&mut env.svm, &market, &j1_kp.pubkey(), 10_000_000, 0, 1);
    let j2 = seed_judge(&mut env.svm, &market, &j2_kp.pubkey(), 10_000_000, 0, 2);
    let j3 = seed_judge(&mut env.svm, &market, &j3_kp.pubkey(), 10_000_000, 0, 3);
    seed_treasury_vault(
        &mut env.svm,
        JUDGE_VAULT_SEED,
        &market,
        &env.usdc_mint,
        30_000_000,
    );

    let dispute = seed_voting_dispute(
        &mut env.svm,
        agreement,
        market,
        (client, provider, client_kp.pubkey()),
        [1, 2, 3, 0, 0],
        3,
        1_000,
    );
    let vr1 = seed_vote_record(
        &mut env.svm,
        dispute,
        1,
        1,
        VoteChoice::ClientWins,
        j1_kp.pubkey(),
    );
    let vr2 = seed_vote_record(
        &mut env.svm,
        dispute,
        1,
        2,
        VoteChoice::ClientWins,
        j2_kp.pubkey(),
    );
    let vr3 = seed_vote_record(
        &mut env.svm,
        dispute,
        1,
        3,
        VoteChoice::ProviderWins,
        j3_kp.pubkey(),
    );

    let (cpi_authority, _) =
        Pubkey::find_program_address(&[CPI_AUTHORITY_SEED.as_bytes()], &pactum_disputes::id());
    let (escrow_cpi_authority, _) =
        Pubkey::find_program_address(&[ESCROW_CPI_SEED.as_bytes()], &pactum_escrow::id());
    let (treasury_cpi_authority, _) =
        Pubkey::find_program_address(&[TREASURY_CPI_SEED.as_bytes()], &pactum_treasury::id());

    let client_ta = create_token_account(&mut env.svm, &env.usdc_mint, &client_kp.pubkey(), 0);
    let provider_ta = create_token_account(&mut env.svm, &env.usdc_mint, &provider_kp.pubkey(), 0);
    let fee_dest_ta =
        create_token_account(&mut env.svm, &env.usdc_mint, &env.authority.pubkey(), 0);

    warp_to_ts(&mut env.svm, 1_000 + 172_800 + 86_400);

    let mut metas = pactum_disputes::accounts::TallyOrTriggerRunoff {
        dispute,
        dispute_config: env.dispute_config,
        marketplace: market,
        client,
        provider,
        agreement: Some(agreement),
        reservation: None,
        escrow_vault: vault,
        client_token_account: client_ta,
        provider_token_account: provider_ta,
        platform_vault: env.platform_vault,
        fee_destination: fee_dest_ta,
        judge_stake_vault: judge_vault_pda(&market),
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
    .to_account_metas(None);

    for (j, vr) in [(j1, vr1), (j2, vr2), (j3, vr3)] {
        metas.push(AccountMeta::new(j, false));
        metas.push(AccountMeta::new_readonly(vr, false));
    }

    let ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::TallyOrTriggerRunoff {}.data(),
        metas,
    );
    send_ixs(
        &mut env.svm,
        &[set_cu_limit_ix(600_000), ix],
        &client_kp,
        &[&client_kp],
    )
    .unwrap();

    assert_eq!(token_account_amount(&env.svm, &client_ta), 13_000_000);
    assert_eq!(token_account_amount(&env.svm, &provider_ta), 1_000_000);
    // 2M jury fee in fee_dest_ta, 500k slash on j3 in platform_vault
    assert_eq!(token_account_amount(&env.svm, &fee_dest_ta), 2_000_000);
    assert_eq!(token_account_amount(&env.svm, &env.platform_vault), 500_000);

    let d: Dispute = deserialize_account(&env.svm, &dispute);
    assert_eq!(d.status, DisputeStatus::Resolved);
    assert_eq!(d.outcome, Some(DisputeOutcome::ClientWins));

    let judge1: Judge = deserialize_account(&env.svm, &j1);
    let judge3: Judge = deserialize_account(&env.svm, &j3);
    assert_eq!(judge1.cases_voted, 1);
    assert_eq!(judge1.cases_in_consensus, 1);
    assert_eq!(judge1.active_disputes, 0);
    assert_eq!(judge3.cases_voted, 1);
    assert_eq!(judge3.cases_in_consensus, 0);
    assert_eq!(judge3.staked_amount, 9_500_000);
}

#[test]
fn tally_or_trigger_runoff_triggers_round_2_on_round_1_tie() {
    let mut env = setup_svm(true);
    let client_kp = Keypair::new();
    let provider_kp = Keypair::new();
    let j1_kp = Keypair::new();
    let j2_kp = Keypair::new();
    env.svm.airdrop(&client_kp.pubkey(), 1_000_000_000).unwrap();

    let market = seed_marketplace(&mut env.svm, 2, MarketDisputeMode::DecentralizedJury);
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

    let j1 = seed_judge(&mut env.svm, &market, &j1_kp.pubkey(), 10_000_000, 0, 1);
    let j2 = seed_judge(&mut env.svm, &market, &j2_kp.pubkey(), 10_000_000, 0, 2);

    let dispute = seed_voting_dispute(
        &mut env.svm,
        agreement,
        market,
        (client, provider, client_kp.pubkey()),
        [1, 2, 0, 0, 0],
        2,
        1_000,
    );
    let vr1 = seed_vote_record(
        &mut env.svm,
        dispute,
        1,
        1,
        VoteChoice::ClientWins,
        j1_kp.pubkey(),
    );
    let vr2 = seed_vote_record(
        &mut env.svm,
        dispute,
        1,
        2,
        VoteChoice::ProviderWins,
        j2_kp.pubkey(),
    );

    let (cpi_authority, _) =
        Pubkey::find_program_address(&[CPI_AUTHORITY_SEED.as_bytes()], &pactum_disputes::id());
    let (escrow_cpi_authority, _) =
        Pubkey::find_program_address(&[ESCROW_CPI_SEED.as_bytes()], &pactum_escrow::id());
    let (treasury_cpi_authority, _) =
        Pubkey::find_program_address(&[TREASURY_CPI_SEED.as_bytes()], &pactum_treasury::id());

    let client_ta = create_token_account(&mut env.svm, &env.usdc_mint, &client_kp.pubkey(), 0);
    let provider_ta = create_token_account(&mut env.svm, &env.usdc_mint, &provider_kp.pubkey(), 0);
    let fee_dest_ta =
        create_token_account(&mut env.svm, &env.usdc_mint, &env.authority.pubkey(), 0);

    warp_to_ts(&mut env.svm, 1_000 + 172_800 + 86_400);

    let mut metas = pactum_disputes::accounts::TallyOrTriggerRunoff {
        dispute,
        dispute_config: env.dispute_config,
        marketplace: market,
        client,
        provider,
        agreement: Some(agreement),
        reservation: None,
        escrow_vault: vault,
        client_token_account: client_ta,
        provider_token_account: provider_ta,
        platform_vault: env.platform_vault,
        fee_destination: fee_dest_ta,
        judge_stake_vault: judge_vault_pda(&market),
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
    .to_account_metas(None);

    for (j, vr) in [(j1, vr1), (j2, vr2)] {
        metas.push(AccountMeta::new(j, false));
        metas.push(AccountMeta::new_readonly(vr, false));
    }

    let ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::TallyOrTriggerRunoff {}.data(),
        metas,
    );
    send_ixs(
        &mut env.svm,
        &[set_cu_limit_ix(400_000), ix],
        &client_kp,
        &[&client_kp],
    )
    .unwrap();

    let d: Dispute = deserialize_account(&env.svm, &dispute_pda(&agreement));
    assert_eq!(d.status, DisputeStatus::Voting);
    assert_eq!(d.round, ROUND_RUNOFF);
    assert!(d.runoff_choices.is_some());
}
