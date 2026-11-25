use anchor_lang::prelude::*;
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{constants::CORE_CONFIG_SEED, error::PactumCoreError, state::CoreConfig};

#[derive(Accounts)]
pub struct InitializeCoreConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + CoreConfig::INIT_SPACE,
        seeds = [CORE_CONFIG_SEED.as_bytes()],
        bump,
    )]
    pub core_config: Account<'info, CoreConfig>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
        constraint = global_config.authority == authority.key() @ PactumCoreError::Unauthorized,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_core_config(ctx: Context<InitializeCoreConfig>) -> Result<()> {
    let config = &mut ctx.accounts.core_config;
    config.authority = ctx.accounts.authority.key();
    config.global_config = ctx.accounts.global_config.key();
    config.bump = ctx.bumps.core_config;
    Ok(())
}
