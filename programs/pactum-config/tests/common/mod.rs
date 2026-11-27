#![allow(dead_code)]

use {
    anchor_lang::{
        prelude::{system_program, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    pactum_config::{
        constants::{GLOBAL_CONFIG_SEED, PLATFORM_VAULT_CONFIG_SEED, PLATFORM_VAULT_SEED},
        instructions::InitializeConfigParams,
    },
    solana_account::Account,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

pub const TOKEN_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
pub const MINT_LEN: usize = 82;
pub const TOKEN_ACCOUNT_LEN: usize = 165;

pub fn setup_svm() -> LiteSVM {
    let program_id = pactum_config::id();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../../target/deploy/pactum_config.so");
    assert!(
        bytes.len() > 10_000,
        "pactum_config.so looks stub-sized ({})",
        bytes.len()
    );
    svm.add_program(program_id, bytes).unwrap();
    svm
}

pub fn create_mint(svm: &mut LiteSVM, mint_authority: &Pubkey, decimals: u8) -> Pubkey {
    let kp = Keypair::new();
    let mut data = vec![0u8; MINT_LEN];
    data[0..4].copy_from_slice(&1u32.to_le_bytes());
    data[4..36].copy_from_slice(mint_authority.as_ref());
    data[44] = decimals;
    data[45] = 1;
    let lamports = svm.minimum_balance_for_rent_exemption(MINT_LEN);
    svm.set_account(
        kp.pubkey(),
        Account {
            lamports,
            data,
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    kp.pubkey()
}

pub fn create_token_account(
    svm: &mut LiteSVM,
    mint: &Pubkey,
    owner: &Pubkey,
    amount: u64,
) -> Pubkey {
    let kp = Keypair::new();
    let mut data = vec![0u8; TOKEN_ACCOUNT_LEN];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1;
    let lamports = svm.minimum_balance_for_rent_exemption(TOKEN_ACCOUNT_LEN);
    svm.set_account(
        kp.pubkey(),
        Account {
            lamports,
            data,
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    kp.pubkey()
}

pub fn token_account_amount(svm: &LiteSVM, account: &Pubkey) -> u64 {
    let acc = svm.get_account(account).unwrap();
    u64::from_le_bytes(acc.data[64..72].try_into().unwrap())
}

pub fn send_ix(
    svm: &mut LiteSVM,
    ix: Instruction,
    payer: &Keypair,
    signers: &[&Keypair],
) -> Result<(), String> {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
}

pub fn initialize_default_config(
    svm: &mut LiteSVM,
    authority: &Keypair,
    usdc_mint: Pubkey,
    sas_program: Pubkey,
    sas_credential: Pubkey,
) -> (Pubkey, Pubkey, Pubkey) {
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
                protocol_fee_bps: 50,
                max_protocol_fee: 0,
                dispute_escalation_fee: 250_000,
                sas_program,
                sas_credential,
            },
        }
        .data(),
        pactum_config::accounts::InitializeConfig {
            global_config,
            authority: authority.pubkey(),
            platform_vault_pda,
            platform_vault,
            usdc_mint,
            token_program: TOKEN_PROGRAM_ID,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    send_ix(svm, ix, authority, &[authority]).unwrap();
    (global_config, platform_vault_pda, platform_vault)
}
