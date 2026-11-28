#![allow(dead_code)]

use {
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, AccountSerialize, Clock, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    pactum_config::{constants::GLOBAL_CONFIG_SEED, state::GlobalConfig},
    pactum_core::{
        constants::{
            ARBITER_SEED, CLIENT_SEED, INTEGRATOR_SEED, JUDGE_SEED, MARKETPLACE_SEED, OFFER_SEED,
            PROVIDER_SEED,
        },
        state::{
            ArbiterProfile, Client, DisputeMode as MarketDisputeMode, IntegratorPolicy,
            IntegratorProfile, Judge, Marketplace, Offer, PricingType, Provider,
            RoleIdentityConfig, StorageBackend,
        },
    },
    pactum_disputes::{
        constants::{DISPUTE_CONFIG_SEED, DISPUTE_SEED, POOL_LEAF_SEED, VOTE_RECORD_SEED},
        state::DisputeConfig,
        InitializeDisputeConfigParams,
    },
    pactum_escrow::{
        constants::{AGREEMENT_SEED, ESCROW_CONFIG_SEED, ESCROW_VAULT_SEED},
        state::{Agreement, AgreementStatus},
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

pub fn deserialize_account<T: AccountDeserialize>(svm: &LiteSVM, address: &Pubkey) -> T {
    let acc = svm.get_account(address).expect("account not found");
    let mut data: &[u8] = &acc.data;
    T::try_deserialize(&mut data).expect("failed to deserialize account")
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
    let res = svm
        .send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{e:?}"));
    svm.expire_blockhash();
    res
}

pub fn warp_to_ts(svm: &mut LiteSVM, unix_timestamp: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = unix_timestamp;
    svm.set_sysvar(&clock);
    svm.expire_blockhash();
}

pub struct TestEnv {
    pub svm: LiteSVM,
    pub authority: Keypair,
    pub global_config: Pubkey,
    pub dispute_config: Pubkey,
    pub escrow_config: Pubkey,
    pub treasury_config: Pubkey,
    pub usdc_mint: Pubkey,
    pub platform_vault: Pubkey,
}

/// Boots LiteSVM with all 5 Pactum programs, initializes `GlobalConfig`, `EscrowConfig`,
/// `TreasuryConfig`, and optionally initializes `DisputeConfig`.
pub fn setup_svm(init_dispute_config: bool) -> TestEnv {
    let mut svm = LiteSVM::new();
    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    svm.add_program(
        pactum_config::id(),
        include_bytes!("../../../../target/deploy/pactum_config.so"),
    )
    .unwrap();
    svm.add_program(
        pactum_core::id(),
        include_bytes!("../../../../target/deploy/pactum_core.so"),
    )
    .unwrap();
    svm.add_program(
        pactum_escrow::id(),
        include_bytes!("../../../../target/deploy/pactum_escrow.so"),
    )
    .unwrap();
    svm.add_program(
        pactum_treasury::id(),
        include_bytes!("../../../../target/deploy/pactum_treasury.so"),
    )
    .unwrap();
    svm.add_program(
        pactum_disputes::id(),
        include_bytes!("../../../../target/deploy/pactum_disputes.so"),
    )
    .unwrap();

    let usdc_mint = create_mint(&mut svm, &authority.pubkey(), 6);
    let platform_vault = create_token_account(&mut svm, &usdc_mint, &authority.pubkey(), 0);
    let (global_config, gc_bump) =
        Pubkey::find_program_address(&[GLOBAL_CONFIG_SEED.as_bytes()], &pactum_config::id());
    let gc = GlobalConfig {
        authority: authority.pubkey(),
        usdc_mint,
        platform_vault,
        platform_vault_bump: 255,
        protocol_fee_bps: 100,
        max_protocol_fee: 0,
        dispute_escalation_fee: 0,
        sas_program: Pubkey::new_unique(),
        sas_credential: Pubkey::new_unique(),
        core_program: pactum_core::id(),
        escrow_program: pactum_escrow::id(),
        disputes_program: pactum_disputes::id(),
        treasury_program: pactum_treasury::id(),
        is_initialized: true,
        bump: gc_bump,
    };
    svm.set_account(
        global_config,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&gc),
            owner: pactum_config::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let (escrow_config, _) =
        Pubkey::find_program_address(&[ESCROW_CONFIG_SEED.as_bytes()], &pactum_escrow::id());
    let init_escrow_ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::InitializeEscrow {}.data(),
        pactum_escrow::accounts::InitializeEscrow {
            authority: authority.pubkey(),
            config: escrow_config,
            global_config,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    send_ix(&mut svm, init_escrow_ix, &authority, &[&authority]).unwrap();

    let (treasury_config, _) =
        Pubkey::find_program_address(&[TREASURY_CONFIG_SEED.as_bytes()], &pactum_treasury::id());
    let init_treasury_ix = Instruction::new_with_bytes(
        pactum_treasury::id(),
        &pactum_treasury::instruction::InitializeTreasury {}.data(),
        pactum_treasury::accounts::InitializeTreasury {
            authority: authority.pubkey(),
            config: treasury_config,
            global_config,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    send_ix(&mut svm, init_treasury_ix, &authority, &[&authority]).unwrap();

    let (dispute_config, _) =
        Pubkey::find_program_address(&[DISPUTE_CONFIG_SEED.as_bytes()], &pactum_disputes::id());

    if init_dispute_config {
        let ix = Instruction::new_with_bytes(
            pactum_disputes::id(),
            &pactum_disputes::instruction::InitializeDisputeConfig {
                params: InitializeDisputeConfigParams {
                    p2p_window_secs: 172_800,
                    vote_window_secs: 172_800,
                    reveal_window_secs: 86_400,
                    arbiter_sla_slash_bps: 500,
                    no_fault_fee_bps: 5_000,
                },
            }
            .data(),
            pactum_disputes::accounts::InitializeDisputeConfig {
                dispute_config,
                global_config,
                authority: authority.pubkey(),
                system_program: system_program::id(),
            }
            .to_account_metas(None),
        );
        send_ix(&mut svm, ix, &authority, &[&authority]).unwrap();
    }

    TestEnv {
        svm,
        authority,
        global_config,
        dispute_config,
        escrow_config,
        treasury_config,
        usdc_mint,
        platform_vault,
    }
}

pub fn seed_marketplace(
    svm: &mut LiteSVM,
    market_id: u64,
    dispute_mode: MarketDisputeMode,
) -> Pubkey {
    let (market_pda, bump) = Pubkey::find_program_address(
        &[
            MARKETPLACE_SEED.as_bytes(),
            market_id.to_le_bytes().as_ref(),
        ],
        &pactum_core::id(),
    );
    let market = Marketplace {
        market_id,
        name: format!("Market {market_id}"),
        committee: Pubkey::new_unique(),
        dispute_mode,
        storage_backend: StorageBackend::Arweave,
        integrator_policy: IntegratorPolicy::Permissionless,
        kyb_credential: None,
        client_requires_kyc: false,
        provider_identity: RoleIdentityConfig::none(),
        judge_identity: RoleIdentityConfig::none(),
        min_judge_stake: 100,
        max_judge_stake: 10_000,
        min_arbiter_stake: 500,
        min_jury_fee: 2_000_000,
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
            owner: pactum_core::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    market_pda
}

pub fn seed_client(svm: &mut LiteSVM, marketplace: &Pubkey, authority: &Pubkey) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[
            CLIENT_SEED.as_bytes(),
            marketplace.as_ref(),
            authority.as_ref(),
        ],
        &pactum_core::id(),
    );
    let acc = Client {
        marketplace: *marketplace,
        authority: *authority,
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
        pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&acc),
            owner: pactum_core::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    pda
}

pub fn seed_provider(svm: &mut LiteSVM, marketplace: &Pubkey, authority: &Pubkey) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[
            PROVIDER_SEED.as_bytes(),
            marketplace.as_ref(),
            authority.as_ref(),
        ],
        &pactum_core::id(),
    );
    let acc = Provider {
        marketplace: *marketplace,
        authority: *authority,
        is_banned: false,
        offer_count: 1,
        completed_agreements: 0,
        cancelled_agreements: 0,
        disputes_won: 0,
        disputes_lost: 0,
        total_reviews: 0,
        total_score: 0,
        bump,
    };
    svm.set_account(
        pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&acc),
            owner: pactum_core::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    pda
}

pub fn seed_offer(svm: &mut LiteSVM, provider: &Pubkey, offer_id: u32) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[
            OFFER_SEED.as_bytes(),
            provider.as_ref(),
            &offer_id.to_le_bytes(),
        ],
        &pactum_core::id(),
    );
    let acc = Offer {
        offer_id,
        base_price: Some(10_000_000),
        pricing_type: PricingType::Fixed,
        approval_window_secs: 86_400,
        bond: 1_000_000,
        jury_fee: 2_000_000,
        default_arbiter: None,
        cancellation_policy: None,
        content_ref: [0u8; 32],
        is_active: true,
        rent_payer: Pubkey::new_unique(),
        bump,
    };
    svm.set_account(
        pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&acc),
            owner: pactum_core::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    pda
}

