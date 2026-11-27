use anchor_lang::prelude::*;

use crate::{
    constants::{ARBITER_SEED, MARKETPLACE_SEED},
    error::PactumCoreError,
    state::{ArbiterProfile, Marketplace},
};

#[derive(Accounts)]
pub struct RevokeArbiter<'info> {
    #[account(
        mut,
        seeds = [
            ARBITER_SEED.as_bytes(),
            marketplace.key().as_ref(),
            arbiter_profile.authority.as_ref(),
        ],
        bump = arbiter_profile.bump,
        has_one = marketplace,
    )]
    pub arbiter_profile: Account<'info, ArbiterProfile>,

    #[account(
        seeds = [MARKETPLACE_SEED.as_bytes(), marketplace.market_id.to_le_bytes().as_ref()],
        bump = marketplace.bump,
        has_one = committee @ PactumCoreError::Unauthorized,
    )]
    pub marketplace: Account<'info, Marketplace>,

    pub committee: Signer<'info>,
}

pub fn handler_revoke_arbiter(ctx: Context<RevokeArbiter>) -> Result<()> {
    ctx.accounts.arbiter_profile.is_active = false;
    Ok(())
}
