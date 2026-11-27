use anchor_lang::prelude::*;
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    constants::{PROVIDER_SEED, USER_MAIN_SEED},
    error::PactumCoreError,
    state::{Marketplace, Provider, UserMainAcc},
};

#[derive(Accounts)]
pub struct InitProvider<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + Provider::INIT_SPACE,
        seeds = [PROVIDER_SEED.as_bytes(), marketplace.key().as_ref(), user_main.key().as_ref()],
        bump,
    )]
    pub provider: Account<'info, Provider>,

    pub marketplace: Account<'info, Marketplace>,

    #[account(
        seeds = [USER_MAIN_SEED.as_bytes(), authority.key().as_ref()],
        bump = user_main.bump,
        has_one = authority @ PactumCoreError::Unauthorized,
    )]
    pub user_main: Account<'info, UserMainAcc>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: Optional SAS attestation verified when `provider_identity.policy == KycAndMarketplaceProof`.
    pub marketplace_attestation: Option<UncheckedAccount<'info>>,

    /// CHECK: Optional SAS credential verified when `provider_identity.policy == KycAndMarketplaceProof`.
    pub marketplace_credential: Option<UncheckedAccount<'info>>,

    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_init_provider(ctx: Context<InitProvider>) -> Result<()> {
    let marketplace_sas = ctx
        .accounts
        .marketplace_attestation
        .as_ref()
        .zip(ctx.accounts.marketplace_credential.as_ref())
        .map(|(a, c)| (a.as_ref(), c.as_ref()));

    ctx.accounts.marketplace.assert_role_eligible(
        &ctx.accounts.user_main,
        &ctx.accounts.marketplace.provider_identity,
        ctx.accounts.global_config.sas_program,
        marketplace_sas,
    )?;

    let provider = &mut ctx.accounts.provider;
    provider.marketplace = ctx.accounts.marketplace.key();
    provider.user_main = ctx.accounts.user_main.key();
    provider.is_banned = false;
    provider.offer_count = 0;
    provider.completed_agreements = 0;
    provider.cancelled_agreements = 0;
    provider.disputes_won = 0;
    provider.disputes_lost = 0;
    provider.total_reviews = 0;
    provider.total_score = 0;
    provider.bump = ctx.bumps.provider;
    Ok(())
}
