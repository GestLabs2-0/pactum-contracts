mod common;

use {
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    pactum_config::{constants::GLOBAL_CONFIG_SEED, state::GlobalConfig},
    pactum_escrow::{constants::ESCROW_CONFIG_SEED, state::EscrowConfig},
    solana_account::Account,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn initialize_escrow_persists_config() {
    let authority = Keypair::new();
    let (svm, global_config_pda, escrow_config_pda, _, _) = common::setup_svm(&authority);

    let acc = svm.get_account(&escrow_config_pda).unwrap();
    let config = EscrowConfig::try_deserialize(&mut acc.data.as_slice()).unwrap();
    assert_eq!(config.authority, authority.pubkey());
    assert_eq!(config.global_config, global_config_pda);
    assert!(config.is_initialized);
}

#[test]
fn initialize_escrow_rejects_unauthorized_caller() {
    let authority = Keypair::new();
    let attacker = Keypair::new();
    let mut svm = LiteSVM::new();
    svm.add_program(
        pactum_escrow::id(),
        include_bytes!("../../../target/deploy/pactum_escrow.so"),
    )
    .unwrap();
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();

    let (global_config_pda, gc_bump) =
        Pubkey::find_program_address(&[GLOBAL_CONFIG_SEED.as_bytes()], &pactum_config::id());
    let gc = GlobalConfig {
        authority: authority.pubkey(),
        usdc_mint: Pubkey::new_unique(),
        platform_vault: Pubkey::new_unique(),
        platform_vault_bump: 255,
        protocol_fee_bps: 100,
        max_protocol_fee: 0,
        dispute_escalation_fee: 250_000,
        sas_program: Pubkey::new_unique(),
        sas_credential: Pubkey::new_unique(),
        core_program: pactum_core::id(),
        escrow_program: pactum_escrow::id(),
        disputes_program: pactum_config::DISPUTES_PROGRAM_ID,
        treasury_program: pactum_config::TREASURY_PROGRAM_ID,
        is_initialized: true,
        bump: gc_bump,
    };
    svm.set_account(
        global_config_pda,
        Account {
            lamports: 1_000_000_000,
            data: common::serialize_account(&gc),
            owner: pactum_config::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (escrow_config_pda, _) =
        Pubkey::find_program_address(&[ESCROW_CONFIG_SEED.as_bytes()], &pactum_escrow::id());
    let ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::InitializeEscrow {}.data(),
        pactum_escrow::accounts::InitializeEscrow {
            authority: attacker.pubkey(),
            config: escrow_config_pda,
            global_config: global_config_pda,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, ix, &attacker, &[&attacker]).is_err());
}
