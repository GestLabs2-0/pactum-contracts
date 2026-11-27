#![allow(dead_code)]

use {
    anchor_lang::{
        prelude::{system_program, AccountSerialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    pactum_config::{constants::GLOBAL_CONFIG_SEED, state::GlobalConfig},
    pactum_core::{
        constants::{ARBITER_SEED, JUDGE_SEED, MARKETPLACE_SEED, USER_MAIN_SEED},
        state::{
            ArbiterProfile, DisputeMode, IntegratorPolicy, Judge, Marketplace, RoleIdentityConfig,
            StorageBackend, UserMainAcc,
        },
    },
    pactum_treasury::constants::{ARBITER_VAULT_SEED, JUDGE_VAULT_SEED, TREASURY_CONFIG_SEED},
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

pub fn serialize_account<T: AccountSerialize>(data: &T) -> Vec<u8> {
    let mut out = Vec::new();
    data.try_serialize(&mut out).unwrap();
    out
}

pub fn setup_svm(authority: &Keypair) -> (LiteSVM, Pubkey, Pubkey, Pubkey) {
    let mut svm = LiteSVM::new();
    let config_bytes = include_bytes!("../../../../target/deploy/pactum_config.so");
    let core_bytes = include_bytes!("../../../../target/deploy/pactum_core.so");
    let treasury_bytes = include_bytes!("../../../../target/deploy/pactum_treasury.so");

    svm.add_program(pactum_config::id(), config_bytes).unwrap();
    svm.add_program(pactum_core::id(), core_bytes).unwrap();
    svm.add_program(pactum_treasury::id(), treasury_bytes)
        .unwrap();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let usdc_mint = create_mint(&mut svm, &authority.pubkey(), 6);
    let (global_config_pda, gc_bump) =
        Pubkey::find_program_address(&[GLOBAL_CONFIG_SEED.as_bytes()], &pactum_config::id());
    let gc = GlobalConfig {
        authority: authority.pubkey(),
        usdc_mint,
        platform_vault: Pubkey::new_unique(),
        platform_vault_bump: 255,
        protocol_fee_bps: 50,
        max_protocol_fee: 0,
        dispute_escalation_fee: 250_000,
        sas_program: Pubkey::new_unique(),
        sas_credential: Pubkey::new_unique(),
        core_program: pactum_core::id(),
        escrow_program: pactum_config::ESCROW_PROGRAM_ID,
        disputes_program: pactum_config::DISPUTES_PROGRAM_ID,
        treasury_program: pactum_treasury::id(),
        is_initialized: true,
        bump: gc_bump,
    };
    svm.set_account(
        global_config_pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&gc),
            owner: pactum_config::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (treasury_config_pda, _) =
        Pubkey::find_program_address(&[TREASURY_CONFIG_SEED.as_bytes()], &pactum_treasury::id());

    let ix = Instruction::new_with_bytes(
        pactum_treasury::id(),
        &pactum_treasury::instruction::InitializeTreasury {}.data(),
        pactum_treasury::accounts::InitializeTreasury {
            authority: authority.pubkey(),
            config: treasury_config_pda,
            global_config: global_config_pda,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    send_ix(&mut svm, ix, authority, &[authority]).unwrap();

    (svm, global_config_pda, treasury_config_pda, usdc_mint)
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

pub fn judge_vault_pda(marketplace: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[JUDGE_VAULT_SEED.as_bytes(), marketplace.as_ref()],
        &pactum_treasury::id(),
    )
    .0
}

pub fn arbiter_vault_pda(marketplace: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[ARBITER_VAULT_SEED.as_bytes(), marketplace.as_ref()],
        &pactum_treasury::id(),
    )
    .0
}

pub fn seed_token_account_at(
    svm: &mut LiteSVM,
    address: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
    amount: u64,
) {
    let mut data = vec![0u8; TOKEN_ACCOUNT_LEN];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1;
    let lamports = svm.minimum_balance_for_rent_exemption(TOKEN_ACCOUNT_LEN);
    svm.set_account(
        *address,
        Account {
            lamports,
            data,
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

pub fn create_token_account(
    svm: &mut LiteSVM,
    mint: &Pubkey,
    owner: &Pubkey,
    amount: u64,
) -> Pubkey {
    let kp = Keypair::new();
    seed_token_account_at(svm, &kp.pubkey(), mint, owner, amount);
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

pub fn seed_marketplace(
    svm: &mut LiteSVM,
    market_id: u64,
    min_judge_stake: u64,
    max_judge_stake: u64,
    min_arbiter_stake: u64,
) -> Pubkey {
    let core_id = pactum_core::id();
    let (market_pda, bump) = Pubkey::find_program_address(
        &[
            MARKETPLACE_SEED.as_bytes(),
            market_id.to_le_bytes().as_ref(),
        ],
        &core_id,
    );
    let market = Marketplace {
        market_id,
        name: format!("Market {market_id}"),
        committee: Pubkey::new_unique(),
        dispute_mode: DisputeMode::Hybrid,
        storage_backend: StorageBackend::Arweave,
        integrator_policy: IntegratorPolicy::Permissionless,
        kyb_credential: None,
        client_requires_kyc: false,
        provider_identity: RoleIdentityConfig::none(),
        judge_identity: RoleIdentityConfig::none(),
        min_judge_stake,
        max_judge_stake,
        min_arbiter_stake,
        min_jury_fee: 0,
        provider_cancellation_penalty_pct: 10,
        pool_count: 1,
        is_active: true,
        bump,
    };
    svm.set_account(
        market_pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&market),
            owner: core_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    market_pda
}

pub fn seed_judge(
    svm: &mut LiteSVM,
    authority: &Pubkey,
    market_pda: &Pubkey,
    staked_amount: u64,
    active_disputes: u32,
) -> (Pubkey, Pubkey) {
    let core_id = pactum_core::id();
    let (user_main_pda, um_bump) =
        Pubkey::find_program_address(&[USER_MAIN_SEED.as_bytes(), authority.as_ref()], &core_id);
    let um = UserMainAcc {
        authority: *authority,
        sas_attestation: None,
        is_banned: false,
        bump: um_bump,
    };
    svm.set_account(
        user_main_pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&um),
            owner: core_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (judge_pda, j_bump) = Pubkey::find_program_address(
        &[
            JUDGE_SEED.as_bytes(),
            market_pda.as_ref(),
            user_main_pda.as_ref(),
        ],
        &core_id,
    );
    let judge = Judge {
        marketplace: *market_pda,
        user_main: user_main_pda,
        staked_amount,
        pool_id: 0,
        leaf_index: 0,
        is_active_in_pool: staked_amount > 0,
        active_disputes,
        cases_voted: 0,
        cases_in_consensus: 0,
        is_banned: false,
        bump: j_bump,
    };
    svm.set_account(
        judge_pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&judge),
            owner: core_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    (user_main_pda, judge_pda)
}

pub fn seed_arbiter(
    svm: &mut LiteSVM,
    authority: &Pubkey,
    market_pda: &Pubkey,
    staked_amount: u64,
    active_disputes: u32,
) -> Pubkey {
    let core_id = pactum_core::id();
    let (arbiter_pda, bump) = Pubkey::find_program_address(
        &[
            ARBITER_SEED.as_bytes(),
            market_pda.as_ref(),
            authority.as_ref(),
        ],
        &core_id,
    );
    let profile = ArbiterProfile {
        marketplace: *market_pda,
        authority: *authority,
        sas_kyb_attestation: Pubkey::new_unique(),
        arbiter_fee: 5_000_000,
        staked_amount,
        sla_secs: 86_400,
        active_disputes,
        cases_resolved: 0,
        is_active: true,
        bump,
    };
    svm.set_account(
        arbiter_pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&profile),
            owner: core_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    arbiter_pda
}
