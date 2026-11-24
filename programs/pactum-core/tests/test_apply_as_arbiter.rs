mod common;

use {
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::{
        constants::ARBITER_APP_SEED,
        state::{ArbiterApplication, DisputeMode},
    },
    solana_account::Account,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn apply_as_arbiter_stores_application_and_rent_payer() {
    let sas_program = Pubkey::new_unique();
    let admin = Keypair::new();
    let arbiter = Keypair::new();
    let payer = Keypair::new();
    let (mut svm, global_config_pda) = common::setup_svm(sas_program, admin.pubkey());
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let market_pda = common::create_marketplace_account(
        &mut svm,
        1,
        Pubkey::new_unique(),
        DisputeMode::ArbiterOnly,
        false,
        &payer,
    );
    let sas_kyb = Pubkey::new_unique();
    svm.set_account(
        sas_kyb,
        Account {
            lamports: 1_000_000,
            data: vec![4, 5, 6],
            owner: sas_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (app_pda, _) = Pubkey::find_program_address(
        &[
            ARBITER_APP_SEED.as_bytes(),
            market_pda.as_ref(),
            arbiter.pubkey().as_ref(),
        ],
        &pactum_core::id(),
    );
    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
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
    common::send_ix(&mut svm, ix, &payer, &[&payer, &arbiter]).unwrap();

    let app = ArbiterApplication::try_deserialize(
        &mut svm.get_account(&app_pda).unwrap().data.as_slice(),
    )
    .unwrap();
    assert_eq!(app.rent_payer, payer.pubkey());
    assert_eq!(app.arbiter_fee, 5_000_000);
}
