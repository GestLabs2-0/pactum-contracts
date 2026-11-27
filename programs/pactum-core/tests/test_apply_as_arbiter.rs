mod common;

use {
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::{
        constants::{ARBITER_APP_SEED, IDENTITY_SEED, MARKETPLACE_SEED},
        state::{ArbiterApplication, DisputeMode, Identity, StorageBackend},
        CreateMarketplaceParams,
    },
    solana_keypair::Keypair,
    solana_sha256_hasher::hash,
    solana_signer::Signer,
};

#[test]
fn apply_as_arbiter_stores_application_and_creates_identity() {
    let sas_program = Pubkey::new_unique();
    let signer = Keypair::new();
    let admin = Keypair::new();
    let arbiter = Keypair::new();
    let payer = Keypair::new();
    let (mut svm, global_config_pda, gc) = common::setup_svm(sas_program, admin.pubkey());
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let market_pda = common::create_marketplace_account(
        &mut svm,
        1,
        Pubkey::new_unique(),
        DisputeMode::ArbiterOnly,
        false,
        &payer,
    );
    let raw_kyb = [1u8; 32];
    let id_hash = hash(&raw_kyb).to_bytes();
    let sas_kyb = common::seed_sas_credential_and_attestation(
        &mut svm,
        sas_program,
        gc.sas_credential,
        signer.pubkey(),
        arbiter.pubkey(),
        raw_kyb,
        0,
    );
    let (identity_pda, _) = Pubkey::find_program_address(
        &[IDENTITY_SEED.as_bytes(), id_hash.as_ref()],
        &pactum_core::id(),
    );
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
            id_hash,
        }
        .data(),
        pactum_core::accounts::ApplyAsArbiter {
            arbiter_application: app_pda,
            identity: identity_pda,
            marketplace: market_pda,
            global_config: global_config_pda,
            sas_kyb_attestation: sas_kyb,
            sas_credential: gc.sas_credential,
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

    let identity =
        Identity::try_deserialize(&mut svm.get_account(&identity_pda).unwrap().data.as_slice())
            .unwrap();
    assert_eq!(identity.authority, arbiter.pubkey());
    assert!(identity.linked);
}

#[test]
fn apply_as_arbiter_allows_same_wallet_across_markets_and_blocks_second_wallet() {
    let sas_program = Pubkey::new_unique();
    let signer = Keypair::new();
    let admin = Keypair::new();
    let arbiter_a = Keypair::new();
    let arbiter_b = Keypair::new();
    let payer = Keypair::new();
    let (mut svm, global_config_pda, gc) = common::setup_svm(sas_program, admin.pubkey());
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let market_1 = common::create_marketplace_account(
        &mut svm,
        1,
        Pubkey::new_unique(),
        DisputeMode::ArbiterOnly,
        false,
        &payer,
    );
    let market_2 = common::create_marketplace_account(
        &mut svm,
        2,
        Pubkey::new_unique(),
        DisputeMode::ArbiterOnly,
        false,
        &payer,
    );

    let raw_kyb = [6u8; 32];
    let id_hash = hash(&raw_kyb).to_bytes();
    let (identity_pda, _) = Pubkey::find_program_address(
        &[IDENTITY_SEED.as_bytes(), id_hash.as_ref()],
        &pactum_core::id(),
    );
    let kyb_a = common::seed_sas_credential_and_attestation(
        &mut svm,
        sas_program,
        gc.sas_credential,
        signer.pubkey(),
        arbiter_a.pubkey(),
        raw_kyb,
        0,
    );

    // Same wallet (arbiter_a) applies to market_1 and market_2 with the same KYB id_hash
    for market in [market_1, market_2] {
        let (app_pda, _) = Pubkey::find_program_address(
            &[
                ARBITER_APP_SEED.as_bytes(),
                market.as_ref(),
                arbiter_a.pubkey().as_ref(),
            ],
            &pactum_core::id(),
        );
        let ix = Instruction::new_with_bytes(
            pactum_core::id(),
            &pactum_core::instruction::ApplyAsArbiter {
                arbiter_fee: 5_000_000,
                sla_secs: 172_800,
                id_hash,
            }
            .data(),
            pactum_core::accounts::ApplyAsArbiter {
                arbiter_application: app_pda,
                identity: identity_pda,
                marketplace: market,
                global_config: global_config_pda,
                sas_kyb_attestation: kyb_a,
                sas_credential: gc.sas_credential,
                authority: arbiter_a.pubkey(),
                payer: payer.pubkey(),
                system_program: system_program::id(),
            }
            .to_account_metas(None),
        );
        common::send_ix(&mut svm, ix, &payer, &[&payer, &arbiter_a]).unwrap();
    }

    // Different wallet (arbiter_b) cannot claim the same KYB id_hash
    let kyb_b = common::seed_sas_credential_and_attestation(
        &mut svm,
        sas_program,
        gc.sas_credential,
        signer.pubkey(),
        arbiter_b.pubkey(),
        raw_kyb,
        0,
    );
    let (app_b, _) = Pubkey::find_program_address(
        &[
            ARBITER_APP_SEED.as_bytes(),
            market_1.as_ref(),
            arbiter_b.pubkey().as_ref(),
        ],
        &pactum_core::id(),
    );
    let ix_b = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::ApplyAsArbiter {
            arbiter_fee: 5_000_000,
            sla_secs: 172_800,
            id_hash,
        }
        .data(),
        pactum_core::accounts::ApplyAsArbiter {
            arbiter_application: app_b,
            identity: identity_pda,
            marketplace: market_1,
            global_config: global_config_pda,
            sas_kyb_attestation: kyb_b,
            sas_credential: gc.sas_credential,
            authority: arbiter_b.pubkey(),
            payer: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix_b, &payer, &[&payer, &arbiter_b]).is_err());
}

