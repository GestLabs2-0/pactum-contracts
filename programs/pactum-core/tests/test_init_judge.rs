mod common;

use {
    crate::common::serialize_sas_acc,
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::{
        constants::{IDENTITY_SEED, JUDGE_SEED, MARKETPLACE_SEED},
        instructions::CreateMarketplaceParams,
        state::{
            DisputeMode, IdentityPolicy, IntegratorPolicy, Judge, RoleIdentityConfig,
            StorageBackend,
        },
        Attestation, Credential, ATTESTATION_DISCRIMINATOR, CREDENTIAL_DISCRIMINATOR,
    },
    solana_account::Account,
    solana_keypair::Keypair,
    solana_sha256_hasher::hash,
    solana_signer::Signer,
};

#[test]
fn init_judge_rejected_on_arbiter_only_marketplace() {
    let admin = Keypair::new();
    let user = Keypair::new();
    let payer = Keypair::new();
    let (mut svm, global_config_pda, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let _user_main_pda = common::create_user_main(&mut svm, &user, &payer);
    let market_pda = common::create_marketplace_account(
        &mut svm,
        1,
        Pubkey::new_unique(),
        DisputeMode::ArbiterOnly,
        false,
        &payer,
    );
    let (judge_pda, _) = Pubkey::find_program_address(
        &[
            JUDGE_SEED.as_bytes(),
            market_pda.as_ref(),
            user.pubkey().as_ref(),
        ],
        &pactum_core::id(),
    );

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::InitJudge {}.data(),
        pactum_core::accounts::InitJudge {
            judge: judge_pda,
            marketplace: market_pda,
            user_main: None,
            global_config: global_config_pda,
            marketplace_attestation: None,
            marketplace_credential: None,
            authority: user.pubkey(),
            payer: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &payer, &[&payer, &user]).is_err());
}

