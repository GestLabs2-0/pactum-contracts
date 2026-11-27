mod common;

use {
    anchor_lang::{
        prelude::{AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::state::ArbiterProfile,
    pactum_treasury::constants::CPI_AUTHORITY_SEED,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn unstake_arbiter_returns_usdc_when_no_active_disputes() {
    let admin = Keypair::new();
    let arbiter_auth = Keypair::new();
    let (mut svm, global_config_pda, treasury_config_pda, usdc_mint) = common::setup_svm(&admin);
    svm.airdrop(&arbiter_auth.pubkey(), 10_000_000_000).unwrap();

    let market_pda = common::seed_marketplace(&mut svm, 1, 100_000, 1_000_000, 500_000);
    let arbiter_vault_pda = common::arbiter_vault_pda(&market_pda);
    let arbiter_pda =
        common::seed_arbiter(&mut svm, &arbiter_auth.pubkey(), &market_pda, 500_000, 0);
    common::seed_token_account_at(
        &mut svm,
        &arbiter_vault_pda,
        &usdc_mint,
        &arbiter_vault_pda,
        500_000,
    );

    let user_token = common::create_token_account(&mut svm, &usdc_mint, &arbiter_auth.pubkey(), 0);
    let (cpi_authority, _) =
        Pubkey::find_program_address(&[CPI_AUTHORITY_SEED.as_bytes()], &pactum_treasury::id());

    let ix = Instruction::new_with_bytes(
        pactum_treasury::id(),
        &pactum_treasury::instruction::UnstakeArbiter { amount: 500_000 }.data(),
        pactum_treasury::accounts::UnstakeArbiter {
            authority: arbiter_auth.pubkey(),
            marketplace: market_pda,
            arbiter_profile: arbiter_pda,
            config: treasury_config_pda,
            global_config: global_config_pda,
            arbiter_stake_vault: arbiter_vault_pda,
            user_token_account: user_token,
            usdc_mint,
            cpi_authority,
            core_program: pactum_core::id(),
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &arbiter_auth, &[&arbiter_auth]).unwrap();

    assert_eq!(common::token_account_amount(&svm, &user_token), 500_000);
    assert_eq!(common::token_account_amount(&svm, &arbiter_vault_pda), 0);
    let profile = ArbiterProfile::try_deserialize(
        &mut svm.get_account(&arbiter_pda).unwrap().data.as_slice(),
    )
    .unwrap();
    assert_eq!(profile.staked_amount, 0);
}
