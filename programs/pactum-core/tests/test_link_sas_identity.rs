mod common;

use {
    anchor_lang::{
        prelude::{AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::state::UserMainAcc,
    solana_account::Account,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn link_sas_identity_succeeds_for_valid_sas_program() {
    let sas_program = Pubkey::new_unique();
    let admin = Keypair::new();
    let user = Keypair::new();
    let (mut svm, global_config_pda) = common::setup_svm(sas_program, admin.pubkey());
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let user_main_pda = common::create_user_main(&mut svm, &user, &user);
    let sas_attestation = Pubkey::new_unique();
    svm.set_account(
        sas_attestation,
        Account {
            lamports: 1_000_000,
            data: vec![1, 2, 3],
            owner: sas_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::LinkSasIdentity {}.data(),
        pactum_core::accounts::LinkSasIdentity {
            user_main: user_main_pda,
            authority: user.pubkey(),
            global_config: global_config_pda,
            sas_attestation,
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &user, &[&user]).unwrap();

    let acc = svm.get_account(&user_main_pda).unwrap();
    let user_main = UserMainAcc::try_deserialize(&mut acc.data.as_slice()).unwrap();
    assert_eq!(user_main.sas_attestation, Some(sas_attestation));
}

#[test]
fn link_sas_identity_rejects_wrong_program_owner() {
    let sas_program = Pubkey::new_unique();
    let wrong_program = Pubkey::new_unique();
    let admin = Keypair::new();
    let user = Keypair::new();
    let (mut svm, global_config_pda) = common::setup_svm(sas_program, admin.pubkey());
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let user_main_pda = common::create_user_main(&mut svm, &user, &user);
    let fake_attestation = Pubkey::new_unique();
    svm.set_account(
        fake_attestation,
        Account {
            lamports: 1_000_000,
            data: vec![1, 2, 3],
            owner: wrong_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::LinkSasIdentity {}.data(),
        pactum_core::accounts::LinkSasIdentity {
            user_main: user_main_pda,
            authority: user.pubkey(),
            global_config: global_config_pda,
            sas_attestation: fake_attestation,
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &user, &[&user]).is_err());
}
