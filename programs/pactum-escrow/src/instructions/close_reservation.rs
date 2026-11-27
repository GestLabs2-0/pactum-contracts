use anchor_lang::prelude::*;

use crate::{
    constants::{DATE_RANGE_AGREEMENT_SEED, REVIEW_WINDOW_SECS},
    error::PactumEscrowError,
    events::ReservationClosed,
    state::{DateRangeAgreement, DateRangeStatus},
};

#[derive(Accounts)]
pub struct CloseReservation<'info> {
    pub caller: Signer<'info>,

    #[account(
        mut,
        close = rent_payer,
        seeds = [
            DATE_RANGE_AGREEMENT_SEED.as_bytes(),
            reservation.offer.as_ref(),
            reservation.client.as_ref(),
            reservation.check_in.to_le_bytes().as_ref(),
        ],
        bump = reservation.bump,
        has_one = rent_payer @ PactumEscrowError::InvalidRentPayer,
        constraint = (reservation.status == DateRangeStatus::Released
            || reservation.status == DateRangeStatus::Cancelled)
            @ PactumEscrowError::InvalidDateRangeStatus,
    )]
    pub reservation: Box<Account<'info, DateRangeAgreement>>,

    /// CHECK: Verified via `has_one = rent_payer` on `reservation`.
    #[account(mut)]
    pub rent_payer: UncheckedAccount<'info>,
}

pub fn handler_close_reservation(ctx: Context<CloseReservation>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let reservation = &ctx.accounts.reservation;
    let close_at = reservation
        .updated_at
        .checked_add(REVIEW_WINDOW_SECS)
        .ok_or(PactumEscrowError::MathOverflow)?;
    require!(now >= close_at, PactumEscrowError::ReviewWindowNotElapsed);

    emit!(ReservationClosed {
        reservation: reservation.key(),
        rent_payer: reservation.rent_payer,
    });
    Ok(())
}
