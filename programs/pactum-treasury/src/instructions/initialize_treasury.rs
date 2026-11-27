use anchor_lang::prelude::*;
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    constants::TREASURY_CONFIG_SEED, error::PactumTreasuryError, events::TreasuryInitialized,
    state::TreasuryConfig,
};

#[derive(Accounts)]
pub struct InitializeTreasury<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + TreasuryConfig::INIT_SPACE,
        seeds = [TREASURY_CONFIG_SEED.as_bytes()],
        bump,
    )]
    pub config: Account<'info, TreasuryConfig>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
        constraint = global_config.authority == authority.key() @ PactumTreasuryError::Unauthorized,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_treasury(ctx: Context<InitializeTreasury>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    require!(
        !config.is_initialized,
        PactumTreasuryError::AlreadyInitialized
    );

    config.authority = ctx.accounts.authority.key();
    config.global_config = ctx.accounts.global_config.key();
    config.is_initialized = true;
    config.bump = ctx.bumps.config;

    emit!(TreasuryInitialized {
        authority: config.authority,
        global_config: config.global_config,
    });
    Ok(())
}
