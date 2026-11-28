mod common;

use {
    anchor_lang::{
        prelude::{AccountMeta, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    common::{dispute_pda, send_ix, serialize_account, setup_svm, vote_record_pda},
    pactum_core::state::DisputeParty,
    pactum_disputes::{
        constants::{DISPUTE_SEED, JURY_PANEL_SIZE, VOTE_RECORD_SEED},
        state::{Dispute, DisputeMode, DisputeOutcome, DisputeStatus, VoteChoice, VoteRecord},
    },
    solana_account::Account,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn close_dispute_closes_dispute_and_vote_records_to_respective_rent_payers() {
    let mut env = setup_svm(true);
    let dispute_payer = Keypair::new();
    let vote_payer = Keypair::new();
    env.svm
        .airdrop(&dispute_payer.pubkey(), 1_000_000_000)
        .unwrap();

    let agreement = Pubkey::new_unique();
    let (dispute, bump) = Pubkey::find_program_address(
        &[DISPUTE_SEED.as_bytes(), agreement.as_ref()],
        &pactum_disputes::id(),
    );
    let d = Dispute {
        agreement,
        marketplace: Pubkey::new_unique(),
        client: Pubkey::new_unique(),
        provider: Pubkey::new_unique(),
        opened_by: DisputeParty::Client,
        mode: DisputeMode::DecentralizedJury,
        assigned_arbiter: None,
        proposed_replacement_arbiter: None,
        proposed_by: None,
        selected_leaves: [1, 0, 0, 0, 0],
        panel_size: 1,
        round: 1,
        runoff_choices: None,
        client_evidence: None,
        provider_evidence: None,
        status: DisputeStatus::Resolved,
        outcome: Some(DisputeOutcome::ClientWins),
        is_reservation: false,
        created_at: 100,
        phase_started_at: 100,
        sla_deadline: 0,
        rent_payer: dispute_payer.pubkey(),
        bump,
    };
    let dispute_lamports = 5_000_000u64;
    env.svm
        .set_account(
            dispute,
            Account {
                lamports: dispute_lamports,
                data: serialize_account(&d),
                owner: pactum_disputes::id(),
                executable: false,
                rent_epoch: 0,
            },
        )
        .unwrap();

    let vr_pda = vote_record_pda(&dispute, 1, 1);
    let vr_bump = Pubkey::find_program_address(
        &[
            VOTE_RECORD_SEED.as_bytes(),
            dispute.as_ref(),
            &[1],
            &1u32.to_le_bytes(),
        ],
        &pactum_disputes::id(),
    )
    .1;
    let vr = VoteRecord {
        dispute,
        round: 1,
        leaf_index: 1,
        commit_hash: [2u8; 32],
        revealed_choice: Some(VoteChoice::ClientWins),
        has_revealed: true,
        rent_payer: vote_payer.pubkey(),
        bump: vr_bump,
    };
    let vr_lamports = 2_000_000u64;
    env.svm
        .set_account(
            vr_pda,
            Account {
                lamports: vr_lamports,
                data: serialize_account(&vr),
                owner: pactum_disputes::id(),
                executable: false,
                rent_epoch: 0,
            },
        )
        .unwrap();

    let before_dispute_payer = env
        .svm
        .get_account(&dispute_payer.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);
    let before_vote_payer = env
        .svm
        .get_account(&vote_payer.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);

    let mut metas = pactum_disputes::accounts::CloseDispute {
        dispute: dispute_pda(&agreement),
        rent_payer: dispute_payer.pubkey(),
    }
    .to_account_metas(None);
    metas.push(AccountMeta::new(vr_pda, false));
    metas.push(AccountMeta::new(vote_payer.pubkey(), false));

    let ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::CloseDispute {}.data(),
        metas,
    );
    send_ix(&mut env.svm, ix, &env.authority, &[&env.authority]).unwrap();

    assert!(env.svm.get_account(&dispute).is_none());
    let vr_after = env.svm.get_account(&vr_pda);
    assert!(vr_after.is_none() || vr_after.unwrap().lamports == 0);

    let after_dispute_payer = env
        .svm
        .get_account(&dispute_payer.pubkey())
        .unwrap()
        .lamports;
    let after_vote_payer = env.svm.get_account(&vote_payer.pubkey()).unwrap().lamports;
    assert_eq!(after_dispute_payer, before_dispute_payer + dispute_lamports);
    assert_eq!(after_vote_payer, before_vote_payer + vr_lamports);
    let _ = JURY_PANEL_SIZE;
}
