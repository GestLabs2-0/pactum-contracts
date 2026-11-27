use anchor_lang::prelude::*;

use crate::{
    constants::{AGREEMENT_SEED, REVIEW_WINDOW_SECS},
    error::PactumEscrowError,
    events::AgreementClosed,
    state::{Agreement, AgreementStatus},
};

#[derive(Accounts)]
pub struct CloseAgreement<'info> {
    pub caller: Signer<'info>,

    #[account(
        mut,
        close = rent_payer,
        seeds = [
            AGREEMENT_SEED.as_bytes(),
            agreement.offer.as_ref(),
            agreement.client.as_ref(),
            agreement.nonce.to_le_bytes().as_ref(),
        ],
        bump = agreement.bump,
        has_one = rent_payer @ PactumEscrowError::InvalidRentPayer,
        constraint = (agreement.status == AgreementStatus::Completed
            || agreement.status == AgreementStatus::Cancelled)
            @ PactumEscrowError::InvalidAgreementStatus,
    )]
    pub agreement: Box<Account<'info, Agreement>>,

    /// CHECK: Verified via `has_one = rent_payer` on `agreement`.
    #[account(mut)]
    pub rent_payer: UncheckedAccount<'info>,
}

pub fn handler_close_agreement(ctx: Context<CloseAgreement>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let agreement = &ctx.accounts.agreement;
    let close_at = agreement
        .completed_at
        .checked_add(REVIEW_WINDOW_SECS)
        .ok_or(PactumEscrowError::MathOverflow)?;
    require!(now >= close_at, PactumEscrowError::ReviewWindowNotElapsed);

    emit!(AgreementClosed {
        agreement: agreement.key(),
        rent_payer: agreement.rent_payer,
    });
    Ok(())
}
