use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    constants::{CLIENT_SEED, PROVIDER_SEED},
    program::PactumCore,
    state::{AgreementOutcome, Client, Provider},
};

use crate::{
    constants::{AGREEMENT_SEED, CPI_AUTHORITY_SEED, ESCROW_CONFIG_SEED, ESCROW_VAULT_SEED},
    error::PactumEscrowError,
    events::AgreementCancelled,
    state::{Agreement, AgreementStatus, EscrowConfig},
    utils::{close_vault, cpi_record_outcomes, transfer_from_vault},
};

#[derive(Accounts)]
pub struct CancelAgreement<'info> {
    pub caller: Signer<'info>,

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
        mut,
        seeds = [
            CLIENT_SEED.as_bytes(),
            client.marketplace.as_ref(),
            client.authority.as_ref(),
        ],
        bump = client.bump,
        seeds::program = pactum_core::ID,
    )]
    pub client: Box<Account<'info, Client>>,

    #[account(
        mut,
        seeds = [
            PROVIDER_SEED.as_bytes(),
            provider.marketplace.as_ref(),
            provider.authority.as_ref(),
        ],
        bump = provider.bump,
        seeds::program = pactum_core::ID,
    )]
    pub provider: Box<Account<'info, Provider>>,

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
        has_one = provider @ PactumEscrowError::Unauthorized,
        has_one = rent_payer @ PactumEscrowError::InvalidRentPayer,
        constraint = agreement.status == AgreementStatus::Funded @ PactumEscrowError::InvalidAgreementStatus,
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
        constraint = client_token_account.owner == client.authority @ PactumEscrowError::Unauthorized,
    )]
    pub client_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = provider_token_account.mint == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
        constraint = provider_token_account.owner == provider.authority @ PactumEscrowError::Unauthorized,
    )]
    pub provider_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = usdc_mint.key() == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Verified via `has_one = rent_payer` on `agreement`.
    #[account(mut)]
    pub rent_payer: UncheckedAccount<'info>,

    /// CHECK: Escrow CPI authority PDA signing outcome CPIs into `pactum-core`.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub core_program: Program<'info, PactumCore>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_cancel_agreement(ctx: Context<CancelAgreement>) -> Result<()> {
    let caller = ctx.accounts.caller.key();
    require!(
        caller == ctx.accounts.client.authority || caller == ctx.accounts.provider.authority,
        PactumEscrowError::Unauthorized
    );

    let agreement = &mut ctx.accounts.agreement;
    require!(
        agreement.completed_milestones == 0,
        PactumEscrowError::MilestonesAlreadyStarted
    );

    let client_refund = agreement
        .total_amount
        .checked_add(agreement.client_bond)
        .ok_or(PactumEscrowError::MathOverflow)?
        .checked_add(agreement.jury_fee)
        .ok_or(PactumEscrowError::MathOverflow)?;
    let provider_refund = agreement
        .provider_bond
        .checked_add(agreement.jury_fee)
        .ok_or(PactumEscrowError::MathOverflow)?;
    let agreement_key = agreement.key();
    let vault_bump = agreement.escrow_vault_bump;

    transfer_from_vault(
        &ctx.accounts.token_program,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.client_token_account,
        &ctx.accounts.usdc_mint,
        agreement_key,
        vault_bump,
        client_refund,
    )?;
    transfer_from_vault(
        &ctx.accounts.token_program,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.provider_token_account,
        &ctx.accounts.usdc_mint,
        agreement_key,
        vault_bump,
        provider_refund,
    )?;
    close_vault(
        &ctx.accounts.token_program,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.rent_payer.to_account_info(),
        agreement_key,
        vault_bump,
    )?;

    agreement.status = AgreementStatus::Cancelled;
    agreement.completed_at = Clock::get()?.unix_timestamp;

    cpi_record_outcomes(
        &ctx.accounts.core_program,
        &ctx.accounts.client,
        &ctx.accounts.provider,
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.to_account_info(),
        ctx.bumps.cpi_authority,
        AgreementOutcome::Cancelled,
    )?;

    emit!(AgreementCancelled {
        agreement: agreement_key,
        reason: "pre_delivery_cancelled".to_string(),
    });
    Ok(())
}
