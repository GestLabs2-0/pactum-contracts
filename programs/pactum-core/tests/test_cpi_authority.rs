mod common;

use {
    anchor_lang::{
        prelude::{AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::{constants::JUDGE_SEED, state::Judge},
    solana_account::Account,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn wallet_cannot_call_cpi_update_judge_stake() {
    let admin = Keypair::new();
    let attacker = Keypair::new();
    let (mut svm, global_config_pda) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();

    let core_id = pactum_core::id();
    let (judge_pda, judge_bump) = Pubkey::find_program_address(
        &[
            JUDGE_SEED.as_bytes(),
            Pubkey::new_unique().as_ref(),
            Pubkey::new_unique().as_ref(),
        ],
        &core_id,
    );
    let judge = Judge {
        marketplace: Pubkey::new_unique(),
        user_main: Pubkey::new_unique(),
        staked_amount: 100,
        pool_id: 0,
        leaf_index: 0,
        is_active_in_pool: true,
        active_disputes: 0,
        cases_voted: 0,
        cases_in_consensus: 0,
        is_banned: false,
        bump: judge_bump,
    };
    svm.set_account(
        judge_pda,
        Account {
            lamports: 1_000_000_000,
            data: common::serialize_account(&judge),
            owner: core_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let ix = Instruction::new_with_bytes(
        core_id,
        &pactum_core::instruction::CpiUpdateJudgeStake {
            amount: 500,
            is_stake: true,
            pool_id: 0,
            leaf_index: 1,
        }
        .data(),
        pactum_core::accounts::CpiUpdateJudgeStake {
            judge: judge_pda,
            global_config: global_config_pda,
            cpi_authority: attacker.pubkey(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &attacker, &[&attacker]).is_err());

    let after =
        Judge::try_deserialize(&mut svm.get_account(&judge_pda).unwrap().data.as_slice()).unwrap();
    assert_eq!(after.staked_amount, 100);
}
