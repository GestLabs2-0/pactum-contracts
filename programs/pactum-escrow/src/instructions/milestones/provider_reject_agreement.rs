use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    constants::{CLIENT_SEED, PROVIDER_SEED, USER_MAIN_SEED},
    state::{Client, Provider, UserMainAcc},
};

use crate::{
    constants::{AGREEMENT_SEED, ESCROW_CONFIG_SEED, ESCROW_VAULT_SEED},
    error::PactumEscrowError,
    events::AgreementCancelled,
    state::{Agreement, AgreementStatus, EscrowConfig},
    utils::{close_vault, transfer_from_vault},
};

#[derive(Accounts)]
pub struct ProviderRejectAgreement<'info> {
    pub provider_authority: Signer<'info>,

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
        seeds = [USER_MAIN_SEED.as_bytes(), provider_authority.key().as_ref()],
        bump = provider_user_main.bump,
        seeds::program = pactum_core::ID,
    )]
    pub provider_user_main: Box<Account<'info, UserMainAcc>>,

    #[account(
        seeds = [
            PROVIDER_SEED.as_bytes(),
            provider.marketplace.as_ref(),
            provider_user_main.key().as_ref(),
        ],
        bump = provider.bump,
        seeds::program = pactum_core::ID,
    )]
    pub provider: Box<Account<'info, Provider>>,

    #[account(
        seeds = [USER_MAIN_SEED.as_bytes(), client_user_main.authority.as_ref()],
        bump = client_user_main.bump,
        seeds::program = pactum_core::ID,
    )]
    pub client_user_main: Box<Account<'info, UserMainAcc>>,

    #[account(
        seeds = [
            CLIENT_SEED.as_bytes(),
            client.marketplace.as_ref(),
            client_user_main.key().as_ref(),
        ],
        bump = client.bump,
        seeds::program = pactum_core::ID,
    )]
    pub client: Box<Account<'info, Client>>,

    #[account(
        mut,
        close = rent_payer,
        seeds = [
            AGREEMENT_SEED.as_bytes(),
            agreement.offer.as_ref(),
            agreement.client.as_ref(),
            agreement.nonce.to_le_bytes().as_ref(),
        ],
        bump = agreement.bump,
        has_one = provider @ PactumEscrowError::Unauthorized,
        has_one = client @ PactumEscrowError::Unauthorized,
        has_one = rent_payer @ PactumEscrowError::InvalidRentPayer,
        constraint = agreement.status == AgreementStatus::Pending @ PactumEscrowError::InvalidAgreementStatus,
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
        constraint = client_token_account.mint == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
        constraint = client_token_account.owner == client_user_main.authority @ PactumEscrowError::Unauthorized,
    )]
    pub client_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = usdc_mint.key() == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Verified via `has_one = rent_payer` on `agreement`.
    #[account(mut)]
    pub rent_payer: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_provider_reject_agreement(ctx: Context<ProviderRejectAgreement>) -> Result<()> {
    let agreement = &ctx.accounts.agreement;
    let refund = agreement
        .total_amount
        .checked_add(agreement.client_bond)
        .ok_or(PactumEscrowError::MathOverflow)?;

    transfer_from_vault(
        &ctx.accounts.token_program,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.client_token_account,
        &ctx.accounts.usdc_mint,
        agreement.key(),
        agreement.escrow_vault_bump,
        refund,
    )?;

    close_vault(
        &ctx.accounts.token_program,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.rent_payer.to_account_info(),
        agreement.key(),
        agreement.escrow_vault_bump,
    )?;

    emit!(AgreementCancelled {
        agreement: agreement.key(),
        reason: "provider_rejected".to_string(),
    });
    Ok(())
}
