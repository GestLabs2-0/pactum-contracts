use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{constants::CLIENT_SEED, state::Client};

use crate::{
    constants::{
        ACCEPT_WINDOW_SECS, BOOKING_DAYS_SEED, DATE_RANGE_AGREEMENT_SEED, ESCROW_CONFIG_SEED,
        ESCROW_VAULT_SEED,
    },
    error::PactumEscrowError,
    state::{BookingDays, DateRangeAgreement, DateRangeStatus, EscrowConfig},
    utils::{close_pending_reservation, release_days_single_year, TimestampExt},
};

#[derive(Accounts)]
pub struct ExpireReservation<'info> {
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
            BOOKING_DAYS_SEED.as_bytes(),
            reservation.offer.as_ref(),
            reservation.check_in.to_date()?.year.to_le_bytes().as_ref(),
        ],
        bump = booking_days.bump,
    )]
    pub booking_days: Box<Account<'info, BookingDays>>,

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
        has_one = rent_payer @ PactumEscrowError::InvalidRentPayer,
        constraint = reservation.status == DateRangeStatus::Pending @ PactumEscrowError::InvalidDateRangeStatus,
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
        constraint = client_token_account.owner == client.authority @ PactumEscrowError::Unauthorized,
    )]
    pub client_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = usdc_mint.key() == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Verified via `has_one = rent_payer` on `reservation`.
    #[account(mut)]
    pub rent_payer: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_expire_reservation(ctx: Context<ExpireReservation>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let expires_at = ctx
        .accounts
        .reservation
        .updated_at
        .checked_add(ACCEPT_WINDOW_SECS)
        .ok_or(PactumEscrowError::MathOverflow)?;
    require!(now > expires_at, PactumEscrowError::AcceptWindowNotElapsed);

    let check_in_date = ctx.accounts.reservation.check_in.to_date()?;
    let check_out_date = ctx.accounts.reservation.check_out.to_date()?;
    release_days_single_year(
        &mut ctx.accounts.booking_days,
        &check_in_date,
        &check_out_date,
    )?;

    close_pending_reservation(
        &mut ctx.accounts.reservation,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.client_token_account,
        &ctx.accounts.usdc_mint,
        &ctx.accounts.rent_payer.to_account_info(),
        &ctx.accounts.token_program,
    )
}
