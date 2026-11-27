mod common;

use {
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::{
        constants::JUDGE_SEED,
        state::{DisputeMode, Judge},
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn init_judge_rejected_on_arbiter_only_marketplace() {
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
        DisputeMode::ArbiterOnly,
        false,
        &payer,
    );
    let (judge_pda, _) = Pubkey::find_program_address(
        &[
            JUDGE_SEED.as_bytes(),
            market_pda.as_ref(),
            user_main_pda.as_ref(),
        ],
        &pactum_core::id(),
    );

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::InitJudge {}.data(),
        pactum_core::accounts::InitJudge {
            judge: judge_pda,
            marketplace: market_pda,
            user_main: user_main_pda,
            authority: user.pubkey(),
            payer: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &payer, &[&payer, &user]).is_err());
}

#[test]
fn init_judge_succeeds_on_decentralized_jury_marketplace() {
    let admin = Keypair::new();
    let user = Keypair::new();
    let payer = Keypair::new();
    let (mut svm, _, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let user_main_pda = common::create_user_main(&mut svm, &user, &payer);
    let market_pda = common::create_marketplace_account(
        &mut svm,
        2,
        Pubkey::new_unique(),
        DisputeMode::DecentralizedJury,
        false,
        &payer,
    );
    let (judge_pda, _) = Pubkey::find_program_address(
        &[
            JUDGE_SEED.as_bytes(),
            market_pda.as_ref(),
            user_main_pda.as_ref(),
        ],
        &pactum_core::id(),
    );

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::InitJudge {}.data(),
        pactum_core::accounts::InitJudge {
            judge: judge_pda,
            marketplace: market_pda,
            user_main: user_main_pda,
            authority: user.pubkey(),
            payer: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &payer, &[&payer, &user]).unwrap();

    let acc = svm.get_account(&judge_pda).unwrap();
    let judge = Judge::try_deserialize(&mut acc.data.as_slice()).unwrap();
    assert!(!judge.is_active_in_pool);
    assert!(!judge.is_banned);
}
