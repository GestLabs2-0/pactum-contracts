mod common;

use {
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::{
        constants::CLIENT_SEED,
        state::{Client, DisputeMode, ParticipantRole},
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn set_local_ban_by_committee_bans_client() {
    let admin = Keypair::new();
    let committee = Keypair::new();
    let user = Keypair::new();
    let (mut svm, _, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&committee.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let market_pda = common::create_marketplace_account(
        &mut svm,
        1,
        committee.pubkey(),
        DisputeMode::ArbiterOnly,
        false,
        &user,
    );
    let (client_pda, _) = Pubkey::find_program_address(
        &[
            CLIENT_SEED.as_bytes(),
            market_pda.as_ref(),
            user.pubkey().as_ref(),
        ],
        &pactum_core::id(),
    );
    let init_ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::InitClient {}.data(),
        pactum_core::accounts::InitClient {
            client: client_pda,
            marketplace: market_pda,
            user_main: None,
            authority: user.pubkey(),
            payer: user.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, init_ix, &user, &[&user]).unwrap();

    let ban_ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::SetLocalBan {
            role: ParticipantRole::Client,
            banned: true,
        }
        .data(),
        pactum_core::accounts::SetLocalBan {
            marketplace: market_pda,
            committee: committee.pubkey(),
            target: user.pubkey(),
            client: Some(client_pda),
            provider: None,
            judge: None,
            integrator: None,
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ban_ix, &committee, &[&committee]).unwrap();

    let acc = svm.get_account(&client_pda).unwrap();
    let client = Client::try_deserialize(&mut acc.data.as_slice()).unwrap();
    assert!(client.is_banned);
}
