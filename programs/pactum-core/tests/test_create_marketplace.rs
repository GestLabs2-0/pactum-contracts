mod common;

use {
    anchor_lang::prelude::{AccountDeserialize, Pubkey},
    pactum_core::state::{DisputeMode, Marketplace, StorageBackend},
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
    assert!(!market.requires_identity);
    assert!(market.is_active);
}
