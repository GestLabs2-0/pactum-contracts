mod common;

use {
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::{
        constants::IDENTITY_SEED,
        state::{Attestation, Identity, UserMainAcc, ATTESTATION_DISCRIMINATOR},
    },
    solana_account::Account,
    solana_keypair::Keypair,
    solana_sha256_hasher::hash,
    solana_signer::Signer,
};

#[test]
fn link_sas_identity_succeeds_and_creates_identity() {
    let sas_program = Pubkey::new_unique();
    let signer = Keypair::new();
    let admin = Keypair::new();
    let user = Keypair::new();
    let (mut svm, global_config_pda, gc) = common::setup_svm(sas_program, admin.pubkey());
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let user_main_pda = common::create_user_main(&mut svm, &user, &user);
    let raw_data = [9u8; 32];
    let id_hash = hash(&raw_data).to_bytes();
    let sas_attestation = common::seed_sas_credential_and_attestation(
        &mut svm,
        sas_program,
        gc.sas_credential,
        signer.pubkey(),
        user.pubkey(),
        raw_data,
        0,
    );
    let (identity_pda, _) = Pubkey::find_program_address(
        &[IDENTITY_SEED.as_bytes(), id_hash.as_ref()],
        &pactum_core::id(),
    );

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::LinkSasIdentity {}.data(),
        pactum_core::accounts::LinkSasIdentity {
            user_main: user_main_pda,
            identity: identity_pda,
            authority: user.pubkey(),
            payer: user.pubkey(),
            global_config: global_config_pda,
            sas_attestation,
            sas_credential: gc.sas_credential,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &user, &[&user]).unwrap();

    let acc = svm.get_account(&user_main_pda).unwrap();
    let user_main = UserMainAcc::try_deserialize(&mut acc.data.as_slice()).unwrap();
    assert_eq!(user_main.sas_attestation, Some(sas_attestation));

    let id_acc = svm.get_account(&identity_pda).unwrap();
    let identity = Identity::try_deserialize(&mut id_acc.data.as_slice()).unwrap();
    assert_eq!(identity.authority, user.pubkey());
    assert!(identity.linked);
}

#[test]
fn link_sas_identity_rejects_duplicate_id_hash_on_second_wallet() {
    let sas_program = Pubkey::new_unique();
    let signer = Keypair::new();
    let admin = Keypair::new();
    let wallet_a = Keypair::new();
    let wallet_b = Keypair::new();
    let (mut svm, global_config_pda, gc) = common::setup_svm(sas_program, admin.pubkey());
    svm.airdrop(&wallet_a.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&wallet_b.pubkey(), 10_000_000_000).unwrap();

    let raw_data = [7u8; 32];
    let id_hash = hash(&raw_data).to_bytes();
    let (identity_pda, _) = Pubkey::find_program_address(
        &[IDENTITY_SEED.as_bytes(), id_hash.as_ref()],
        &pactum_core::id(),
    );

    let um_a = common::create_user_main(&mut svm, &wallet_a, &wallet_a);
    let att_a = common::seed_sas_credential_and_attestation(
        &mut svm,
        sas_program,
        gc.sas_credential,
        signer.pubkey(),
        wallet_a.pubkey(),
        raw_data,
        0,
    );
    let ix_a = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::LinkSasIdentity {}.data(),
        pactum_core::accounts::LinkSasIdentity {
            user_main: um_a,
            identity: identity_pda,
            authority: wallet_a.pubkey(),
            payer: wallet_a.pubkey(),
            global_config: global_config_pda,
            sas_attestation: att_a,
            sas_credential: gc.sas_credential,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix_a, &wallet_a, &[&wallet_a]).unwrap();

    let um_b = common::create_user_main(&mut svm, &wallet_b, &wallet_b);
    let att_b = common::seed_sas_credential_and_attestation(
        &mut svm,
        sas_program,
        gc.sas_credential,
        signer.pubkey(),
        wallet_b.pubkey(),
        raw_data,
        0,
    );
    let ix_b = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::LinkSasIdentity {}.data(),
        pactum_core::accounts::LinkSasIdentity {
            user_main: um_b,
            identity: identity_pda,
            authority: wallet_b.pubkey(),
            payer: wallet_b.pubkey(),
            global_config: global_config_pda,
            sas_attestation: att_b,
            sas_credential: gc.sas_credential,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix_b, &wallet_b, &[&wallet_b]).is_err());
}

