use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use pactum_config::{assert_cpi_authority, AllowedCaller, GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    program::PactumCore,
    state::{ArbiterProfile, Judge, ParticipantRole},
};

use crate::{
    constants::{CPI_AUTHORITY_SEED, TREASURY_CONFIG_SEED, TREASURY_SEED},
    error::PactumTreasuryError,
    events::StakeSlashed,
    state::TreasuryConfig,
};

#[derive(Accounts)]
pub struct CpiSlashStake<'info> {
    pub cpi_authority: Signer<'info>,

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

    #[account(mut)]
    pub judge: Option<Box<Account<'info, Judge>>>,

    #[account(mut)]
    pub arbiter_profile: Option<Box<Account<'info, ArbiterProfile>>>,

    /// CHECK: Treasury PDA that signs the SPL Token transfer out of `stake_vault`.
    #[account(seeds = [TREASURY_SEED.as_bytes()], bump = config.treasury_bump)]
    pub treasury_pda: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = stake_vault.key() == config.stake_vault @ PactumTreasuryError::InvalidStakeVault,
    )]
    pub stake_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = recipient_token_account.mint == global_config.usdc_mint @ PactumTreasuryError::InvalidTokenMint,
    )]
    pub recipient_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = usdc_mint.key() == global_config.usdc_mint @ PactumTreasuryError::InvalidTokenMint,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Treasury CPI authority PDA that signs CPI calls into `pactum-core`.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub treasury_cpi_authority: UncheckedAccount<'info>,

    pub core_program: Program<'info, PactumCore>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_cpi_slash_stake(
    ctx: Context<CpiSlashStake>,
    target_role: ParticipantRole,
    amount: u64,
) -> Result<()> {
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Disputes],
    )?;
    require!(amount > 0, PactumTreasuryError::ZeroAmount);

    let cpi_seeds: &[&[&[u8]]] = &[&[
        CPI_AUTHORITY_SEED.as_bytes(),
        &[ctx.bumps.treasury_cpi_authority],
    ]];

    let target_key = match target_role {
        ParticipantRole::Judge => {
            let judge = ctx
                .accounts
                .judge
                .as_ref()
                .ok_or(PactumTreasuryError::MissingTargetAccount)?;
            require!(
                judge.staked_amount >= amount,
                PactumTreasuryError::InsufficientStake
            );
            let pool_id = judge.pool_id;
            let leaf_index = judge.leaf_index;
            let key = judge.key();

            let cpi_core = CpiContext::new_with_signer(
                ctx.accounts.core_program.key(),
                pactum_core::cpi::accounts::CpiUpdateJudgeStake {
                    judge: judge.to_account_info(),
                    global_config: ctx.accounts.global_config.to_account_info(),
                    cpi_authority: ctx.accounts.treasury_cpi_authority.to_account_info(),
                },
                cpi_seeds,
            );
            pactum_core::cpi::cpi_update_judge_stake(cpi_core, amount, false, pool_id, leaf_index)?;
            key
        }
        ParticipantRole::Arbiter => {
            let arbiter = ctx
                .accounts
                .arbiter_profile
                .as_ref()
                .ok_or(PactumTreasuryError::MissingTargetAccount)?;
            require!(
                arbiter.staked_amount >= amount,
                PactumTreasuryError::InsufficientStake
            );
            let key = arbiter.key();

            let cpi_core = CpiContext::new_with_signer(
                ctx.accounts.core_program.key(),
                pactum_core::cpi::accounts::CpiUpdateArbiterStake {
                    arbiter_profile: arbiter.to_account_info(),
                    global_config: ctx.accounts.global_config.to_account_info(),
                    cpi_authority: ctx.accounts.treasury_cpi_authority.to_account_info(),
                },
                cpi_seeds,
            );
            pactum_core::cpi::cpi_update_arbiter_stake(cpi_core, amount, false)?;
            key
        }
        ParticipantRole::Client | ParticipantRole::Provider => {
            return err!(PactumTreasuryError::InvalidTargetRole);
        }
    };

    let treasury_seeds: &[&[&[u8]]] = &[&[
        TREASURY_SEED.as_bytes(),
        &[ctx.accounts.config.treasury_bump],
    ]];
    let cpi_transfer = CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        TransferChecked {
            from: ctx.accounts.stake_vault.to_account_info(),
            to: ctx.accounts.recipient_token_account.to_account_info(),
            authority: ctx.accounts.treasury_pda.to_account_info(),
            mint: ctx.accounts.usdc_mint.to_account_info(),
        },
        treasury_seeds,
    );
    transfer_checked(cpi_transfer, amount, ctx.accounts.usdc_mint.decimals)?;

    emit!(StakeSlashed {
        target: target_key,
        role: target_role,
        amount,
        recipient: ctx.accounts.recipient_token_account.key(),
    });
    Ok(())
}
