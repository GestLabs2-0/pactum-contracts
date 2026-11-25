mod common;

use {
    anchor_lang::{
        prelude::{AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::state::UserMainAcc,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn set_global_ban_updates_user_main() {
    let admin = Keypair::new();
    let user = Keypair::new();
    let (mut svm, global_config_pda, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&admin.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let user_main_pda = common::create_user_main(&mut svm, &user, &user);
    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::SetGlobalBan { banned: true }.data(),
        pactum_core::accounts::SetGlobalBan {
            user_main: user_main_pda,
            global_config: global_config_pda,
            authority: admin.pubkey(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &admin, &[&admin]).unwrap();

    let acc = svm.get_account(&user_main_pda).unwrap();
    let user_main = UserMainAcc::try_deserialize(&mut acc.data.as_slice()).unwrap();
    assert!(user_main.is_banned);
}
