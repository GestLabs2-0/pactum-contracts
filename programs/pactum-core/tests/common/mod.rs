#![allow(dead_code)]

use {
    anchor_lang::{
        prelude::{system_program, AccountSerialize, Pubkey},
        solana_program::instruction::Instruction,
        AnchorSerialize, InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED},
    pactum_core::{
        constants::*,
        instructions::CreateOfferParams,
        state::{DisputeMode, PricingType, StorageBackend},
        Attestation, CreateMarketplaceParams, Credential, ATTESTATION_DISCRIMINATOR,
        CREDENTIAL_DISCRIMINATOR,
    },
    solana_account::Account,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

pub fn serialize_account<T: AccountSerialize>(data: &T) -> Vec<u8> {
    let mut out = Vec::new();
    data.try_serialize(&mut out).unwrap();
    out
}

pub fn setup_svm(sas_program: Pubkey, global_authority: Pubkey) -> (LiteSVM, Pubkey, GlobalConfig) {
    let core_id = pactum_core::id();
    let config_id = pactum_config::id();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../../target/deploy/pactum_core.so");
    assert!(
        bytes.len() > 10_000,
        "pactum_core.so looks stub-sized ({})",
        bytes.len()
    );
    svm.add_program(core_id, bytes).unwrap();

    let (global_config_pda, gc_bump) =
        Pubkey::find_program_address(&[GLOBAL_CONFIG_SEED.as_bytes()], &config_id);
    let gc = GlobalConfig {
        authority: global_authority,
        usdc_mint: Pubkey::new_unique(),
        platform_vault: Pubkey::new_unique(),
        platform_vault_bump: 255,
        protocol_fee_bps: 50,
        max_protocol_fee: 0,
        dispute_escalation_fee: 250_000,
        sas_program,
        sas_credential: Pubkey::new_unique(),
        core_program: pactum_config::CORE_PROGRAM_ID,
        escrow_program: pactum_config::ESCROW_PROGRAM_ID,
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
            owner: config_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    (svm, global_config_pda, gc)
}

pub fn seed_sas_credential_and_attestation(
    svm: &mut LiteSVM,
    sas_program: Pubkey,
    sas_credential: Pubkey,
    authorized_signer: Pubkey,
    nonce: Pubkey,
    id_hash: [u8; 32],
    expiry: i64,
) -> Pubkey {
    let cred = Credential {
        authority: Pubkey::new_unique(),
        name: b"PactumKYC".to_vec(),
        authorized_signers: vec![authorized_signer],
    };
    svm.set_account(
        sas_credential,
        Account {
            lamports: 1_000_000,
            data: serialize_sas_acc(CREDENTIAL_DISCRIMINATOR, &cred),
            owner: sas_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let sas_attestation = Pubkey::new_unique();
    let att = Attestation {
        nonce,
        credential: sas_credential,
        schema: Pubkey::new_unique(),
        data: id_hash.to_vec(),
        signer: authorized_signer,
        expiry,
        token_account: Pubkey::default(),
    };
    svm.set_account(
        sas_attestation,
        Account {
            lamports: 1_000_000,
            data: serialize_sas_acc(ATTESTATION_DISCRIMINATOR, &att),
            owner: sas_program,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    sas_attestation
}

pub fn send_ix(
    svm: &mut LiteSVM,
    ix: Instruction,
    payer: &Keypair,
    signers: &[&Keypair],
) -> Result<(), String> {
    let mut unique_signers: Vec<&Keypair> = Vec::with_capacity(signers.len());
    for signer in signers {
        if !unique_signers.iter().any(|s| s.pubkey() == signer.pubkey()) {
            unique_signers.push(signer);
        }
    }
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &unique_signers).unwrap();
    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
}

pub fn create_user_main(svm: &mut LiteSVM, user: &Keypair, payer: &Keypair) -> Pubkey {
    let core_id = pactum_core::id();
    let (user_main_pda, _) = Pubkey::find_program_address(
        &[USER_MAIN_SEED.as_bytes(), user.pubkey().as_ref()],
        &core_id,
    );
    let ix = Instruction::new_with_bytes(
        core_id,
        &pactum_core::instruction::InitUserMain {}.data(),
        pactum_core::accounts::InitUserMain {
            user_main: user_main_pda,
            authority: user.pubkey(),
            payer: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    send_ix(svm, ix, payer, &[payer, user]).unwrap();
    user_main_pda
}

pub fn create_marketplace_account(
    svm: &mut LiteSVM,
    market_id: u64,
    committee: Pubkey,
    dispute_mode: DisputeMode,
    requires_identity: bool,
    payer: &Keypair,
) -> Pubkey {
    let core_id = pactum_core::id();
    let (market_pda, _) = Pubkey::find_program_address(
        &[
            MARKETPLACE_SEED.as_bytes(),
            market_id.to_le_bytes().as_ref(),
        ],
        &core_id,
    );
    let ix = Instruction::new_with_bytes(
        core_id,
        &pactum_core::instruction::CreateMarketplace {
            market_id,
            params: CreateMarketplaceParams {
                name: format!("Market {market_id}"),
                committee,
                dispute_mode,
                storage_backend: StorageBackend::Arweave,
                integrator_policy: pactum_core::state::IntegratorPolicy::Permissionless,
                kyb_credential: None,
                client_requires_kyc: requires_identity,
                provider_identity: pactum_core::state::RoleIdentityConfig::none(),
                judge_identity: pactum_core::state::RoleIdentityConfig::none(),
                min_judge_stake: 100,
                max_judge_stake: 1000,
                min_arbiter_stake: 500,
                min_jury_fee: 2_000_000,
                provider_cancellation_penalty_pct: 10,
            },
        }
        .data(),
        pactum_core::accounts::CreateMarketplace {
            marketplace: market_pda,
            payer: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    send_ix(svm, ix, payer, &[payer]).unwrap();
    market_pda
}

pub fn create_provider_account(
    svm: &mut LiteSVM,
    market_pda: Pubkey,
    _user_main_pda: Pubkey,
    authority: &Keypair,
    payer: &Keypair,
) -> Pubkey {
    let core_id = pactum_core::id();
    let (provider_pda, _) = Pubkey::find_program_address(
        &[
            PROVIDER_SEED.as_bytes(),
            market_pda.as_ref(),
            authority.pubkey().as_ref(),
        ],
        &core_id,
    );
    let (global_config_pda, _) =
        Pubkey::find_program_address(&[GLOBAL_CONFIG_SEED.as_bytes()], &pactum_config::id());
    let ix = Instruction::new_with_bytes(
        core_id,
        &pactum_core::instruction::InitProvider {}.data(),
        pactum_core::accounts::InitProvider {
            provider: provider_pda,
            marketplace: market_pda,
            user_main: None,
            global_config: global_config_pda,
            marketplace_attestation: None,
            marketplace_credential: None,
            authority: authority.pubkey(),
            payer: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    send_ix(svm, ix, payer, &[payer, authority]).unwrap();
    provider_pda
}

pub fn create_offer_account(
    svm: &mut LiteSVM,
    provider_pda: Pubkey,
    _user_main_pda: Pubkey,
    market_pda: Pubkey,
    offer_id: u32,
    authority: &Keypair,
    payer: &Keypair,
) -> Pubkey {
    let core_id = pactum_core::id();
    let (offer_pda, _) = Pubkey::find_program_address(
        &[
            OFFER_SEED.as_bytes(),
            provider_pda.as_ref(),
            offer_id.to_le_bytes().as_ref(),
        ],
        &core_id,
    );
    let ix = Instruction::new_with_bytes(
        core_id,
        &pactum_core::instruction::CreateOffer {
            params: CreateOfferParams {
                base_price: Some(50_000_000),
                pricing_type: PricingType::Fixed,
                approval_window_secs: 86_400,
                bond: 0,
                jury_fee: 2_500_000,
                default_arbiter: None,
                cancellation_policy: None,
                content_ref: [1u8; 32],
            },
        }
        .data(),
        pactum_core::accounts::CreateOffer {
            offer: offer_pda,
            provider: provider_pda,
            marketplace: market_pda,
            authority: authority.pubkey(),
            payer: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    send_ix(svm, ix, payer, &[payer, authority]).unwrap();
    offer_pda
}

pub fn serialize_sas_acc<T: AnchorSerialize>(discriminator: u8, val: &T) -> Vec<u8> {
    let mut out = vec![discriminator];
    val.serialize(&mut out).unwrap();
    out
}