#[test]
fn apply_as_arbiter_enforces_marketplace_kyb_credential() {
    let sas_program = Pubkey::new_unique();
    let market_kyb_cred = Pubkey::new_unique();
    let signer = Keypair::new();
    let admin = Keypair::new();
    let arbiter = Keypair::new();
    let payer = Keypair::new();
    let (mut svm, global_config_pda, gc) = common::setup_svm(sas_program, admin.pubkey());
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let market_id: u64 = 10;
    let (market_pda, _) = Pubkey::find_program_address(
        &[
            MARKETPLACE_SEED.as_bytes(),
            market_id.to_le_bytes().as_ref(),
        ],
        &pactum_core::id(),
    );
    let create_market_ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::CreateMarketplace {
            market_id,
            params: CreateMarketplaceParams {
                committee: Pubkey::new_unique(),
                dispute_mode: DisputeMode::ArbiterOnly,
                storage_backend: StorageBackend::Arweave,
                kyb_credential: Some(market_kyb_cred),
                requires_identity: false,
                min_judge_stake: 0,
                max_judge_stake: 0,
                min_arbiter_stake: 500,
                min_jury_fee: 0,
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
    common::send_ix(&mut svm, create_market_ix, &payer, &[&payer]).unwrap();

    let raw_kyb = [2u8; 32];
    let id_hash = hash(&raw_kyb).to_bytes();
    let (identity_pda, _) = Pubkey::find_program_address(
        &[IDENTITY_SEED.as_bytes(), id_hash.as_ref()],
        &pactum_core::id(),
    );
    let (app_pda, _) = Pubkey::find_program_address(
        &[
            ARBITER_APP_SEED.as_bytes(),
            market_pda.as_ref(),
            arbiter.pubkey().as_ref(),
        ],
        &pactum_core::id(),
    );

    // Attestation issued under global credential instead of marketplace's kyb_credential must fail
    let wrong_kyb = common::seed_sas_credential_and_attestation(
        &mut svm,
        sas_program,
        gc.sas_credential,
        signer.pubkey(),
        arbiter.pubkey(),
        raw_kyb,
        0,
    );
    let fail_ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::ApplyAsArbiter {
            arbiter_fee: 5_000_000,
            sla_secs: 172_800,
            id_hash,
        }
        .data(),
        pactum_core::accounts::ApplyAsArbiter {
            arbiter_application: app_pda,
            identity: identity_pda,
            marketplace: market_pda,
            global_config: global_config_pda,
            sas_kyb_attestation: wrong_kyb,
            sas_credential: gc.sas_credential,
            authority: arbiter.pubkey(),
            payer: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, fail_ix, &payer, &[&payer, &arbiter]).is_err());

    // Attestation issued under marketplace.kyb_credential succeeds
    let valid_kyb = common::seed_sas_credential_and_attestation(
        &mut svm,
        sas_program,
        market_kyb_cred,
        signer.pubkey(),
        arbiter.pubkey(),
        raw_kyb,
        0,
    );
    let ok_ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::ApplyAsArbiter {
            arbiter_fee: 5_000_000,
            sla_secs: 172_800,
            id_hash,
        }
        .data(),
        pactum_core::accounts::ApplyAsArbiter {
            arbiter_application: app_pda,
            identity: identity_pda,
            marketplace: market_pda,
            global_config: global_config_pda,
            sas_kyb_attestation: valid_kyb,
            sas_credential: market_kyb_cred,
            authority: arbiter.pubkey(),
            payer: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ok_ix, &payer, &[&payer, &arbiter]).unwrap();
}
