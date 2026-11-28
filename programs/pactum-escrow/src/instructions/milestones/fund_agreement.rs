use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{constants::CLIENT_SEED, state::Client};

use crate::{
    constants::{AGREEMENT_SEED, ESCROW_CONFIG_SEED, ESCROW_VAULT_SEED},
    error::PactumEscrowError,
    events::AgreementFunded,
    state::{Agreement, AgreementStatus, EscrowConfig},
    utils::transfer_from_user,
};

#[derive(Accounts)]
pub struct FundAgreement<'info> {
    pub client_authority: Signer<'info>,

    #[account(
        seeds = [ESCROW_CONFIG_SEED.as_bytes()],
        bump = escrow_config.bump,
        constraint = escrow_config.is_initialized @ PactumEscrowError::EscrowNotInitialized,
    )]
    pub escrow_config: Box<Account<'info, EscrowConfig>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
        constraint = global_config.key() == escrow_config.global_config @ PactumEscrowError::EscrowNotInitialized,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        seeds = [
            CLIENT_SEED.as_bytes(),
            client.marketplace.as_ref(),
            client_authority.key().as_ref(),
        ],
        bump = client.bump,
        seeds::program = pactum_core::ID,
        constraint = client.authority == client_authority.key() @ PactumEscrowError::Unauthorized,
        constraint = !client.is_banned @ PactumEscrowError::ParticipantBanned,
    )]
    pub client: Box<Account<'info, Client>>,

    #[account(
        mut,
        seeds = [
            AGREEMENT_SEED.as_bytes(),
            agreement.offer.as_ref(),
            agreement.client.as_ref(),
            agreement.nonce.to_le_bytes().as_ref(),
        ],
        bump = agreement.bump,
        has_one = client @ PactumEscrowError::Unauthorized,
        constraint = agreement.status == AgreementStatus::Proposed @ PactumEscrowError::InvalidAgreementStatus,
    )]
    pub agreement: Box<Account<'info, Agreement>>,

    #[account(
        mut,
        seeds = [ESCROW_VAULT_SEED.as_bytes(), agreement.key().as_ref()],
        bump = agreement.escrow_vault_bump,
    )]
    pub escrow_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = client_token_account.mint == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
    )]
    pub client_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = usdc_mint.key() == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_fund_agreement(
    ctx: Context<FundAgreement>,
    expected_contract_hash: Option<[u8; 32]>,
) -> Result<()> {
    let agreement = &mut ctx.accounts.agreement;
    require!(
        agreement.deal_contract_hash == expected_contract_hash,
        PactumEscrowError::InvalidContractHash
    );

    let deposit = agreement
        .total_amount
        .checked_add(agreement.client_bond)
        .ok_or(PactumEscrowError::MathOverflow)?
        .checked_add(agreement.jury_fee)
        .ok_or(PactumEscrowError::MathOverflow)?;

    transfer_from_user(
        &ctx.accounts.token_program,
        &ctx.accounts.client_token_account,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.usdc_mint,
        &ctx.accounts.client_authority,
        deposit,
    )?;

    let now = Clock::get()?.unix_timestamp;
    agreement.status = AgreementStatus::Funded;
    agreement.current_deadline_ts = now
        .checked_add(agreement.step_duration_secs)
        .ok_or(PactumEscrowError::MathOverflow)?;

    emit!(AgreementFunded {
        agreement: agreement.key(),
        deadline_ts: agreement.current_deadline_ts,
    });
    Ok(())
}
