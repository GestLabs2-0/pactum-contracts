use anchor_lang::prelude::*;

use crate::{
    constants::{INTEGRATOR_SEED, MARKETPLACE_SEED},
    error::PactumCoreError,
    state::{IntegratorProfile, Marketplace},
};

#[derive(Accounts)]
pub struct RevokeIntegrator<'info> {
    #[account(
        mut,
        seeds = [
            INTEGRATOR_SEED.as_bytes(),
            marketplace.key().as_ref(),
            integrator_profile.authority.as_ref(),
        ],
        bump = integrator_profile.bump,
        has_one = marketplace,
    )]
    pub integrator_profile: Account<'info, IntegratorProfile>,

    #[account(
        seeds = [MARKETPLACE_SEED.as_bytes(), marketplace.market_id.to_le_bytes().as_ref()],
        bump = marketplace.bump,
        has_one = committee @ PactumCoreError::Unauthorized,
    )]
    pub marketplace: Account<'info, Marketplace>,

    pub committee: Signer<'info>,
}

pub fn handler_revoke_integrator(ctx: Context<RevokeIntegrator>) -> Result<()> {
    ctx.accounts.integrator_profile.is_active = false;
    Ok(())
}
