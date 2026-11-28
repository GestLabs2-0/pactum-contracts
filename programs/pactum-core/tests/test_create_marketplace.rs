mod common;

use {
    anchor_lang::{
        prelude::{AccountDeserialize, Pubkey},
        solana_program::{instruction::Instruction, system_program},
        InstructionData, ToAccountMetas,
    },
    pactum_core::{
        constants::MARKETPLACE_SEED,
        instructions::CreateMarketplaceParams,
        state::{DisputeMode, IntegratorPolicy, Marketplace, RoleIdentityConfig, StorageBackend},
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn create_marketplace_initializes_permissionlessly() {
    let admin = Keypair::new();
    let committee = Keypair::new();
    let payer = Keypair::new();
    let (mut svm, _, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let market_pda = common::create_marketplace_account(
        &mut svm,
        1,
        committee.pubkey(),
        DisputeMode::Hybrid,
        false,
        &payer,
    );

    let acc = svm.get_account(&market_pda).unwrap();
    let market = Marketplace::try_deserialize(&mut acc.data.as_slice()).unwrap();
    assert_eq!(market.market_id, 1);
    assert_eq!(market.committee, committee.pubkey());
    assert_eq!(market.dispute_mode, DisputeMode::Hybrid);
    assert_eq!(market.storage_backend, StorageBackend::Arweave);
    assert!(!market.client_requires_kyc);
    assert_eq!(market.provider_cancellation_penalty_pct, 10);
    assert!(market.is_active);
}

#[test]
fn create_marketplace_rejects_penalty_pct_over_100() {
    let admin = Keypair::new();
    let committee = Keypair::new();
    let payer = Keypair::new();
    let (mut svm, _, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let market_id = 99u64;
    let (market_pda, _) = Pubkey::find_program_address(
        &[
            MARKETPLACE_SEED.as_bytes(),
            market_id.to_le_bytes().as_ref(),
        ],
        &pactum_core::id(),
    );
    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::CreateMarketplace {
            market_id,
            params: CreateMarketplaceParams {
                name: "Bad Penalty".to_string(),
                committee: committee.pubkey(),
                dispute_mode: DisputeMode::Hybrid,
                storage_backend: StorageBackend::Arweave,
                integrator_policy: IntegratorPolicy::Permissionless,
                kyb_credential: None,
                client_requires_kyc: false,
                provider_identity: RoleIdentityConfig::none(),
                judge_identity: RoleIdentityConfig::none(),
                min_judge_stake: 100,
                max_judge_stake: 1000,
                min_arbiter_stake: 500,
                min_jury_fee: 2_000_000,
                provider_cancellation_penalty_pct: 101,
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
    assert!(common::send_ix(&mut svm, ix, &payer, &[&payer]).is_err());
}