#[test]
fn link_sas_identity_rejects_wrong_program_owner() {
    let sas_program = Pubkey::new_unique();
    let wrong_program = Pubkey::new_unique();
    let admin = Keypair::new();
    let user = Keypair::new();
    let (mut svm, global_config_pda, gc) = common::setup_svm(sas_program, admin.pubkey());
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let user_main_pda = common::create_user_main(&mut svm, &user, &user);
    let id_hash = [5u8; 32];
    let (identity_pda, _) = Pubkey::find_program_address(
        &[IDENTITY_SEED.as_bytes(), id_hash.as_ref()],
        &pactum_core::id(),
    );
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
            identity: identity_pda,
            authority: user.pubkey(),
            payer: user.pubkey(),
            global_config: global_config_pda,
            sas_attestation: fake_attestation,
            sas_credential: gc.sas_credential,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &user, &[&user]).is_err());
}

#[test]
fn link_sas_identity_rejects_expired_attestation() {
    let sas_program = Pubkey::new_unique();
    let signer = Keypair::new();
    let admin = Keypair::new();
    let user = Keypair::new();
    let (mut svm, global_config_pda, gc) = common::setup_svm(sas_program, admin.pubkey());
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let user_main_pda = common::create_user_main(&mut svm, &user, &user);
    let id_hash = [3u8; 32];
    let expired_attestation = common::seed_sas_credential_and_attestation(
        &mut svm,
        sas_program,
        gc.sas_credential,
        signer.pubkey(),
        user.pubkey(),
        id_hash,
        -1,
    );
    let (identity_pda, _) = Pubkey::find_program_address(
        &[IDENTITY_SEED.as_bytes(), id_hash.as_ref()],
        &pactum_core::id(),
    );

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::LinkSasIdentity {}.data(),
        pactum_core::accounts::LinkSasIdentity {
            user_main: user_main_pda,
            identity: identity_pda,
            authority: user.pubkey(),
            payer: user.pubkey(),
            global_config: global_config_pda,
            sas_attestation: expired_attestation,
            sas_credential: gc.sas_credential,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &user, &[&user]).is_err());
}

#[test]
fn link_sas_identity_rejects_unauthorized_signer() {
    let sas_program = Pubkey::new_unique();
    let authorized_signer = Keypair::new();
    let rogue_signer = Keypair::new();
    let admin = Keypair::new();
    let user = Keypair::new();
    let (mut svm, global_config_pda, gc) = common::setup_svm(sas_program, admin.pubkey());
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let user_main_pda = common::create_user_main(&mut svm, &user, &user);
    let id_hash = [4u8; 32];
    let attestation = common::seed_sas_credential_and_attestation(
        &mut svm,
        sas_program,
        gc.sas_credential,
        authorized_signer.pubkey(),
        user.pubkey(),
        id_hash,
        0,
    );
    let att = Attestation {
        nonce: user.pubkey(),
        credential: gc.sas_credential,
        schema: Pubkey::new_unique(),
        data: id_hash.to_vec(),
        signer: rogue_signer.pubkey(),
        expiry: 0,
        token_account: Pubkey::default(),
    };
    svm.set_account(
        attestation,
        Account {
            lamports: 1_000_000,
            data: common::serialize_sas_acc(ATTESTATION_DISCRIMINATOR, &att),
            owner: sas_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    let (identity_pda, _) = Pubkey::find_program_address(
        &[IDENTITY_SEED.as_bytes(), id_hash.as_ref()],
        &pactum_core::id(),
    );

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::LinkSasIdentity {}.data(),
        pactum_core::accounts::LinkSasIdentity {
            user_main: user_main_pda,
            identity: identity_pda,
            authority: user.pubkey(),
            payer: user.pubkey(),
            global_config: global_config_pda,
            sas_attestation: attestation,
            sas_credential: gc.sas_credential,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &user, &[&user]).is_err());
}
