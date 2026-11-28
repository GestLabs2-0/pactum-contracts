use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    constants::{PROVIDER_SEED, USER_MAIN_SEED},
    state::{Provider, UserMainAcc},
};

use crate::{
    constants::{
        ACCEPT_WINDOW_SECS, DATE_RANGE_AGREEMENT_SEED, ESCROW_CONFIG_SEED, ESCROW_VAULT_SEED,
    },
    error::PactumEscrowError,
    events::ReservationAccepted,
    state::{DateRangeAgreement, DateRangeStatus, EscrowConfig},
    utils::transfer_from_user,
};

#[derive(Accounts)]
pub struct ProviderAcceptReservation<'info> {
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
        seeds = [USER_MAIN_SEED.as_bytes(), provider_authority.key().as_ref()],
        bump = provider_user_main.bump,
        seeds::program = pactum_core::ID,
    )]
    pub provider_user_main: Box<Account<'info, UserMainAcc>>,

    #[account(
        seeds = [
            PROVIDER_SEED.as_bytes(),
            provider.marketplace.as_ref(),
            provider_user_main.key().as_ref(),
        ],
        bump = provider.bump,
        seeds::program = pactum_core::ID,
        constraint = !provider.is_banned @ PactumEscrowError::ParticipantBanned,
    )]
    pub provider: Box<Account<'info, Provider>>,

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
        constraint = provider_token_account.mint == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
    )]
    pub provider_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = usdc_mint.key() == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_provider_accept_reservation(ctx: Context<ProviderAcceptReservation>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let reservation = &mut ctx.accounts.reservation;
    let accept_deadline = reservation
        .updated_at
        .checked_add(ACCEPT_WINDOW_SECS)
        .ok_or(PactumEscrowError::MathOverflow)?;
    require!(
        now <= accept_deadline,
        PactumEscrowError::AcceptWindowExpired
    );

    if reservation.provider_bond > 0 {
        transfer_from_user(
            &ctx.accounts.token_program,
            &ctx.accounts.provider_token_account,
            &ctx.accounts.escrow_vault,
            &ctx.accounts.usdc_mint,
            &ctx.accounts.provider_authority,
            reservation.provider_bond,
        )?;
    }

    reservation.status = DateRangeStatus::Accepted;
    reservation.updated_at = now;

    emit!(ReservationAccepted {
        reservation: reservation.key(),
        updated_at: now,
    });
    Ok(())
}
