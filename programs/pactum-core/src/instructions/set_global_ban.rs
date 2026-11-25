use anchor_lang::prelude::*;
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    constants::USER_MAIN_SEED, error::PactumCoreError, events::GlobalBanUpdated, state::UserMainAcc,
};

#[derive(Accounts)]
pub struct SetGlobalBan<'info> {
    #[account(
        mut,
        seeds = [USER_MAIN_SEED.as_bytes(), user_main.authority.as_ref()],
        bump = user_main.bump,
    )]
    pub user_main: Account<'info, UserMainAcc>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
        constraint = global_config.authority == authority.key() @ PactumCoreError::Unauthorized,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub authority: Signer<'info>,
}

pub fn handler_set_global_ban(ctx: Context<SetGlobalBan>, banned: bool) -> Result<()> {
    let user_main = &mut ctx.accounts.user_main;
    user_main.is_banned = banned;

    emit!(GlobalBanUpdated {
        user_main: user_main.key(),
        authority: user_main.authority,
        is_banned: banned,
    });
    Ok(())
}
