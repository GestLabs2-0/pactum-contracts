mod common;

use {
    anchor_lang::prelude::{AccountDeserialize, Pubkey},
    pactum_core::state::{DisputeMode, Provider},
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn init_provider_succeeds_on_open_marketplace() {
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
        DisputeMode::Hybrid,
        false,
        &payer,
    );
    let provider_pda =
        common::create_provider_account(&mut svm, market_pda, user_main_pda, &user, &payer);

    let acc = svm.get_account(&provider_pda).unwrap();
    let provider = Provider::try_deserialize(&mut acc.data.as_slice()).unwrap();
    assert_eq!(provider.offer_count, 0);
    assert!(!provider.is_banned);
}
