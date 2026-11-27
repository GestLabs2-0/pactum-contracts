#![allow(dead_code)]

use {
    anchor_lang::{
        prelude::{system_program, AccountSerialize, Clock, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    pactum_config::{constants::GLOBAL_CONFIG_SEED, state::GlobalConfig},
    pactum_core::{
        constants::{CLIENT_SEED, MARKETPLACE_SEED, OFFER_SEED, PROVIDER_SEED, USER_MAIN_SEED},
        state::{
            CancellationPolicy, Client, DisputeMode, IntegratorPolicy, Marketplace, Offer,
            PricingType, Provider, RoleIdentityConfig, StorageBackend, UserMainAcc,
        },
    },
    pactum_escrow::constants::ESCROW_CONFIG_SEED,
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

pub fn warp_to_ts(svm: &mut LiteSVM, unix_timestamp: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = unix_timestamp;
    svm.set_sysvar(&clock);
}

/// Boots LiteSVM with `pactum_config`, `pactum_core`, `pactum_escrow`, `GlobalConfig`,
/// `usdc_mint`, `platform_vault`, and initializes `EscrowConfig`.
pub fn setup_svm(authority: &Keypair) -> (LiteSVM, Pubkey, Pubkey, Pubkey, Pubkey) {
    let mut svm = LiteSVM::new();
    let config_bytes = include_bytes!("../../../../target/deploy/pactum_config.so");
    let core_bytes = include_bytes!("../../../../target/deploy/pactum_core.so");
    let escrow_bytes = include_bytes!("../../../../target/deploy/pactum_escrow.so");

    svm.add_program(pactum_config::id(), config_bytes).unwrap();
    svm.add_program(pactum_core::id(), core_bytes).unwrap();
    svm.add_program(pactum_escrow::id(), escrow_bytes).unwrap();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let usdc_mint = create_mint(&mut svm, &authority.pubkey(), 6);
    let platform_vault = create_token_account(&mut svm, &usdc_mint, &authority.pubkey(), 0);
    let (global_config_pda, gc_bump) =
        Pubkey::find_program_address(&[GLOBAL_CONFIG_SEED.as_bytes()], &pactum_config::id());
    let gc = GlobalConfig {
        authority: authority.pubkey(),
        usdc_mint,
        platform_vault,
        platform_vault_bump: 255,
        protocol_fee_bps: 100, // 1%
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
            data: serialize_account(&gc),
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
            authority: authority.pubkey(),
            config: escrow_config_pda,
            global_config: global_config_pda,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    send_ix(&mut svm, ix, authority, &[authority]).unwrap();

    (
        svm,
        global_config_pda,
        escrow_config_pda,
        usdc_mint,
        platform_vault,
    )
}

pub fn seed_marketplace(svm: &mut LiteSVM, market_id: u64) -> Pubkey {
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
        min_judge_stake: 100,
        max_judge_stake: 1000,
        min_arbiter_stake: 500,
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

pub fn seed_user_main(svm: &mut LiteSVM, authority: &Pubkey) -> Pubkey {
    let core_id = pactum_core::id();
    let (user_main_pda, bump) =
        Pubkey::find_program_address(&[USER_MAIN_SEED.as_bytes(), authority.as_ref()], &core_id);
    let user_main = UserMainAcc {
        authority: *authority,
        sas_attestation: None,
        is_banned: false,
        bump,
    };
    svm.set_account(
        user_main_pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&user_main),
            owner: core_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    user_main_pda
}

pub fn seed_client(svm: &mut LiteSVM, market_pda: Pubkey, authority: &Pubkey) -> (Pubkey, Pubkey) {
    let core_id = pactum_core::id();
    let user_main_pda = seed_user_main(svm, authority);
    let (client_pda, bump) = Pubkey::find_program_address(
        &[
            CLIENT_SEED.as_bytes(),
            market_pda.as_ref(),
            user_main_pda.as_ref(),
        ],
        &core_id,
    );
    let client = Client {
        marketplace: market_pda,
        user_main: user_main_pda,
        is_banned: false,
        completed_agreements: 0,
        cancelled_agreements: 0,
        disputes_won: 0,
        disputes_lost: 0,
        total_reviews: 0,
        total_score: 0,
        bump,
    };
    svm.set_account(
        client_pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&client),
            owner: core_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    (client_pda, user_main_pda)
}

pub fn seed_provider(
    svm: &mut LiteSVM,
    market_pda: Pubkey,
    authority: &Pubkey,
) -> (Pubkey, Pubkey) {
    let core_id = pactum_core::id();
    let user_main_pda = seed_user_main(svm, authority);
    let (provider_pda, bump) = Pubkey::find_program_address(
        &[
            PROVIDER_SEED.as_bytes(),
            market_pda.as_ref(),
            user_main_pda.as_ref(),
        ],
        &core_id,
    );
    let provider = Provider {
        marketplace: market_pda,
        user_main: user_main_pda,
        is_banned: false,
        offer_count: 0,
        completed_agreements: 0,
        cancelled_agreements: 0,
        disputes_won: 0,
        disputes_lost: 0,
        total_reviews: 0,
        total_score: 0,
        bump,
    };
    svm.set_account(
        provider_pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&provider),
            owner: core_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    (provider_pda, user_main_pda)
}

pub struct SeedOfferOpts {
    pub pricing_type: PricingType,
    pub base_price: Option<u64>,
    pub provider_bond: u64,
    pub required_client_bond: u64,
    pub cancellation_policy: Option<CancellationPolicy>,
}

pub fn seed_offer(
    svm: &mut LiteSVM,
    provider_pda: Pubkey,
    offer_id: u32,
    opts: SeedOfferOpts,
) -> Pubkey {
    let core_id = pactum_core::id();
    let (offer_pda, bump) = Pubkey::find_program_address(
        &[
            OFFER_SEED.as_bytes(),
            provider_pda.as_ref(),
            offer_id.to_le_bytes().as_ref(),
        ],
        &core_id,
    );
    let offer = Offer {
        offer_id,
        base_price: opts.base_price,
        pricing_type: opts.pricing_type,
        approval_window_secs: 86_400,
        provider_bond: opts.provider_bond,
        required_client_bond: opts.required_client_bond,
        jury_fee: 0,
        default_arbiter: None,
        cancellation_policy: opts.cancellation_policy,
        content_ref: [1u8; 32],
        state_hash: [2u8; 32],
        is_active: true,
        rent_payer: Pubkey::new_unique(),
        bump,
    };
    svm.set_account(
        offer_pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&offer),
            owner: core_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    offer_pda
}
