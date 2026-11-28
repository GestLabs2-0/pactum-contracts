use anchor_lang::prelude::*;
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    constants::ESCROW_CONFIG_SEED, error::PactumEscrowError, events::EscrowInitialized,
    state::EscrowConfig,
};

#[derive(Accounts)]
pub struct InitializeEscrow<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + EscrowConfig::INIT_SPACE,
        seeds = [ESCROW_CONFIG_SEED.as_bytes()],
        bump,
    )]
    pub config: Account<'info, EscrowConfig>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
        constraint = global_config.authority == authority.key() @ PactumEscrowError::Unauthorized,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_escrow(ctx: Context<InitializeEscrow>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    require!(
        !config.is_initialized,
        PactumEscrowError::AlreadyInitialized
    );

    config.authority = ctx.accounts.authority.key();
    config.global_config = ctx.accounts.global_config.key();
    config.is_initialized = true;
    config.bump = ctx.bumps.config;

    emit!(EscrowInitialized {
        authority: config.authority,
        global_config: config.global_config,
    });
    Ok(())
}
