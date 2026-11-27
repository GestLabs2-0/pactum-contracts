mod common;

use {
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::{
        constants::{INTEGRATOR_APP_SEED, INTEGRATOR_SEED, MARKETPLACE_SEED},
        instructions::CreateMarketplaceParams,
        state::{DisputeMode, IntegratorPolicy, IntegratorProfile, StorageBackend},
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

fn create_market_with_policy(
    svm: &mut litesvm::LiteSVM,
    market_id: u64,
    committee: Pubkey,
    integrator_policy: IntegratorPolicy,
    payer: &Keypair,
) -> Pubkey {
    let core_id = pactum_core::id();
    let (market_pda, _) = Pubkey::find_program_address(
        &[
            MARKETPLACE_SEED.as_bytes(),
            market_id.to_le_bytes().as_ref(),
        ],
        &core_id,
    );
    let ix = Instruction::new_with_bytes(
        core_id,
        &pactum_core::instruction::CreateMarketplace {
            market_id,
            params: CreateMarketplaceParams {
                name: format!("Market {market_id}"),
                committee,
                dispute_mode: DisputeMode::Hybrid,
                storage_backend: StorageBackend::Arweave,
                integrator_policy,
                kyb_credential: None,
                requires_identity: false,
                min_judge_stake: 100,
                max_judge_stake: 1000,
                min_arbiter_stake: 500,
                min_jury_fee: 2_000_000,
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
    common::send_ix(svm, ix, payer, &[payer]).unwrap();
    market_pda
}

#[test]
fn init_integrator_succeeds_in_permissionless_market_and_fails_in_committee_market() {
    let admin = Keypair::new();
    let committee = Keypair::new();
    let integrator_auth = Keypair::new();
    let (mut svm, _, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&integrator_auth.pubkey(), 10_000_000_000)
        .unwrap();

    let open_market = create_market_with_policy(
        &mut svm,
        10,
        committee.pubkey(),
        IntegratorPolicy::Permissionless,
        &integrator_auth,
    );
    let (profile_pda, _) = Pubkey::find_program_address(
        &[
            INTEGRATOR_SEED.as_bytes(),
            open_market.as_ref(),
            integrator_auth.pubkey().as_ref(),
        ],
        &pactum_core::id(),
    );

    let init_ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::InitIntegrator {
            default_fee_bps: 100,
        }
        .data(),
        pactum_core::accounts::InitIntegrator {
            integrator_profile: profile_pda,
            marketplace: open_market,
            user_main: None,
            authority: integrator_auth.pubkey(),
            payer: integrator_auth.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, init_ix, &integrator_auth, &[&integrator_auth]).unwrap();

    let profile = IntegratorProfile::try_deserialize(
        &mut svm.get_account(&profile_pda).unwrap().data.as_slice(),
    )
    .unwrap();
    assert_eq!(profile.marketplace, open_market);
    assert_eq!(profile.authority, integrator_auth.pubkey());
    assert_eq!(profile.default_fee_bps, 100);
    assert!(profile.is_active);
    assert!(!profile.is_banned);

    let regulated_market = create_market_with_policy(
        &mut svm,
        11,
        committee.pubkey(),
        IntegratorPolicy::CommitteeApproval,
        &integrator_auth,
    );
    let (regulated_profile_pda, _) = Pubkey::find_program_address(
        &[
            INTEGRATOR_SEED.as_bytes(),
            regulated_market.as_ref(),
            integrator_auth.pubkey().as_ref(),
        ],
        &pactum_core::id(),
    );
    let bad_ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::InitIntegrator {
            default_fee_bps: 100,
        }
        .data(),
        pactum_core::accounts::InitIntegrator {
            integrator_profile: regulated_profile_pda,
            marketplace: regulated_market,
            user_main: None,
            authority: integrator_auth.pubkey(),
            payer: integrator_auth.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, bad_ix, &integrator_auth, &[&integrator_auth]).is_err());
}

#[test]
fn apply_review_and_revoke_integrator_in_committee_market() {
    let admin = Keypair::new();
    let committee = Keypair::new();
    let applicant = Keypair::new();
    let rent_payer = Keypair::new();
    let (mut svm, global_config_pda, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&committee.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&applicant.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&rent_payer.pubkey(), 10_000_000_000).unwrap();

    let market_pda = create_market_with_policy(
        &mut svm,
        12,
        committee.pubkey(),
        IntegratorPolicy::CommitteeApproval,
        &rent_payer,
    );
    let (app_pda, _) = Pubkey::find_program_address(
        &[
            INTEGRATOR_APP_SEED.as_bytes(),
            market_pda.as_ref(),
            applicant.pubkey().as_ref(),
        ],
        &pactum_core::id(),
    );
    let (profile_pda, _) = Pubkey::find_program_address(
        &[
            INTEGRATOR_SEED.as_bytes(),
            market_pda.as_ref(),
            applicant.pubkey().as_ref(),
        ],
        &pactum_core::id(),
    );

    let apply_ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::ApplyAsIntegrator {
            default_fee_bps: 75,
            id_hash: None,
        }
        .data(),
        pactum_core::accounts::ApplyAsIntegrator {
            integrator_application: app_pda,
            marketplace: market_pda,
            global_config: global_config_pda,
            identity: None,
            sas_kyb_attestation: None,
            sas_credential: None,
            authority: applicant.pubkey(),
            payer: rent_payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, apply_ix, &rent_payer, &[&rent_payer, &applicant]).unwrap();

    let rent_before = svm.get_account(&rent_payer.pubkey()).unwrap().lamports;
    let review_ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::ReviewIntegratorApplication { approve: true }.data(),
        pactum_core::accounts::ReviewIntegratorApplication {
            integrator_application: app_pda,
            marketplace: market_pda,
            committee: committee.pubkey(),
            integrator_profile: profile_pda,
            rent_payer: rent_payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, review_ix, &committee, &[&committee]).unwrap();

    assert!(svm.get_account(&app_pda).is_none());
    assert!(svm.get_account(&rent_payer.pubkey()).unwrap().lamports > rent_before);

    let profile = IntegratorProfile::try_deserialize(
        &mut svm.get_account(&profile_pda).unwrap().data.as_slice(),
    )
    .unwrap();
    assert_eq!(profile.default_fee_bps, 75);
    assert!(profile.is_active);

    let revoke_ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::RevokeIntegrator {}.data(),
        pactum_core::accounts::RevokeIntegrator {
            integrator_profile: profile_pda,
            marketplace: market_pda,
            committee: committee.pubkey(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, revoke_ix, &committee, &[&committee]).unwrap();

    let after = IntegratorProfile::try_deserialize(
        &mut svm.get_account(&profile_pda).unwrap().data.as_slice(),
    )
    .unwrap();
    assert!(!after.is_active);
}
