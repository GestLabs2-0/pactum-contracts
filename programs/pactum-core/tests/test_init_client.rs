mod common;

use {
    crate::common::serialize_sas_acc,
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::{
        constants::{CLIENT_SEED, IDENTITY_SEED},
        state::{Client, DisputeMode},
        Attestation, Credential, ATTESTATION_DISCRIMINATOR, CREDENTIAL_DISCRIMINATOR,
    },
    solana_account::Account,
    solana_keypair::Keypair,
    solana_sha256_hasher::hash,
    solana_signer::Signer,
};

#[test]
fn init_client_requires_sas_when_marketplace_requires_identity() {
    let admin = Keypair::new();
    let user = Keypair::new();
    let payer = Keypair::new();
    let (mut svm, _, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let user_main_pda = common::create_user_main(&mut svm, &user, &payer);
    let market_pda = common::create_marketplace_account(
        &mut svm,
        1,
        Pubkey::new_unique(),
        DisputeMode::ArbiterOnly,
        true,
        &payer,
    );
    let (client_pda, _) = Pubkey::find_program_address(
        &[
            CLIENT_SEED.as_bytes(),
            market_pda.as_ref(),
            user_main_pda.as_ref(),
        ],
        &pactum_core::id(),
    );

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::InitClient {}.data(),
        pactum_core::accounts::InitClient {
            client: client_pda,
            marketplace: market_pda,
            user_main: user_main_pda,
            authority: user.pubkey(),
            payer: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &payer, &[&payer, &user]).is_err());
}

#[test]
fn init_client_succeeds_after_linking_sas() {
    let sas_program = Pubkey::new_unique();
    let admin = Keypair::new();
    let user = Keypair::new();
    let signer = Pubkey::new_unique();
    let (mut svm, global_config_pda, gc) = common::setup_svm(sas_program, admin.pubkey());
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let credential = Credential {
        authority: signer,
        authorized_signers: vec![signer],
        name: vec![],
    };

    let raw_id_data = [8u8; 32];
    let id_hash = hash(&raw_id_data).to_bytes();
    let attestation = Attestation {
        credential: gc.sas_credential,
        data: raw_id_data.to_vec(),
        expiry: 0,
        nonce: user.pubkey(),
        schema: Pubkey::new_unique(),
        signer,
        token_account: Pubkey::new_unique(),
    };

    let user_main_pda = common::create_user_main(&mut svm, &user, &user);
    let market_pda = common::create_marketplace_account(
        &mut svm,
        2,
        Pubkey::new_unique(),
        DisputeMode::ArbiterOnly,
        true,
        &user,
    );

    let sas_attestation = Pubkey::new_unique();
    svm.set_account(
        sas_attestation,
        Account {
            lamports: 1_000_000,
            data: serialize_sas_acc(ATTESTATION_DISCRIMINATOR, &attestation),
            owner: sas_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    svm.set_account(
        gc.sas_credential,
        Account {
            lamports: 1_000_000,
            data: serialize_sas_acc(CREDENTIAL_DISCRIMINATOR, &credential),
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
    let link_ix = Instruction::new_with_bytes(
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
    common::send_ix(&mut svm, link_ix, &user, &[&user]).unwrap();

    let (client_pda, _) = Pubkey::find_program_address(
        &[
            CLIENT_SEED.as_bytes(),
            market_pda.as_ref(),
            user_main_pda.as_ref(),
        ],
        &pactum_core::id(),
    );
    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::InitClient {}.data(),
        pactum_core::accounts::InitClient {
            client: client_pda,
            marketplace: market_pda,
            user_main: user_main_pda,
            authority: user.pubkey(),
            payer: user.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &user, &[&user]).unwrap();

    let acc = svm.get_account(&client_pda).unwrap();
    let client = Client::try_deserialize(&mut acc.data.as_slice()).unwrap();
    assert!(!client.is_banned);
}
