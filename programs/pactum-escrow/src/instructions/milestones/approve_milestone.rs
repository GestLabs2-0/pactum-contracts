use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    constants::{CLIENT_SEED, PROVIDER_SEED},
    program::PactumCore,
    state::{Client, IntegratorProfile, Provider},
};

use crate::{
    constants::{AGREEMENT_SEED, CPI_AUTHORITY_SEED, ESCROW_CONFIG_SEED, ESCROW_VAULT_SEED},
    error::PactumEscrowError,
    state::{Agreement, AgreementStatus, EscrowConfig},
    utils::release_current_milestone,
};

#[derive(Accounts)]
pub struct ApproveMilestone<'info> {
    pub client_authority: Signer<'info>,

    #[account(
        seeds = [ESCROW_CONFIG_SEED.as_bytes()],
        bump = escrow_config.bump,
        constraint = escrow_config.is_initialized @ PactumEscrowError::EscrowNotInitialized,
    )]
    pub escrow_config: Box<Account<'info, EscrowConfig>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
        constraint = global_config.key() == escrow_config.global_config @ PactumEscrowError::EscrowNotInitialized,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        seeds = [
            CLIENT_SEED.as_bytes(),
            client.marketplace.as_ref(),
            client_authority.key().as_ref(),
        ],
        bump = client.bump,
        seeds::program = pactum_core::ID,
        constraint = client.authority == client_authority.key() @ PactumEscrowError::Unauthorized,
    )]
    pub client: Box<Account<'info, Client>>,

    #[account(
        mut,
        seeds = [
            PROVIDER_SEED.as_bytes(),
            provider.marketplace.as_ref(),
            provider.authority.as_ref(),
        ],
        bump = provider.bump,
        seeds::program = pactum_core::ID,
    )]
    pub provider: Box<Account<'info, Provider>>,

    pub integrator: Option<Box<Account<'info, IntegratorProfile>>>,

    #[account(
        mut,
        seeds = [
            AGREEMENT_SEED.as_bytes(),
            agreement.offer.as_ref(),
            agreement.client.as_ref(),
            agreement.nonce.to_le_bytes().as_ref(),
        ],
        bump = agreement.bump,
        has_one = client @ PactumEscrowError::Unauthorized,
        has_one = provider @ PactumEscrowError::Unauthorized,
        has_one = rent_payer @ PactumEscrowError::InvalidRentPayer,
        constraint = agreement.status == AgreementStatus::Submitted @ PactumEscrowError::InvalidAgreementStatus,
    )]
    pub agreement: Box<Account<'info, Agreement>>,

    #[account(
        mut,
        seeds = [ESCROW_VAULT_SEED.as_bytes(), agreement.key().as_ref()],
        bump = agreement.escrow_vault_bump,
    )]
    pub escrow_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = platform_vault.key() == global_config.platform_vault @ PactumEscrowError::InvalidPlatformVault,
    )]
    pub platform_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = provider_token_account.mint == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
        constraint = provider_token_account.owner == provider.authority @ PactumEscrowError::Unauthorized,
    )]
    pub provider_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = client_token_account.mint == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
        constraint = client_token_account.owner == client.authority @ PactumEscrowError::Unauthorized,
    )]
    pub client_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub integrator_token_account: Option<Box<InterfaceAccount<'info, TokenAccount>>>,

    #[account(
        constraint = usdc_mint.key() == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Verified via `has_one = rent_payer` on `agreement`.
    #[account(mut)]
    pub rent_payer: UncheckedAccount<'info>,

    /// CHECK: Escrow CPI authority PDA signing outcome CPIs into `pactum-core`.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub core_program: Program<'info, PactumCore>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_approve_milestone(ctx: Context<ApproveMilestone>) -> Result<()> {
    release_current_milestone(
        &mut ctx.accounts.agreement,
        &ctx.accounts.global_config,
        &ctx.accounts.client,
        &ctx.accounts.provider,
        &ctx.accounts.integrator,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.platform_vault,
        &ctx.accounts.provider_token_account,
        &ctx.accounts.client_token_account,
        &ctx.accounts.integrator_token_account,
        &ctx.accounts.usdc_mint,
        &ctx.accounts.rent_payer.to_account_info(),
        &ctx.accounts.cpi_authority.to_account_info(),
        ctx.bumps.cpi_authority,
        &ctx.accounts.core_program,
        &ctx.accounts.token_program,
    )
}
