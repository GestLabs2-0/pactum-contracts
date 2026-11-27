use anchor_lang::prelude::*;

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

    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_init_provider(ctx: Context<InitProvider>) -> Result<()> {
    ctx.accounts
        .marketplace
        .assert_participant_eligible(&ctx.accounts.user_main)?;

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