#[test]
fn init_judge_succeeds_on_decentralized_jury_marketplace() {
    let admin = Keypair::new();
    let user = Keypair::new();
    let payer = Keypair::new();
    let (mut svm, global_config_pda, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let _user_main_pda = common::create_user_main(&mut svm, &user, &payer);
    let market_pda = common::create_marketplace_account(
        &mut svm,
        2,
        Pubkey::new_unique(),
        DisputeMode::DecentralizedJury,
        false,
        &payer,
    );
    let (judge_pda, _) = Pubkey::find_program_address(
        &[
            JUDGE_SEED.as_bytes(),
            market_pda.as_ref(),
            user.pubkey().as_ref(),
        ],
        &pactum_core::id(),
    );

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::InitJudge {}.data(),
        pactum_core::accounts::InitJudge {
            judge: judge_pda,
            marketplace: market_pda,
            user_main: None,
            global_config: global_config_pda,
            marketplace_attestation: None,
            marketplace_credential: None,
            authority: user.pubkey(),
            payer: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &payer, &[&payer, &user]).unwrap();

    let acc = svm.get_account(&judge_pda).unwrap();
    let judge = Judge::try_deserialize(&mut acc.data.as_slice()).unwrap();
    assert!(!judge.is_active_in_pool);
    assert_eq!(judge.staked_amount, 0);
}

#[test]
fn init_judge_blocks_base_kyc_bypass_and_requires_judge_credential() {
    let sas_program = Pubkey::new_unique();
    let admin = Keypair::new();
    let user = Keypair::new();
    let payer = Keypair::new();
    let base_signer = Keypair::new();
    let judge_signer = Keypair::new();
    let (mut svm, global_config_pda, gc) = common::setup_svm(sas_program, admin.pubkey());
    let base_cred = gc.sas_credential;
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let base_att = Pubkey::new_unique();
    let judge_cred = Pubkey::new_unique();
    let judge_att = Pubkey::new_unique();

    svm.set_account(
        base_cred,
        Account {
            lamports: 1_000_000,
            data: serialize_sas_acc(
                CREDENTIAL_DISCRIMINATOR,
                &Credential {
                    authority: admin.pubkey(),
                    name: b"BaseKYC".to_vec(),
                    authorized_signers: vec![base_signer.pubkey()],
                },
            ),
            owner: sas_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    svm.set_account(
        base_att,
        Account {
            lamports: 1_000_000,
            data: serialize_sas_acc(
                ATTESTATION_DISCRIMINATOR,
                &Attestation {
                    nonce: user.pubkey(),
                    credential: base_cred,
                    schema: Pubkey::new_unique(),
                    data: b"base-kyc-data".to_vec(),
                    signer: base_signer.pubkey(),
                    expiry: 0,
                    token_account: Pubkey::default(),
                },
            ),
            owner: sas_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    svm.set_account(
        judge_cred,
        Account {
            lamports: 1_000_000,
            data: serialize_sas_acc(
                CREDENTIAL_DISCRIMINATOR,
                &Credential {
                    authority: admin.pubkey(),
                    name: b"JudgeProof".to_vec(),
                    authorized_signers: vec![judge_signer.pubkey()],
                },
            ),
            owner: sas_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    svm.set_account(
        judge_att,
        Account {
            lamports: 1_000_000,
            data: serialize_sas_acc(
                ATTESTATION_DISCRIMINATOR,
                &Attestation {
                    nonce: user.pubkey(),
                    credential: judge_cred,
                    schema: Pubkey::new_unique(),
                    data: b"judge-legal-proof".to_vec(),
                    signer: judge_signer.pubkey(),
                    expiry: 0,
                    token_account: Pubkey::default(),
                },
            ),
            owner: sas_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let user_main_pda = common::create_user_main(&mut svm, &user, &payer);
    let id_hash = hash(b"base-kyc-data").to_bytes();
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
            global_config: global_config_pda,
            sas_attestation: base_att,
            sas_credential: base_cred,
            authority: user.pubkey(),
            payer: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, link_ix, &payer, &[&payer, &user]).unwrap();

    let market_id = 99u64;
    let (market_pda, _) = Pubkey::find_program_address(
        &[
            MARKETPLACE_SEED.as_bytes(),
            market_id.to_le_bytes().as_ref(),
        ],
        &pactum_core::id(),
    );
    let create_mkt_ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::CreateMarketplace {
            market_id,
            params: CreateMarketplaceParams {
                name: "Regulated Jury".to_string(),
                committee: admin.pubkey(),
                dispute_mode: DisputeMode::DecentralizedJury,
                storage_backend: StorageBackend::Arweave,
                integrator_policy: IntegratorPolicy::Permissionless,
                kyb_credential: None,
                client_requires_kyc: false,
                provider_identity: RoleIdentityConfig::none(),
                judge_identity: RoleIdentityConfig {
                    policy: IdentityPolicy::KycAndMarketplaceProof,
                    credential: Some(judge_cred),
                },
                min_judge_stake: 100,
                max_judge_stake: 1000,
                min_arbiter_stake: 500,
                min_jury_fee: 2_000_000,
                provider_cancellation_penalty_pct: 10,
            },
        }
        .data(),
        pactum_core::accounts::CreateMarketplace {
            marketplace: market_pda,
            payer: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, create_mkt_ix, &payer, &[&payer]).unwrap();

    let (judge_pda, _) = Pubkey::find_program_address(
        &[
            JUDGE_SEED.as_bytes(),
            market_pda.as_ref(),
            user.pubkey().as_ref(),
        ],
        &pactum_core::id(),
    );

    // Base KYC alone (no marketplace judge attestation) must fail
    let bypass_ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::InitJudge {}.data(),
        pactum_core::accounts::InitJudge {
            judge: judge_pda,
            marketplace: market_pda,
            user_main: Some(user_main_pda),
            global_config: global_config_pda,
            marketplace_attestation: Some(base_att),
            marketplace_credential: Some(base_cred),
            authority: user.pubkey(),
            payer: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, bypass_ix, &payer, &[&payer, &user]).is_err());

    // Valid judge credential succeeds
    let valid_ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::InitJudge {}.data(),
        pactum_core::accounts::InitJudge {
            judge: judge_pda,
            marketplace: market_pda,
            user_main: Some(user_main_pda),
            global_config: global_config_pda,
            marketplace_attestation: Some(judge_att),
            marketplace_credential: Some(judge_cred),
            authority: user.pubkey(),
            payer: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, valid_ix, &payer, &[&payer, &user]).unwrap();
}
