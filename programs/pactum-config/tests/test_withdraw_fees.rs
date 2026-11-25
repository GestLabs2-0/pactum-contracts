mod common;

use {
    anchor_lang::{
        prelude::Pubkey, solana_program::instruction::Instruction, InstructionData, ToAccountMetas,
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn withdraw_fees_transfers_usdc_to_destination() {
    let mut svm = common::setup_svm();
    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let usdc_mint = common::create_mint(&mut svm, &authority.pubkey(), 6);
    let (global_config, platform_vault_pda, _) = common::initialize_default_config(
        &mut svm,
        &authority,
        usdc_mint,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
    );

    // Fund the platform_vault directly via set_account and withdraw a portion
    let vault_ata = common::create_token_account(&mut svm, &usdc_mint, &platform_vault_pda, 0);
    let dest = common::create_token_account(&mut svm, &usdc_mint, &authority.pubkey(), 0);
    let _ = vault_ata;

    // Seed balance directly into the initialized PDA platform_vault
    let (_, _, platform_vault) = (global_config, platform_vault_pda, {
        let (pv, _) = Pubkey::find_program_address(
            &[pactum_config::constants::PLATFORM_VAULT_CONFIG_SEED.as_bytes()],
            &pactum_config::id(),
        );
        let mut acc = svm.get_account(&pv).unwrap();
        acc.data[64..72].copy_from_slice(&1_000_000u64.to_le_bytes());
        svm.set_account(pv, acc).unwrap();
        pv
    });

    let ix = Instruction::new_with_bytes(
        pactum_config::id(),
        &pactum_config::instruction::WithdrawFees { amount: 400_000 }.data(),
        pactum_config::accounts::WithdrawFees {
            global_config,
            authority: authority.pubkey(),
            platform_vault_pda,
            platform_vault,
            destination_token_account: dest,
            usdc_mint,
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &authority, &[&authority]).unwrap();

    assert_eq!(common::token_account_amount(&svm, &platform_vault), 600_000);
    assert_eq!(common::token_account_amount(&svm, &dest), 400_000);
}

#[test]
fn withdraw_fees_rejects_unauthorized_wallet() {
    let mut svm = common::setup_svm();
    let authority = Keypair::new();
    let attacker = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();

    let usdc_mint = common::create_mint(&mut svm, &authority.pubkey(), 6);
    let (global_config, platform_vault_pda, platform_vault) = common::initialize_default_config(
        &mut svm,
        &authority,
        usdc_mint,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
    );
    let mut acc = svm.get_account(&platform_vault).unwrap();
    acc.data[64..72].copy_from_slice(&1_000_000u64.to_le_bytes());
    svm.set_account(platform_vault, acc).unwrap();

    let dest = common::create_token_account(&mut svm, &usdc_mint, &attacker.pubkey(), 0);
    let ix = Instruction::new_with_bytes(
        pactum_config::id(),
        &pactum_config::instruction::WithdrawFees { amount: 400_000 }.data(),
        pactum_config::accounts::WithdrawFees {
            global_config,
            authority: attacker.pubkey(),
            platform_vault_pda,
            platform_vault,
            destination_token_account: dest,
            usdc_mint,
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &attacker, &[&attacker]).is_err());
}

#[test]
fn withdraw_fees_rejects_zero_amount() {
    let mut svm = common::setup_svm();
    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let usdc_mint = common::create_mint(&mut svm, &authority.pubkey(), 6);
    let (global_config, platform_vault_pda, platform_vault) = common::initialize_default_config(
        &mut svm,
        &authority,
        usdc_mint,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
    );
    let dest = common::create_token_account(&mut svm, &usdc_mint, &authority.pubkey(), 0);

    let ix = Instruction::new_with_bytes(
        pactum_config::id(),
        &pactum_config::instruction::WithdrawFees { amount: 0 }.data(),
        pactum_config::accounts::WithdrawFees {
            global_config,
            authority: authority.pubkey(),
            platform_vault_pda,
            platform_vault,
            destination_token_account: dest,
            usdc_mint,
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &authority, &[&authority]).is_err());
}
