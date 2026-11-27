mod common;

use {
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_config::{
        constants::{
            CORE_PROGRAM_ID, DISPUTES_PROGRAM_ID, ESCROW_PROGRAM_ID, GLOBAL_CONFIG_SEED,
            PLATFORM_VAULT_CONFIG_SEED, PLATFORM_VAULT_SEED, TREASURY_PROGRAM_ID,
        },
        instructions::InitializeConfigParams,
        state::GlobalConfig,
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn initialize_config_persists_fees_sas_and_program_ids() {
    let mut svm = common::setup_svm();
    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let usdc_mint = common::create_mint(&mut svm, &authority.pubkey(), 6);
    let sas_program = Pubkey::new_unique();
    let sas_credential = Pubkey::new_unique();

    let (global_config_pda, _, platform_vault) = common::initialize_default_config(
        &mut svm,
        &authority,
        usdc_mint,
        sas_program,
        sas_credential,
    );

    let acc = svm.get_account(&global_config_pda).unwrap();
    let cfg = GlobalConfig::try_deserialize(&mut acc.data.as_slice()).unwrap();

    assert_eq!(cfg.authority, authority.pubkey());
    assert_eq!(cfg.usdc_mint, usdc_mint);
    assert_eq!(cfg.platform_vault, platform_vault);
    assert_eq!(cfg.protocol_fee_bps, 50);
    assert_eq!(cfg.max_protocol_fee, 0);
    assert_eq!(cfg.dispute_escalation_fee, 250_000);
    assert_eq!(cfg.sas_program, sas_program);
    assert_eq!(cfg.sas_credential, sas_credential);
    assert_eq!(cfg.core_program, CORE_PROGRAM_ID);
    assert_eq!(cfg.escrow_program, ESCROW_PROGRAM_ID);
    assert_eq!(cfg.disputes_program, DISPUTES_PROGRAM_ID);
    assert_eq!(cfg.treasury_program, TREASURY_PROGRAM_ID);
    assert!(cfg.is_initialized);
}

#[test]
fn initialize_config_rejects_fee_bps_at_or_above_10000() {
    let mut svm = common::setup_svm();
    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();
    let usdc_mint = common::create_mint(&mut svm, &authority.pubkey(), 6);

    let program_id = pactum_config::id();
    let (global_config, _) =
        Pubkey::find_program_address(&[GLOBAL_CONFIG_SEED.as_bytes()], &program_id);
    let (platform_vault_pda, _) =
        Pubkey::find_program_address(&[PLATFORM_VAULT_SEED.as_bytes()], &program_id);
    let (platform_vault, _) =
        Pubkey::find_program_address(&[PLATFORM_VAULT_CONFIG_SEED.as_bytes()], &program_id);

    let ix = Instruction::new_with_bytes(
        program_id,
        &pactum_config::instruction::InitializeConfig {
            params: InitializeConfigParams {
                protocol_fee_bps: 10_000,
                max_protocol_fee: 0,
                dispute_escalation_fee: 250_000,
                sas_program: Pubkey::new_unique(),
                sas_credential: Pubkey::new_unique(),
            },
        }
        .data(),
        pactum_config::accounts::InitializeConfig {
            global_config,
            authority: authority.pubkey(),
            platform_vault_pda,
            platform_vault,
            usdc_mint,
            token_program: common::TOKEN_PROGRAM_ID,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &authority, &[&authority]).is_err());
}
