use anchor_lang::prelude::*;

use crate::{
    constants::{CLIENT_SEED, USER_MAIN_SEED},
    error::PactumCoreError,
    state::{Client, Marketplace, UserMainAcc},
};

#[derive(Accounts)]
pub struct InitClient<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + Client::INIT_SPACE,
        seeds = [CLIENT_SEED.as_bytes(), marketplace.key().as_ref(), authority.key().as_ref()],
        bump,
    )]
    pub client: Account<'info, Client>,

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

pub fn handler_init_client(ctx: Context<InitClient>) -> Result<()> {
    ctx.accounts.marketplace.assert_client_eligible(
        ctx.accounts.user_main.as_deref(),
        ctx.accounts.authority.key(),
    )?;

    let client = &mut ctx.accounts.client;
    client.marketplace = ctx.accounts.marketplace.key();
    client.authority = ctx.accounts.authority.key();
    client.is_banned = false;
    client.completed_agreements = 0;
    client.cancelled_agreements = 0;
    client.disputes_won = 0;
    client.disputes_lost = 0;
    client.total_reviews = 0;
    client.total_score = 0;
    client.bump = ctx.bumps.client;
    Ok(())
}
