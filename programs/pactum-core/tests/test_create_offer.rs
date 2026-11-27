mod common;

use {
    anchor_lang::prelude::{AccountDeserialize, Pubkey},
    pactum_core::state::{DisputeMode, Offer, Provider},
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn create_offer_increments_provider_count_and_sets_rent_payer() {
    let admin = Keypair::new();
    let provider_auth = Keypair::new();
    let relayer = Keypair::new();
    let (mut svm, _, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&relayer.pubkey(), 10_000_000_000).unwrap();

    let user_main_pda = common::create_user_main(&mut svm, &provider_auth, &relayer);
    let market_pda = common::create_marketplace_account(
        &mut svm,
        1,
        Pubkey::new_unique(),
        DisputeMode::Hybrid,
        false,
        &relayer,
    );
    let provider_pda = common::create_provider_account(
        &mut svm,
        market_pda,
        user_main_pda,
        &provider_auth,
        &relayer,
    );
    let offer_pda = common::create_offer_account(
        &mut svm,
        provider_pda,
        user_main_pda,
        market_pda,
        0,
        &provider_auth,
        &relayer,
    );

    let offer =
        Offer::try_deserialize(&mut svm.get_account(&offer_pda).unwrap().data.as_slice()).unwrap();
    assert_eq!(offer.offer_id, 0);
    assert_eq!(offer.rent_payer, relayer.pubkey());
    assert!(offer.is_active);

    let provider =
        Provider::try_deserialize(&mut svm.get_account(&provider_pda).unwrap().data.as_slice())
            .unwrap();
    assert_eq!(provider.offer_count, 1);
}
