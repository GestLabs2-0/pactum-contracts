use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    constants::{CLIENT_SEED, PROVIDER_SEED, USER_MAIN_SEED},
    program::PactumCore,
    state::{AgreementOutcome, Client, IntegratorProfile, Provider, UserMainAcc},
};

use crate::{
    constants::{
        CPI_AUTHORITY_SEED, DATE_RANGE_AGREEMENT_SEED, ESCROW_CONFIG_SEED, ESCROW_VAULT_SEED,
        POST_CHECKOUT_RELEASE_WINDOW_SECS,
    },
    error::PactumEscrowError,
    events::ReservationReleased,
    state::{DateRangeAgreement, DateRangeStatus, EscrowConfig},
    utils::{
        calculate_fees, close_vault, cpi_record_outcomes, pay_integrator_if_needed,
        transfer_from_vault,
    },
};

#[derive(Accounts)]
pub struct ReleaseReservationFunds<'info> {
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
        seeds = [USER_MAIN_SEED.as_bytes(), client_user_main.authority.as_ref()],
        bump = client_user_main.bump,
        seeds::program = pactum_core::ID,
    )]
    pub client_user_main: Box<Account<'info, UserMainAcc>>,

    #[account(
        mut,
        seeds = [
            CLIENT_SEED.as_bytes(),
            client.marketplace.as_ref(),
            client_user_main.key().as_ref(),
        ],
        bump = client.bump,
        seeds::program = pactum_core::ID,
    )]
    pub client: Box<Account<'info, Client>>,

    #[account(
        seeds = [USER_MAIN_SEED.as_bytes(), provider_user_main.authority.as_ref()],
        bump = provider_user_main.bump,
        seeds::program = pactum_core::ID,
    )]
    pub provider_user_main: Box<Account<'info, UserMainAcc>>,

    #[account(
        mut,
        seeds = [
            PROVIDER_SEED.as_bytes(),
            provider.marketplace.as_ref(),
            provider_user_main.key().as_ref(),
        ],
        bump = provider.bump,
        seeds::program = pactum_core::ID,
    )]
    pub provider: Box<Account<'info, Provider>>,

    pub integrator: Option<Box<Account<'info, IntegratorProfile>>>,

    #[account(
        mut,
        seeds = [
            DATE_RANGE_AGREEMENT_SEED.as_bytes(),
            reservation.offer.as_ref(),
            reservation.client.as_ref(),
            reservation.check_in.to_le_bytes().as_ref(),
        ],
        bump = reservation.bump,
        has_one = client @ PactumEscrowError::Unauthorized,
        has_one = provider @ PactumEscrowError::Unauthorized,
        has_one = rent_payer @ PactumEscrowError::InvalidRentPayer,
        constraint = reservation.status == DateRangeStatus::Completed @ PactumEscrowError::InvalidDateRangeStatus,
    )]
    pub reservation: Box<Account<'info, DateRangeAgreement>>,

    #[account(
        mut,
        seeds = [ESCROW_VAULT_SEED.as_bytes(), reservation.key().as_ref()],
        bump = reservation.escrow_vault_bump,
    )]
    pub escrow_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = platform_vault.key() == global_config.platform_vault @ PactumEscrowError::InvalidPlatformVault,
    )]
    pub platform_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = provider_token_account.mint == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
        constraint = provider_token_account.owner == provider_user_main.authority @ PactumEscrowError::Unauthorized,
    )]
    pub provider_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = client_token_account.mint == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
        constraint = client_token_account.owner == client_user_main.authority @ PactumEscrowError::Unauthorized,
    )]
    pub client_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub integrator_token_account: Option<Box<InterfaceAccount<'info, TokenAccount>>>,

    #[account(
        constraint = usdc_mint.key() == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Verified via `has_one = rent_payer` on `reservation`.
    #[account(mut)]
    pub rent_payer: UncheckedAccount<'info>,

    /// CHECK: Escrow CPI authority PDA signing outcome CPIs into `pactum-core`.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub core_program: Program<'info, PactumCore>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_release_reservation_funds(ctx: Context<ReleaseReservationFunds>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let reservation = &mut ctx.accounts.reservation;
    let release_at = reservation
        .updated_at
        .checked_add(POST_CHECKOUT_RELEASE_WINDOW_SECS)
        .ok_or(PactumEscrowError::MathOverflow)?;
    require!(
        now >= release_at,
        PactumEscrowError::ReleaseWindowNotElapsed
    );

    let fees = calculate_fees(
        reservation.total_amount,
        ctx.accounts.global_config.protocol_fee_bps,
        ctx.accounts.global_config.max_protocol_fee,
        reservation.integrator.is_some(),
        reservation.integrator_fee_bps,
    )?;

    let key = reservation.key();
    let vault_bump = reservation.escrow_vault_bump;
    let total_provider = fees
        .provider_payout
        .checked_add(reservation.provider_bond)
        .ok_or(PactumEscrowError::MathOverflow)?;

    transfer_from_vault(
        &ctx.accounts.token_program,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.platform_vault,
        &ctx.accounts.usdc_mint,
        key,
        vault_bump,
        fees.protocol_fee,
    )?;
    pay_integrator_if_needed(
        reservation.integrator,
        fees.integrator_fee,
        &ctx.accounts.integrator,
        &ctx.accounts.integrator_token_account,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.usdc_mint,
        &ctx.accounts.token_program,
        ctx.accounts.global_config.usdc_mint,
        key,
        vault_bump,
    )?;
    transfer_from_vault(
        &ctx.accounts.token_program,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.provider_token_account,
        &ctx.accounts.usdc_mint,
        key,
        vault_bump,
        total_provider,
    )?;
    transfer_from_vault(
        &ctx.accounts.token_program,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.client_token_account,
        &ctx.accounts.usdc_mint,
        key,
        vault_bump,
        reservation.client_bond,
    )?;
    close_vault(
        &ctx.accounts.token_program,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.rent_payer.to_account_info(),
        key,
        vault_bump,
    )?;

    reservation.status = DateRangeStatus::Released;
    reservation.updated_at = now;

    cpi_record_outcomes(
        &ctx.accounts.core_program,
        &ctx.accounts.client,
        &ctx.accounts.provider,
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.to_account_info(),
        ctx.bumps.cpi_authority,
        AgreementOutcome::Completed,
    )?;

    emit!(ReservationReleased {
        reservation: key,
        total_amount: reservation.total_amount,
        protocol_fee: fees.protocol_fee,
        integrator_fee: fees.integrator_fee,
        provider_payout: fees.provider_payout,
    });
    Ok(())
}
