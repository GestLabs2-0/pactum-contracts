mod common;

use {
    anchor_lang::{
        prelude::{AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::state::{Judge, ParticipantRole},
    pactum_treasury::constants::CPI_AUTHORITY_SEED,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn wallet_cannot_call_cpi_slash_stake() {
    let admin = Keypair::new();
    let attacker = Keypair::new();
    let judge_auth = Keypair::new();
    let (mut svm, global_config_pda, treasury_config_pda, treasury_pda, stake_vault_pda, usdc_mint) =
        common::setup_svm(&admin);
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();

    let market_pda = common::seed_marketplace(&mut svm, 1, 100_000, 1_000_000, 500_000);
    let (_, judge_pda) =
        common::seed_judge(&mut svm, &judge_auth.pubkey(), &market_pda, 300_000, 1);
    let recipient_token = common::create_token_account(&mut svm, &usdc_mint, &attacker.pubkey(), 0);
    let (treasury_cpi_authority, _) =
        Pubkey::find_program_address(&[CPI_AUTHORITY_SEED.as_bytes()], &pactum_treasury::id());

    let ix = Instruction::new_with_bytes(
        pactum_treasury::id(),
        &pactum_treasury::instruction::CpiSlashStake {
            target_role: ParticipantRole::Judge,
            amount: 100_000,
        }
        .data(),
        pactum_treasury::accounts::CpiSlashStake {
            cpi_authority: attacker.pubkey(),
            config: treasury_config_pda,
            global_config: global_config_pda,
            judge: Some(judge_pda),
            arbiter_profile: None,
            treasury_pda,
            stake_vault: stake_vault_pda,
            recipient_token_account: recipient_token,
            usdc_mint,
            treasury_cpi_authority,
            core_program: pactum_core::id(),
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &attacker, &[&attacker]).is_err());

    let after =
        Judge::try_deserialize(&mut svm.get_account(&judge_pda).unwrap().data.as_slice()).unwrap();
    assert_eq!(after.staked_amount, 300_000);
}
