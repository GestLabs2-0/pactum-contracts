use anchor_lang::prelude::*;

use crate::{
    constants::{INTEGRATOR_SEED, MARKETPLACE_SEED, USER_MAIN_SEED},
    error::PactumCoreError,
    events::IntegratorInitialized,
    state::{IntegratorPolicy, IntegratorProfile, Marketplace, UserMainAcc},
};

#[derive(Accounts)]
pub struct InitIntegrator<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + IntegratorProfile::INIT_SPACE,
        seeds = [
            INTEGRATOR_SEED.as_bytes(),
            marketplace.key().as_ref(),
            authority.key().as_ref(),
        ],
        bump,
    )]
    pub integrator_profile: Account<'info, IntegratorProfile>,

    #[account(
        seeds = [MARKETPLACE_SEED.as_bytes(), marketplace.market_id.to_le_bytes().as_ref()],
        bump = marketplace.bump,
        constraint = marketplace.is_active @ PactumCoreError::MarketplaceInactive,
        constraint = marketplace.integrator_policy == IntegratorPolicy::Permissionless @ PactumCoreError::InvalidIntegratorPolicy,
    )]
    pub marketplace: Account<'info, Marketplace>,

    #[account(
        seeds = [USER_MAIN_SEED.as_bytes(), authority.key().as_ref()],
        bump = user_main.bump,
        has_one = authority @ PactumCoreError::Unauthorized,
    )]
    pub user_main: Option<Account<'info, UserMainAcc>>,

    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_init_integrator(ctx: Context<InitIntegrator>, default_fee_bps: u16) -> Result<()> {
    require!(
        default_fee_bps <= 10_000,
        PactumCoreError::InvalidIntegratorParams
    );

    if let Some(user_main) = ctx.accounts.user_main.as_ref() {
        ctx.accounts
            .marketplace
            .assert_participant_eligible(user_main)?;
    } else if ctx.accounts.marketplace.requires_identity {
        return err!(PactumCoreError::IdentityRequired);
    }

    let profile = &mut ctx.accounts.integrator_profile;
    profile.marketplace = ctx.accounts.marketplace.key();
    profile.authority = ctx.accounts.authority.key();
    profile.sas_kyb_attestation = None;
    profile.default_fee_bps = default_fee_bps;
    profile.fallback_resolutions = 0;
    profile.is_active = true;
    profile.is_banned = false;
    profile.bump = ctx.bumps.integrator_profile;

    emit!(IntegratorInitialized {
        marketplace: profile.marketplace,
        authority: profile.authority,
        default_fee_bps,
    });
    Ok(())
}
