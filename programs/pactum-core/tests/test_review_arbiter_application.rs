mod common;

use {
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::{
        constants::{ARBITER_APP_SEED, ARBITER_SEED},
        state::{ArbiterProfile, DisputeMode},
    },
    solana_account::Account,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

fn setup_application(
    svm: &mut litesvm::LiteSVM,
    global_config_pda: Pubkey,
    sas_program: Pubkey,
    committee: &Keypair,
    arbiter: &Keypair,
    payer: &Keypair,
    market_id: u64,
) -> (Pubkey, Pubkey, Pubkey) {
    let market_pda = common::create_marketplace_account(
        svm,
        market_id,
        committee.pubkey(),
        DisputeMode::ArbiterOnly,
        false,
        payer,
    );
    let sas_kyb = Pubkey::new_unique();
    svm.set_account(
        sas_kyb,
        Account {
            lamports: 1_000_000,
            data: vec![1],
            owner: sas_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let core_id = pactum_core::id();
    let (app_pda, _) = Pubkey::find_program_address(
        &[
            ARBITER_APP_SEED.as_bytes(),
            market_pda.as_ref(),
            arbiter.pubkey().as_ref(),
        ],
        &core_id,
    );
    let (profile_pda, _) = Pubkey::find_program_address(
        &[
            ARBITER_SEED.as_bytes(),
            market_pda.as_ref(),
            arbiter.pubkey().as_ref(),
        ],
        &core_id,
    );

    let ix = Instruction::new_with_bytes(
        core_id,
        &pactum_core::instruction::ApplyAsArbiter {
            arbiter_fee: 5_000_000,
            sla_secs: 172_800,
        }
        .data(),
        pactum_core::accounts::ApplyAsArbiter {
            arbiter_application: app_pda,
            marketplace: market_pda,
            global_config: global_config_pda,
            sas_kyb_attestation: sas_kyb,
            authority: arbiter.pubkey(),
            payer: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(svm, ix, payer, &[payer, arbiter]).unwrap();
    (market_pda, app_pda, profile_pda)
}

#[test]
fn review_arbiter_approve_creates_profile_and_closes_application() {
    let sas_program = Pubkey::new_unique();
    let admin = Keypair::new();
    let committee = Keypair::new();
    let arbiter = Keypair::new();
    let payer = Keypair::new();
    let (mut svm, global_config_pda) = common::setup_svm(sas_program, admin.pubkey());
    svm.airdrop(&committee.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let (market_pda, app_pda, profile_pda) = setup_application(
        &mut svm,
        global_config_pda,
        sas_program,
        &committee,
        &arbiter,
        &payer,
        1,
    );

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::ReviewArbiterApplication { approve: true }.data(),
        pactum_core::accounts::ReviewArbiterApplication {
            arbiter_application: app_pda,
            marketplace: market_pda,
            committee: committee.pubkey(),
            arbiter_profile: profile_pda,
            rent_payer: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &committee, &[&committee]).unwrap();

    assert!(svm.get_account(&app_pda).is_none());
    let profile = ArbiterProfile::try_deserialize(
        &mut svm.get_account(&profile_pda).unwrap().data.as_slice(),
    )
    .unwrap();
    assert!(profile.is_active);
    assert_eq!(profile.arbiter_fee, 5_000_000);
}

#[test]
fn review_arbiter_reject_closes_application_without_profile() {
    let sas_program = Pubkey::new_unique();
    let admin = Keypair::new();
    let committee = Keypair::new();
    let arbiter = Keypair::new();
    let payer = Keypair::new();
    let (mut svm, global_config_pda) = common::setup_svm(sas_program, admin.pubkey());
    svm.airdrop(&committee.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let (market_pda, app_pda, profile_pda) = setup_application(
        &mut svm,
        global_config_pda,
        sas_program,
        &committee,
        &arbiter,
        &payer,
        2,
    );

    let payer_before = svm.get_account(&payer.pubkey()).unwrap().lamports;
    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::ReviewArbiterApplication { approve: false }.data(),
        pactum_core::accounts::ReviewArbiterApplication {
            arbiter_application: app_pda,
            marketplace: market_pda,
            committee: committee.pubkey(),
            arbiter_profile: profile_pda,
            rent_payer: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &committee, &[&committee]).unwrap();

    assert!(svm.get_account(&app_pda).is_none());
    assert!(svm.get_account(&profile_pda).is_none());
    assert!(svm.get_account(&payer.pubkey()).unwrap().lamports > payer_before);
}