pub fn seed_arbiter(
    svm: &mut LiteSVM,
    marketplace: &Pubkey,
    authority: &Pubkey,
    arbiter_fee: u64,
    staked_amount: u64,
    sla_secs: i64,
    is_active: bool,
) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[
            ARBITER_SEED.as_bytes(),
            marketplace.as_ref(),
            authority.as_ref(),
        ],
        &pactum_core::id(),
    );
    let acc = ArbiterProfile {
        marketplace: *marketplace,
        authority: *authority,
        sas_kyb_attestation: Pubkey::new_unique(),
        arbiter_fee,
        staked_amount,
        sla_secs,
        active_disputes: 0,
        cases_resolved: 0,
        is_active,
        bump,
    };
    svm.set_account(
        pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&acc),
            owner: pactum_core::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    pda
}

pub fn seed_integrator(
    svm: &mut LiteSVM,
    marketplace: &Pubkey,
    authority: &Pubkey,
    is_active: bool,
) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[
            INTEGRATOR_SEED.as_bytes(),
            marketplace.as_ref(),
            authority.as_ref(),
        ],
        &pactum_core::id(),
    );
    let acc = IntegratorProfile {
        marketplace: *marketplace,
        authority: *authority,
        sas_kyb_attestation: None,
        default_fee_bps: 100,
        fallback_resolutions: 0,
        is_active,
        is_banned: false,
        bump,
    };
    svm.set_account(
        pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&acc),
            owner: pactum_core::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    pda
}

