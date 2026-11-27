mod common;

use {
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::state::Judge,
    pactum_treasury::constants::CPI_AUTHORITY_SEED,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn stake_judge_transfers_usdc_and_updates_core_via_cpi() {
    let admin = Keypair::new();
    let judge_auth = Keypair::new();
    let (mut svm, global_config_pda, treasury_config_pda, usdc_mint) = common::setup_svm(&admin);
    svm.airdrop(&judge_auth.pubkey(), 10_000_000_000).unwrap();

    let market_pda = common::seed_marketplace(&mut svm, 1, 100_000, 1_000_000, 500_000);
    let judge_vault_pda = common::judge_vault_pda(&market_pda);
    let (user_main_pda, judge_pda) =
        common::seed_judge(&mut svm, &judge_auth.pubkey(), &market_pda, 0, 0);
    let user_token =
        common::create_token_account(&mut svm, &usdc_mint, &judge_auth.pubkey(), 500_000);
    let (cpi_authority, _) =
        Pubkey::find_program_address(&[CPI_AUTHORITY_SEED.as_bytes()], &pactum_treasury::id());

    let ix = Instruction::new_with_bytes(
        pactum_treasury::id(),
        &pactum_treasury::instruction::StakeJudge {
            amount: 250_000,
            pool_id: 0,
            leaf_index: 3,
        }
        .data(),
        pactum_treasury::accounts::StakeJudge {
            authority: judge_auth.pubkey(),
            user_main: user_main_pda,
            marketplace: market_pda,
            judge: judge_pda,
            config: treasury_config_pda,
            global_config: global_config_pda,
            user_token_account: user_token,
            judge_stake_vault: judge_vault_pda,
            usdc_mint,
            cpi_authority,
            core_program: pactum_core::id(),
            token_program: common::TOKEN_PROGRAM_ID,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &judge_auth, &[&judge_auth]).unwrap();

    assert_eq!(common::token_account_amount(&svm, &user_token), 250_000);
    assert_eq!(
        common::token_account_amount(&svm, &judge_vault_pda),
        250_000
    );

    let judge =
        Judge::try_deserialize(&mut svm.get_account(&judge_pda).unwrap().data.as_slice()).unwrap();
    assert_eq!(judge.staked_amount, 250_000);
    assert_eq!(judge.leaf_index, 3);
    assert!(judge.is_active_in_pool);
}

#[test]
fn stake_judge_rejects_above_max_judge_stake() {
    let admin = Keypair::new();
    let judge_auth = Keypair::new();
    let (mut svm, global_config_pda, treasury_config_pda, usdc_mint) = common::setup_svm(&admin);
    svm.airdrop(&judge_auth.pubkey(), 10_000_000_000).unwrap();

    let market_pda = common::seed_marketplace(&mut svm, 1, 100_000, 500_000, 500_000);
    let judge_vault_pda = common::judge_vault_pda(&market_pda);
    let (user_main_pda, judge_pda) =
        common::seed_judge(&mut svm, &judge_auth.pubkey(), &market_pda, 0, 0);
    let user_token =
        common::create_token_account(&mut svm, &usdc_mint, &judge_auth.pubkey(), 1_000_000);
    let (cpi_authority, _) =
        Pubkey::find_program_address(&[CPI_AUTHORITY_SEED.as_bytes()], &pactum_treasury::id());

    let ix = Instruction::new_with_bytes(
        pactum_treasury::id(),
        &pactum_treasury::instruction::StakeJudge {
            amount: 600_000,
            pool_id: 0,
            leaf_index: 0,
        }
        .data(),
        pactum_treasury::accounts::StakeJudge {
            authority: judge_auth.pubkey(),
            user_main: user_main_pda,
            marketplace: market_pda,
            judge: judge_pda,
            config: treasury_config_pda,
            global_config: global_config_pda,
            user_token_account: user_token,
            judge_stake_vault: judge_vault_pda,
            usdc_mint,
            cpi_authority,
            core_program: pactum_core::id(),
            token_program: common::TOKEN_PROGRAM_ID,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &judge_auth, &[&judge_auth]).is_err());
}
