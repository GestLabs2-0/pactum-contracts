use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use pactum_config::{assert_cpi_authority, AllowedCaller, GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    constants::MARKETPLACE_SEED,
    program::PactumCore,
    state::{ArbiterProfile, Judge, Marketplace, ParticipantRole},
};

use crate::{
    constants::{ARBITER_VAULT_SEED, CPI_AUTHORITY_SEED, JUDGE_VAULT_SEED, TREASURY_CONFIG_SEED},
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

    #[account(
        seeds = [MARKETPLACE_SEED.as_bytes(), marketplace.market_id.to_le_bytes().as_ref()],
        bump = marketplace.bump,
        seeds::program = pactum_core::ID,
    )]
    pub marketplace: Box<Account<'info, Marketplace>>,

    #[account(mut)]
    pub judge: Option<Box<Account<'info, Judge>>>,

    #[account(mut)]
    pub arbiter_profile: Option<Box<Account<'info, ArbiterProfile>>>,

    #[account(mut)]
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

    let marketplace_key = ctx.accounts.marketplace.key();
    let expected_vault_seed = match target_role {
        ParticipantRole::Judge => JUDGE_VAULT_SEED.as_bytes(),
        ParticipantRole::Arbiter => ARBITER_VAULT_SEED.as_bytes(),
        ParticipantRole::Client | ParticipantRole::Provider | ParticipantRole::Integrator => {
            return err!(PactumTreasuryError::InvalidTargetRole);
        }
    };
    let (expected_vault, vault_bump) = Pubkey::find_program_address(
        &[expected_vault_seed, marketplace_key.as_ref()],
        ctx.program_id,
    );
    require_keys_eq!(
        ctx.accounts.stake_vault.key(),
        expected_vault,
        PactumTreasuryError::InvalidStakeVault
    );

    let target_key = match target_role {
        ParticipantRole::Judge => {
            let judge = ctx
                .accounts
                .judge
                .as_ref()
                .ok_or(PactumTreasuryError::MissingTargetAccount)?;
            require_keys_eq!(
                judge.marketplace,
                marketplace_key,
                PactumTreasuryError::InvalidMarketplace
            );
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
            // TODO: move cpi call to disputes instead of making a long cpi call
            pactum_core::cpi::cpi_update_judge_stake(cpi_core, amount, false, pool_id, leaf_index)?;
            key
        }
        ParticipantRole::Arbiter => {
            let arbiter = ctx
                .accounts
                .arbiter_profile
                .as_ref()
                .ok_or(PactumTreasuryError::MissingTargetAccount)?;
            require_keys_eq!(
                arbiter.marketplace,
                marketplace_key,
                PactumTreasuryError::InvalidMarketplace
            );
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
        ParticipantRole::Client | ParticipantRole::Provider | ParticipantRole::Integrator => {
            return err!(PactumTreasuryError::InvalidTargetRole);
        }
    };

    let vault_seeds: &[&[&[u8]]] =
        &[&[expected_vault_seed, marketplace_key.as_ref(), &[vault_bump]]];
    let cpi_transfer = CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        TransferChecked {
            from: ctx.accounts.stake_vault.to_account_info(),
            to: ctx.accounts.recipient_token_account.to_account_info(),
            authority: ctx.accounts.stake_vault.to_account_info(),
            mint: ctx.accounts.usdc_mint.to_account_info(),
        },
        vault_seeds,
    );
    transfer_checked(cpi_transfer, amount, ctx.accounts.usdc_mint.decimals)?;

    emit!(StakeSlashed {
        marketplace: marketplace_key,
        target: target_key,
        role: target_role,
        amount,
        recipient: ctx.accounts.recipient_token_account.key(),
    });
    Ok(())
}
