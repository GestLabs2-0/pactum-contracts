mod common;

use {
    anchor_lang::prelude::{AccountDeserialize, Pubkey},
    pactum_core::state::UserMainAcc,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn init_user_main_creates_unbanned_account() {
    let admin = Keypair::new();
    let user = Keypair::new();
    let payer = Keypair::new();
    let (mut svm, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let user_main_pda = common::create_user_main(&mut svm, &user, &payer);
    let acc = svm.get_account(&user_main_pda).unwrap();
    let user_main = UserMainAcc::try_deserialize(&mut acc.data.as_slice()).unwrap();

    assert_eq!(user_main.authority, user.pubkey());
    assert_eq!(user_main.sas_attestation, None);
    assert!(!user_main.is_banned);
}
