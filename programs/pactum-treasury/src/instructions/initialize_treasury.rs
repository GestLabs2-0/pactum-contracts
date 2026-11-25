use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    constants::{STAKE_VAULT_SEED, TREASURY_CONFIG_SEED, TREASURY_SEED},
    error::PactumTreasuryError,
    events::TreasuryInitialized,
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

    /// CHECK: PDA that owns the stake vault.
    #[account(seeds = [TREASURY_SEED.as_bytes()], bump)]
    pub treasury_pda: UncheckedAccount<'info>,

    #[account(
        init,
        payer = authority,
        token::mint = usdc_mint,
        token::authority = treasury_pda,
        seeds = [STAKE_VAULT_SEED.as_bytes()],
        bump,
    )]
    pub stake_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        constraint = usdc_mint.key() == global_config.usdc_mint @ PactumTreasuryError::InvalidTokenMint,
    )]
    pub usdc_mint: InterfaceAccount<'info, Mint>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
        constraint = global_config.authority == authority.key() @ PactumTreasuryError::Unauthorized,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub token_program: Interface<'info, TokenInterface>,
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
    config.stake_vault = ctx.accounts.stake_vault.key();
    config.treasury_bump = ctx.bumps.treasury_pda;
    config.is_initialized = true;
    config.bump = ctx.bumps.config;

    emit!(TreasuryInitialized {
        authority: config.authority,
        global_config: config.global_config,
        stake_vault: config.stake_vault,
    });
    Ok(())
}
