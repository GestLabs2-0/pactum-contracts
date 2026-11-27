use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    constants::{ARBITER_SEED, MARKETPLACE_SEED},
    program::PactumCore,
    state::{ArbiterProfile, Marketplace, ParticipantRole},
};

use crate::{
    constants::{ARBITER_VAULT_SEED, CPI_AUTHORITY_SEED, TREASURY_CONFIG_SEED},
    error::PactumTreasuryError,
    events::StakeUpdated,
    state::TreasuryConfig,
};

#[derive(Accounts)]
pub struct UnstakeArbiter<'info> {
    pub authority: Signer<'info>,

    #[account(
        seeds = [MARKETPLACE_SEED.as_bytes(), marketplace.market_id.to_le_bytes().as_ref()],
        bump = marketplace.bump,
        seeds::program = pactum_core::ID,
    )]
    pub marketplace: Box<Account<'info, Marketplace>>,

    #[account(
        mut,
        seeds = [ARBITER_SEED.as_bytes(), marketplace.key().as_ref(), authority.key().as_ref()],
        bump = arbiter_profile.bump,
        seeds::program = pactum_core::ID,
        has_one = marketplace,
        has_one = authority @ PactumTreasuryError::Unauthorized,
    )]
    pub arbiter_profile: Box<Account<'info, ArbiterProfile>>,

    #[account(
        seeds = [TREASURY_CONFIG_SEED.as_bytes()],
        bump = config.bump,
        constraint = config.is_initialized @ PactumTreasuryError::InvalidTreasuryConfig,
    )]
    pub config: Box<Account<'info, TreasuryConfig>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        seeds = [ARBITER_VAULT_SEED.as_bytes(), marketplace.key().as_ref()],
        bump,
    )]
    pub arbiter_stake_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_token_account.mint == global_config.usdc_mint @ PactumTreasuryError::InvalidTokenMint,
    )]
    pub user_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = usdc_mint.key() == global_config.usdc_mint @ PactumTreasuryError::InvalidTokenMint,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Treasury CPI authority PDA that signs CPI calls into `pactum-core`.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub core_program: Program<'info, PactumCore>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_unstake_arbiter(ctx: Context<UnstakeArbiter>, amount: u64) -> Result<()> {
    require!(amount > 0, PactumTreasuryError::ZeroAmount);
    require!(
        ctx.accounts.arbiter_profile.active_disputes == 0,
        PactumTreasuryError::ActiveDisputesExist
    );
    require!(
        ctx.accounts.arbiter_profile.staked_amount >= amount,
        PactumTreasuryError::InsufficientStake
    );

    let remaining = ctx
        .accounts
        .arbiter_profile
        .staked_amount
        .checked_sub(amount)
        .ok_or(PactumTreasuryError::MathOverflow)?;
    if remaining > 0 {
        require!(
            remaining >= ctx.accounts.marketplace.min_arbiter_stake,
            PactumTreasuryError::StakeBelowMinimum
        );
    }

    let marketplace_key = ctx.accounts.marketplace.key();
    let vault_seeds: &[&[&[u8]]] = &[&[
        ARBITER_VAULT_SEED.as_bytes(),
        marketplace_key.as_ref(),
        &[ctx.bumps.arbiter_stake_vault],
    ]];
    let cpi_transfer = CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        TransferChecked {
            from: ctx.accounts.arbiter_stake_vault.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.arbiter_stake_vault.to_account_info(),
            mint: ctx.accounts.usdc_mint.to_account_info(),
        },
        vault_seeds,
    );
    transfer_checked(cpi_transfer, amount, ctx.accounts.usdc_mint.decimals)?;

    let cpi_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[ctx.bumps.cpi_authority]]];
    let cpi_core = CpiContext::new_with_signer(
        ctx.accounts.core_program.key(),
        pactum_core::cpi::accounts::CpiUpdateArbiterStake {
            arbiter_profile: ctx.accounts.arbiter_profile.to_account_info(),
            global_config: ctx.accounts.global_config.to_account_info(),
            cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
        },
        cpi_seeds,
    );
    pactum_core::cpi::cpi_update_arbiter_stake(cpi_core, amount, false)?;

    emit!(StakeUpdated {
        marketplace: ctx.accounts.marketplace.key(),
        target: ctx.accounts.arbiter_profile.key(),
        role: ParticipantRole::Arbiter,
        amount,
        is_stake: false,
        new_total: remaining,
    });
    Ok(())
}
