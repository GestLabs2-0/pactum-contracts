mod common;

use {
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        AnchorSerialize, InstructionData, ToAccountMetas,
    },
    pactum_core::{
        constants::USER_MAIN_SEED,
        state::{Attestation, UserMainAcc, ATTESTATION_DISCRIMINATOR},
    },
    solana_account::Account,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

fn serialize_attestation(att: &Attestation) -> Vec<u8> {
    let mut out = vec![ATTESTATION_DISCRIMINATOR];
    att.serialize(&mut out).unwrap();
    out
}

fn seed_linked_user_main(
    svm: &mut litesvm::LiteSVM,
    user: &Keypair,
    sas_attestation: Pubkey,
) -> Pubkey {
    let core_id = pactum_core::id();
    let (user_main_pda, bump) = Pubkey::find_program_address(
        &[USER_MAIN_SEED.as_bytes(), user.pubkey().as_ref()],
        &core_id,
    );
    let user_main = UserMainAcc {
        authority: user.pubkey(),
        sas_attestation: Some(sas_attestation),
        bump,
    };
    svm.set_account(
        user_main_pda,
        Account {
            lamports: 1_000_000_000,
            data: common::serialize_account(&user_main),
            owner: core_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    user_main_pda
}

#[test]
fn unlink_sas_identity_succeeds_when_attestation_closed() {
    let sas_program = Pubkey::new_unique();
    let admin = Keypair::new();
    let user = Keypair::new();
    let (mut svm, global_config_pda, _) = common::setup_svm(sas_program, admin.pubkey());
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let closed_attestation = Pubkey::new_unique();
    svm.set_account(
        closed_attestation,
        Account {
            lamports: 0,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    let user_main_pda = seed_linked_user_main(&mut svm, &user, closed_attestation);

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::UnlinkSasIdentity {}.data(),
        pactum_core::accounts::UnlinkSasIdentity {
            user_main: user_main_pda,
            authority: user.pubkey(),
            global_config: global_config_pda,
            sas_attestation: closed_attestation,
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &user, &[&user]).unwrap();

    let acc = svm.get_account(&user_main_pda).unwrap();
    let after = UserMainAcc::try_deserialize(&mut acc.data.as_slice()).unwrap();
    assert_eq!(after.sas_attestation, None);
}

#[test]
fn unlink_sas_identity_succeeds_when_attestation_expired() {
    let sas_program = Pubkey::new_unique();
    let admin = Keypair::new();
    let user = Keypair::new();
    let (mut svm, global_config_pda, _) = common::setup_svm(sas_program, admin.pubkey());
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let expired_attestation = Pubkey::new_unique();
    let att = Attestation {
        nonce: user.pubkey(),
        credential: Pubkey::new_unique(),
        schema: Pubkey::new_unique(),
        data: vec![],
        signer: Pubkey::new_unique(),
        expiry: -1,
        token_account: Pubkey::default(),
    };
    svm.set_account(
        expired_attestation,
        Account {
            lamports: 1_000_000,
            data: serialize_attestation(&att),
            owner: sas_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    let user_main_pda = seed_linked_user_main(&mut svm, &user, expired_attestation);

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::UnlinkSasIdentity {}.data(),
        pactum_core::accounts::UnlinkSasIdentity {
            user_main: user_main_pda,
            authority: user.pubkey(),
            global_config: global_config_pda,
            sas_attestation: expired_attestation,
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &user, &[&user]).unwrap();

    let acc = svm.get_account(&user_main_pda).unwrap();
    let after = UserMainAcc::try_deserialize(&mut acc.data.as_slice()).unwrap();
    assert_eq!(after.sas_attestation, None);
}

#[test]
fn unlink_sas_identity_rejects_active_attestation() {
    let sas_program = Pubkey::new_unique();
    let admin = Keypair::new();
    let user = Keypair::new();
    let (mut svm, global_config_pda, _) = common::setup_svm(sas_program, admin.pubkey());
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let active_attestation = Pubkey::new_unique();
    let att = Attestation {
        nonce: user.pubkey(),
        credential: Pubkey::new_unique(),
        schema: Pubkey::new_unique(),
        data: vec![],
        signer: Pubkey::new_unique(),
        expiry: 0,
        token_account: Pubkey::default(),
    };
    svm.set_account(
        active_attestation,
        Account {
            lamports: 1_000_000,
            data: serialize_attestation(&att),
            owner: sas_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    let user_main_pda = seed_linked_user_main(&mut svm, &user, active_attestation);

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::UnlinkSasIdentity {}.data(),
        pactum_core::accounts::UnlinkSasIdentity {
            user_main: user_main_pda,
            authority: user.pubkey(),
            global_config: global_config_pda,
            sas_attestation: active_attestation,
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &user, &[&user]).is_err());
}
