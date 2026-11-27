use anchor_lang::prelude::*;

use crate::{
    constants::OFFER_SEED,
    error::PactumCoreError,
    state::{Offer, Provider},
};

#[derive(Accounts)]
pub struct CloseOffer<'info> {
    #[account(
        mut,
        close = rent_payer,
        seeds = [
            OFFER_SEED.as_bytes(),
            provider.key().as_ref(),
            offer.offer_id.to_le_bytes().as_ref(),
        ],
        bump = offer.bump,
        has_one = rent_payer @ PactumCoreError::InvalidRentPayer,
    )]
    pub offer: Account<'info, Offer>,

    #[account(has_one = authority @ PactumCoreError::Unauthorized)]
    pub provider: Account<'info, Provider>,

    pub authority: Signer<'info>,

    /// CHECK: Verified via `has_one = rent_payer` on `offer`.
    #[account(mut)]
    pub rent_payer: UncheckedAccount<'info>,
}

pub fn handler_close_offer(_ctx: Context<CloseOffer>) -> Result<()> {
    Ok(())
}
