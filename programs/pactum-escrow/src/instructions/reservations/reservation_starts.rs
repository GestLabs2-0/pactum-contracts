use anchor_lang::prelude::*;

use crate::{
    constants::DATE_RANGE_AGREEMENT_SEED,
    error::PactumEscrowError,
    events::ReservationStarted,
    state::{DateRangeAgreement, DateRangeStatus},
};

#[derive(Accounts)]
pub struct ReservationStarts<'info> {
    pub caller: Signer<'info>,

    #[account(
        mut,
        seeds = [
            DATE_RANGE_AGREEMENT_SEED.as_bytes(),
            reservation.offer.as_ref(),
            reservation.client.as_ref(),
            reservation.check_in.to_le_bytes().as_ref(),
        ],
        bump = reservation.bump,
        constraint = reservation.status == DateRangeStatus::Accepted @ PactumEscrowError::InvalidDateRangeStatus,
    )]
    pub reservation: Box<Account<'info, DateRangeAgreement>>,
}

pub fn handler_reservation_starts(ctx: Context<ReservationStarts>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let reservation = &mut ctx.accounts.reservation;
    require!(
        now >= reservation.check_in,
        PactumEscrowError::TimestampNotReached
    );

    reservation.status = DateRangeStatus::Active;
    reservation.updated_at = now;

    emit!(ReservationStarted {
        reservation: reservation.key(),
        updated_at: now,
    });
    Ok(())
}
