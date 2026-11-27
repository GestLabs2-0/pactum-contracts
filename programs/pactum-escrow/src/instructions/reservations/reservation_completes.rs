use anchor_lang::prelude::*;

use crate::{
    constants::DATE_RANGE_AGREEMENT_SEED,
    error::PactumEscrowError,
    events::ReservationCompleted,
    state::{DateRangeAgreement, DateRangeStatus},
};

#[derive(Accounts)]
pub struct ReservationCompletes<'info> {
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
        constraint = reservation.status == DateRangeStatus::Active @ PactumEscrowError::InvalidDateRangeStatus,
    )]
    pub reservation: Box<Account<'info, DateRangeAgreement>>,
}

pub fn handler_reservation_completes(ctx: Context<ReservationCompletes>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let reservation = &mut ctx.accounts.reservation;
    require!(
        now >= reservation.check_out,
        PactumEscrowError::TimestampNotReached
    );

    reservation.status = DateRangeStatus::Completed;
    reservation.updated_at = now;

    emit!(ReservationCompleted {
        reservation: reservation.key(),
        updated_at: now,
    });
    Ok(())
}