pub fn seed_judge(
    svm: &mut LiteSVM,
    marketplace: &Pubkey,
    authority: &Pubkey,
    staked_amount: u64,
    pool_id: u16,
    leaf_index: u32,
) -> Pubkey {
    let (pda, bump) = Pubkey::find_program_address(
        &[
            JUDGE_SEED.as_bytes(),
            marketplace.as_ref(),
            authority.as_ref(),
        ],
        &pactum_core::id(),
    );
    let acc = Judge {
        marketplace: *marketplace,
        authority: *authority,
        staked_amount,
        freezed_stake: 0,
        pool_id,
        leaf_index,
        is_active_in_pool: true,
        active_disputes: 1,
        cases_voted: 0,
        cases_in_consensus: 0,
        is_banned: false,
        bump,
    };
    svm.set_account(
        pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&acc),
            owner: pactum_core::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    pda
}

#[allow(clippy::too_many_arguments)]
pub fn seed_agreement_with_vault(
    svm: &mut LiteSVM,
    usdc_mint: &Pubkey,
    offer: &Pubkey,
    client: &Pubkey,
    provider: &Pubkey,
    arbiter: Option<Pubkey>,
    integrator: Option<Pubkey>,
    nonce: u64,
    total_amount: u64,
    bond: u64,
    jury_fee: u64,
    rent_payer: &Pubkey,
) -> (Pubkey, Pubkey) {
    let (agreement_pda, bump) = Pubkey::find_program_address(
        &[
            AGREEMENT_SEED.as_bytes(),
            offer.as_ref(),
            client.as_ref(),
            &nonce.to_le_bytes(),
        ],
        &pactum_escrow::id(),
    );
    let (vault_pda, vault_bump) = Pubkey::find_program_address(
        &[ESCROW_VAULT_SEED.as_bytes(), agreement_pda.as_ref()],
        &pactum_escrow::id(),
    );

    let vault_amount = total_amount + (bond * 2) + (jury_fee * 2);
    seed_token_account_at(svm, &vault_pda, usdc_mint, &vault_pda, vault_amount);

    let ag = Agreement {
        nonce,
        offer: *offer,
        client: *client,
        provider: *provider,
        integrator,
        integrator_fee_bps: 0,
        arbiter,
        jury_fee,
        total_amount,
        provider_bond: bond,
        client_bond: bond,
        released_amount: 0,
        milestone_amounts: vec![total_amount],
        completed_milestones: 0,
        step_duration_secs: 86_400,
        current_deadline_ts: 1_000_000,
        approval_window_secs: 86_400,
        created_at: 100,
        submitted_at: 0,
        completed_at: 0,
        deliverable_hash: [0u8; 32],
        content_ref: [0u8; 32],
        deal_contract_hash: None,
        status: AgreementStatus::Funded,
        previous_status: None,
        client_reviewed: false,
        provider_reviewed: false,
        rent_payer: *rent_payer,
        escrow_vault_bump: vault_bump,
        bump,
    };
    svm.set_account(
        agreement_pda,
        Account {
            lamports: 1_000_000_000,
            data: serialize_account(&ag),
            owner: pactum_escrow::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    (agreement_pda, vault_pda)
}

pub fn dispute_pda(agreement: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[DISPUTE_SEED.as_bytes(), agreement.as_ref()],
        &pactum_disputes::id(),
    )
    .0
}

pub fn vote_record_pda(dispute: &Pubkey, round: u8, leaf_index: u32) -> Pubkey {
    Pubkey::find_program_address(
        &[
            VOTE_RECORD_SEED.as_bytes(),
            dispute.as_ref(),
            &[round],
            &leaf_index.to_le_bytes(),
        ],
        &pactum_disputes::id(),
    )
    .0
}

pub fn pool_leaf_pda(marketplace: &Pubkey, pool_id: u16, leaf_index: u32) -> Pubkey {
    Pubkey::find_program_address(
        &[
            POOL_LEAF_SEED.as_bytes(),
            marketplace.as_ref(),
            &pool_id.to_le_bytes(),
            &leaf_index.to_le_bytes(),
        ],
        &pactum_disputes::id(),
    )
    .0
}

pub fn seed_treasury_vault(
    svm: &mut LiteSVM,
    seed: &str,
    marketplace: &Pubkey,
    usdc_mint: &Pubkey,
    amount: u64,
) -> Pubkey {
    let (vault_pda, _) = Pubkey::find_program_address(
        &[seed.as_bytes(), marketplace.as_ref()],
        &pactum_treasury::id(),
    );
    seed_token_account_at(svm, &vault_pda, usdc_mint, &vault_pda, amount);
    vault_pda
}

pub fn arbiter_vault_pda(marketplace: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[ARBITER_VAULT_SEED.as_bytes(), marketplace.as_ref()],
        &pactum_treasury::id(),
    )
    .0
}

pub fn judge_vault_pda(marketplace: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[JUDGE_VAULT_SEED.as_bytes(), marketplace.as_ref()],
        &pactum_treasury::id(),
    )
    .0
}

pub fn get_dispute_config(svm: &LiteSVM, address: &Pubkey) -> DisputeConfig {
    deserialize_account(svm, address)
}
