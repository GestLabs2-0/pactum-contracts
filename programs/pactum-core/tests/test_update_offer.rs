mod common;

use {
    anchor_lang::{
        prelude::{AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::{
        instructions::UpdateOfferParams,
        state::{DisputeMode, Offer},
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn update_offer_changes_base_price() {
    let admin = Keypair::new();
    let provider_auth = Keypair::new();
    let (mut svm, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&provider_auth.pubkey(), 10_000_000_000)
        .unwrap();

    let user_main_pda = common::create_user_main(&mut svm, &provider_auth, &provider_auth);
    let market_pda = common::create_marketplace_account(
        &mut svm,
        1,
        Pubkey::new_unique(),
        DisputeMode::Hybrid,
        false,
        &provider_auth,
    );
    let provider_pda = common::create_provider_account(
        &mut svm,
        market_pda,
        user_main_pda,
        &provider_auth,
        &provider_auth,
    );
    let offer_pda = common::create_offer_account(
        &mut svm,
        provider_pda,
        user_main_pda,
        market_pda,
        0,
        &provider_auth,
        &provider_auth,
    );

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::UpdateOffer {
            params: UpdateOfferParams {
                base_price: Some(Some(75_000_000)),
                provider_bond: None,
                required_client_bond: None,
                default_arbiter: None,
                content_ref: None,
                state_hash: None,
                is_active: None,
            },
        }
        .data(),
        pactum_core::accounts::UpdateOffer {
            offer: offer_pda,
            provider: provider_pda,
            user_main: user_main_pda,
            authority: provider_auth.pubkey(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &provider_auth, &[&provider_auth]).unwrap();

    let offer =
        Offer::try_deserialize(&mut svm.get_account(&offer_pda).unwrap().data.as_slice()).unwrap();
    assert_eq!(offer.base_price, Some(75_000_000));
}
