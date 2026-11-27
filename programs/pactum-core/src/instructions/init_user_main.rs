use anchor_lang::prelude::*;

use crate::{constants::USER_MAIN_SEED, state::UserMainAcc};

#[derive(Accounts)]
pub struct InitUserMain<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + UserMainAcc::INIT_SPACE,
        seeds = [USER_MAIN_SEED.as_bytes(), authority.key().as_ref()],
        bump,
    )]
    pub user_main: Account<'info, UserMainAcc>,

    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_init_user_main(ctx: Context<InitUserMain>) -> Result<()> {
    let user_main = &mut ctx.accounts.user_main;
    user_main.authority = ctx.accounts.authority.key();
    user_main.sas_attestation = None;
    user_main.bump = ctx.bumps.user_main;
    Ok(())
}
