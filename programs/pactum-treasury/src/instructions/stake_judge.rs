use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    constants::{JUDGE_SEED, MARKETPLACE_SEED, USER_MAIN_SEED},
    program::PactumCore,
    state::{Judge, Marketplace, ParticipantRole, UserMainAcc},
};

use crate::{
    constants::{CPI_AUTHORITY_SEED, JUDGE_VAULT_SEED, TREASURY_CONFIG_SEED},
    error::PactumTreasuryError,
    events::StakeUpdated,
    state::TreasuryConfig,
};

#[derive(Accounts)]
pub struct StakeJudge<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [USER_MAIN_SEED.as_bytes(), authority.key().as_ref()],
        bump = user_main.bump,
        seeds::program = pactum_core::ID,
        has_one = authority @ PactumTreasuryError::Unauthorized,
        constraint = !user_main.is_banned @ PactumTreasuryError::UserGloballyBanned,
    )]
    pub user_main: Box<Account<'info, UserMainAcc>>,

    #[account(
        seeds = [MARKETPLACE_SEED.as_bytes(), marketplace.market_id.to_le_bytes().as_ref()],
        bump = marketplace.bump,
        seeds::program = pactum_core::ID,
    )]
    pub marketplace: Box<Account<'info, Marketplace>>,

    #[account(
        mut,
        seeds = [JUDGE_SEED.as_bytes(), marketplace.key().as_ref(), user_main.key().as_ref()],
        bump = judge.bump,
        seeds::program = pactum_core::ID,
        has_one = marketplace,
        has_one = user_main,
        constraint = !judge.is_banned @ PactumTreasuryError::JudgeBanned,
    )]
    pub judge: Box<Account<'info, Judge>>,

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
        constraint = user_token_account.mint == global_config.usdc_mint @ PactumTreasuryError::InvalidTokenMint,
    )]
    pub user_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = authority,
        token::mint = usdc_mint,
        token::authority = judge_stake_vault,
        seeds = [JUDGE_VAULT_SEED.as_bytes(), marketplace.key().as_ref()],
        bump,
    )]
    pub judge_stake_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = usdc_mint.key() == global_config.usdc_mint @ PactumTreasuryError::InvalidTokenMint,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Treasury CPI authority PDA that signs CPI calls into `pactum-core`.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub core_program: Program<'info, PactumCore>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handler_stake_judge(
    ctx: Context<StakeJudge>,
    amount: u64,
    pool_id: u16,
    leaf_index: u32,
) -> Result<()> {
    require!(amount > 0, PactumTreasuryError::ZeroAmount);

    let new_total = ctx
        .accounts
        .judge
        .staked_amount
        .checked_add(amount)
        .ok_or(PactumTreasuryError::MathOverflow)?;
    require!(
        new_total >= ctx.accounts.marketplace.min_judge_stake,
        PactumTreasuryError::StakeBelowMinimum
    );
    let max_stake = ctx.accounts.marketplace.max_judge_stake;
    if max_stake > 0 {
        require!(
            new_total <= max_stake,
            PactumTreasuryError::StakeAboveMaximum
        );
    }

    let cpi_transfer = CpiContext::new(
        ctx.accounts.token_program.key(),
        TransferChecked {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.judge_stake_vault.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
            mint: ctx.accounts.usdc_mint.to_account_info(),
        },
    );
    transfer_checked(cpi_transfer, amount, ctx.accounts.usdc_mint.decimals)?;

    let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[ctx.bumps.cpi_authority]]];
    let cpi_core = CpiContext::new_with_signer(
        ctx.accounts.core_program.key(),
        pactum_core::cpi::accounts::CpiUpdateJudgeStake {
            judge: ctx.accounts.judge.to_account_info(),
            global_config: ctx.accounts.global_config.to_account_info(),
            cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
        },
        signer_seeds,
    );
    pactum_core::cpi::cpi_update_judge_stake(cpi_core, amount, true, pool_id, leaf_index)?;

    emit!(StakeUpdated {
        marketplace: ctx.accounts.marketplace.key(),
        target: ctx.accounts.judge.key(),
        role: ParticipantRole::Judge,
        amount,
        is_stake: true,
        new_total,
    });
    Ok(())
}
