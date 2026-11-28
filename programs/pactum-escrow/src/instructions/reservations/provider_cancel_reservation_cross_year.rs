use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    program::PactumCore,
    state::{Client, Provider, UserMainAcc},
};

use crate::{
    constants::{
        BOOKING_DAYS_SEED, CPI_AUTHORITY_SEED, DATE_RANGE_AGREEMENT_SEED, ESCROW_CONFIG_SEED,
        ESCROW_VAULT_SEED,
    },
    error::PactumEscrowError,
    state::{BookingDays, DateRangeAgreement, DateRangeStatus, EscrowConfig},
    utils::{release_days_cross_years, settle_provider_cancel_reservation, TimestampExt},
};

#[derive(Accounts)]
pub struct ProviderCancelReservationCrossYear<'info> {
    pub provider_authority: Signer<'info>,

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
        constraint = provider_user_main.authority == provider_authority.key() @ PactumEscrowError::Unauthorized,
    )]
    pub provider_user_main: Box<Account<'info, UserMainAcc>>,

    #[account(
        mut,
        constraint = provider.user_main == provider_user_main.key() @ PactumEscrowError::Unauthorized,
    )]
    pub provider: Box<Account<'info, Provider>>,

    pub client_user_main: Box<Account<'info, UserMainAcc>>,

    #[account(
        mut,
        constraint = client.user_main == client_user_main.key() @ PactumEscrowError::Unauthorized,
    )]
    pub client: Box<Account<'info, Client>>,

    #[account(mut)]
    pub booking_days: Box<Account<'info, BookingDays>>,

    #[account(mut)]
    pub booking_days_next: Box<Account<'info, BookingDays>>,

    #[account(
        mut,
        seeds = [
            DATE_RANGE_AGREEMENT_SEED.as_bytes(),
            reservation.offer.as_ref(),
            reservation.client.as_ref(),
            reservation.check_in.to_le_bytes().as_ref(),
        ],
        bump = reservation.bump,
        has_one = provider @ PactumEscrowError::Unauthorized,
        has_one = client @ PactumEscrowError::Unauthorized,
        has_one = rent_payer @ PactumEscrowError::InvalidRentPayer,
        constraint = reservation.status == DateRangeStatus::Accepted @ PactumEscrowError::InvalidDateRangeStatus,
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
        constraint = client_token_account.mint == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
        constraint = client_token_account.owner == client_user_main.authority @ PactumEscrowError::Unauthorized,
    )]
    pub client_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = provider_token_account.mint == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
        constraint = provider_token_account.owner == provider_user_main.authority @ PactumEscrowError::Unauthorized,
    )]
    pub provider_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

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

pub fn handler_provider_cancel_reservation_cross_year(
    ctx: Context<ProviderCancelReservationCrossYear>,
) -> Result<()> {
    let check_in_date = ctx.accounts.reservation.check_in.to_date()?;
    let check_out_date = ctx.accounts.reservation.check_out.to_date()?;
    let (expected_bd, _) = Pubkey::find_program_address(
        &[
            BOOKING_DAYS_SEED.as_bytes(),
            ctx.accounts.reservation.offer.as_ref(),
            check_in_date.year.to_le_bytes().as_ref(),
        ],
        &crate::ID,
    );
    let (expected_bd_next, _) = Pubkey::find_program_address(
        &[
            BOOKING_DAYS_SEED.as_bytes(),
            ctx.accounts.reservation.offer.as_ref(),
            check_out_date.year.to_le_bytes().as_ref(),
        ],
        &crate::ID,
    );
    require_keys_eq!(
        ctx.accounts.booking_days.key(),
        expected_bd,
        PactumEscrowError::InvalidBookingDaysAccount
    );
    require_keys_eq!(
        ctx.accounts.booking_days_next.key(),
        expected_bd_next,
        PactumEscrowError::InvalidBookingDaysAccount
    );

    release_days_cross_years(
        &mut ctx.accounts.booking_days,
        &mut ctx.accounts.booking_days_next,
        &check_in_date,
        &check_out_date,
    )?;

    settle_provider_cancel_reservation(
        &mut ctx.accounts.reservation,
        &ctx.accounts.global_config,
        &ctx.accounts.client,
        &ctx.accounts.provider,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.client_token_account,
        &ctx.accounts.provider_token_account,
        &ctx.accounts.usdc_mint,
        &ctx.accounts.rent_payer.to_account_info(),
        &ctx.accounts.cpi_authority.to_account_info(),
        ctx.bumps.cpi_authority,
        &ctx.accounts.core_program,
        &ctx.accounts.token_program,
    )
}
