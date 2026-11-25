mod common;

use {
    anchor_lang::{
        prelude::{AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_config::{instructions::UpdateConfigParams, state::GlobalConfig},
    solana_keypair::Keypair,
    solana_signer::Signer,
};

fn empty_update_params() -> UpdateConfigParams {
    UpdateConfigParams {
        protocol_fee_bps: None,
        max_protocol_fee: None,
        dispute_escalation_fee: None,
        sas_program: None,
        sas_credential: None,
    }
}

#[test]
fn update_config_updates_fees_and_dispute_escalation_fee() {
    let mut svm = common::setup_svm();
    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let usdc_mint = common::create_mint(&mut svm, &authority.pubkey(), 6);
    let (global_config_pda, _, _) = common::initialize_default_config(
        &mut svm,
        &authority,
        usdc_mint,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
    );

    let mut params = empty_update_params();
    params.protocol_fee_bps = Some(75);
    params.max_protocol_fee = Some(10_000_000);
    params.dispute_escalation_fee = Some(500_000);

    let ix = Instruction::new_with_bytes(
        pactum_config::id(),
        &pactum_config::instruction::UpdateConfig { params }.data(),
        pactum_config::accounts::UpdateConfig {
            global_config: global_config_pda,
            authority: authority.pubkey(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &authority, &[&authority]).unwrap();

    let acc = svm.get_account(&global_config_pda).unwrap();
    let cfg = GlobalConfig::try_deserialize(&mut acc.data.as_slice()).unwrap();
    assert_eq!(cfg.protocol_fee_bps, 75);
    assert_eq!(cfg.max_protocol_fee, 10_000_000);
    assert_eq!(cfg.dispute_escalation_fee, 500_000);
    assert_eq!(cfg.authority, authority.pubkey());
}

#[test]
fn update_config_rejects_unauthorized_caller() {
    let mut svm = common::setup_svm();
    let authority = Keypair::new();
    let attacker = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();

    let usdc_mint = common::create_mint(&mut svm, &authority.pubkey(), 6);
    let (global_config_pda, _, _) = common::initialize_default_config(
        &mut svm,
        &authority,
        usdc_mint,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
    );

    let mut params = empty_update_params();
    params.protocol_fee_bps = Some(999);

    let ix = Instruction::new_with_bytes(
        pactum_config::id(),
        &pactum_config::instruction::UpdateConfig { params }.data(),
        pactum_config::accounts::UpdateConfig {
            global_config: global_config_pda,
            authority: attacker.pubkey(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &attacker, &[&attacker]).is_err());
}

#[test]
fn update_config_rejects_invalid_fee_bps() {
    let mut svm = common::setup_svm();
    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let usdc_mint = common::create_mint(&mut svm, &authority.pubkey(), 6);
    let (global_config_pda, _, _) = common::initialize_default_config(
        &mut svm,
        &authority,
        usdc_mint,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
    );

    let mut params = empty_update_params();
    params.protocol_fee_bps = Some(10_000);

    let ix = Instruction::new_with_bytes(
        pactum_config::id(),
        &pactum_config::instruction::UpdateConfig { params }.data(),
        pactum_config::accounts::UpdateConfig {
            global_config: global_config_pda,
            authority: authority.pubkey(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &authority, &[&authority]).is_err());
}
