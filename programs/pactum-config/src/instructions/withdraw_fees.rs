use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

use crate::{
    constants::{GLOBAL_CONFIG_SEED, PLATFORM_VAULT_SEED},
    error::PactumConfigError,
    events::FeesWithdrawn,
    state::GlobalConfig,
};

#[derive(Accounts)]
pub struct WithdrawFees<'info> {
    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        constraint = authority.key() == global_config.authority @ PactumConfigError::Unauthorized,
    )]
    pub authority: Signer<'info>,

    /// CHECK: PDA that signs the SPL Token transfer. Validated by seeds + stored bump.
    #[account(
        seeds = [PLATFORM_VAULT_SEED.as_bytes()],
        bump = global_config.platform_vault_bump,
    )]
    pub platform_vault_pda: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = platform_vault.key() == global_config.platform_vault @ PactumConfigError::InvalidVaultAccount,
    )]
    pub platform_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        constraint = destination_token_account.mint == global_config.usdc_mint @ PactumConfigError::InvalidTokenMint,
    )]
    pub destination_token_account: InterfaceAccount<'info, TokenAccount>,

    pub usdc_mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_withdraw_fees(ctx: Context<WithdrawFees>, amount: u64) -> Result<()> {
    let global_config = &ctx.accounts.global_config;

    require!(
        global_config.is_initialized,
        PactumConfigError::InvalidGlobalConfig
    );
    require!(amount > 0, PactumConfigError::ZeroAmount);

    let bump = global_config.platform_vault_bump;
    let vault_seed = PLATFORM_VAULT_SEED.as_bytes();
    let signer_seeds: &[&[&[u8]]] = &[&[vault_seed, &[bump]]];

    let cpi_accounts = TransferChecked {
        from: ctx.accounts.platform_vault.to_account_info(),
        mint: ctx.accounts.usdc_mint.to_account_info(),
        to: ctx.accounts.destination_token_account.to_account_info(),
        authority: ctx.accounts.platform_vault_pda.to_account_info(),
    };

    let cpi_ctx =
        CpiContext::new_with_signer(ctx.accounts.token_program.key(), cpi_accounts, signer_seeds);

    transfer_checked(cpi_ctx, amount, ctx.accounts.usdc_mint.decimals)?;

    emit!(FeesWithdrawn {
        authority: ctx.accounts.authority.key(),
        destination: ctx.accounts.destination_token_account.key(),
        amount,
    });
    Ok(())
}
