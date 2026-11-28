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
    state::{Agreement, AgreementStatus, EscrowConfig},
    utils::refund_and_close_proposed_agreement,
};

#[derive(Accounts)]
pub struct CancelProposal<'info> {
    pub caller: Signer<'info>,

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
        seeds = [USER_MAIN_SEED.as_bytes(), caller.key().as_ref()],
        bump = caller_user_main.bump,
        seeds::program = pactum_core::ID,
    )]
    pub caller_user_main: Box<Account<'info, UserMainAcc>>,

    #[account(
        seeds = [USER_MAIN_SEED.as_bytes(), provider_user_main.authority.as_ref()],
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
        seeds = [
            CLIENT_SEED.as_bytes(),
            client.marketplace.as_ref(),
            client.user_main.as_ref(),
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
        constraint = agreement.status == AgreementStatus::Proposed @ PactumEscrowError::InvalidAgreementStatus,
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
        constraint = provider_token_account.mint == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
        constraint = provider_token_account.owner == provider_user_main.authority @ PactumEscrowError::Unauthorized,
    )]
    pub provider_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = usdc_mint.key() == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Verified via `has_one = rent_payer` on `agreement`.
    #[account(mut)]
    pub rent_payer: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_cancel_proposal(ctx: Context<CancelProposal>) -> Result<()> {
    let caller_um = ctx.accounts.caller_user_main.key();
    require!(
        caller_um == ctx.accounts.provider.user_main || caller_um == ctx.accounts.client.user_main,
        PactumEscrowError::Unauthorized
    );

    refund_and_close_proposed_agreement(
        &ctx.accounts.agreement,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.provider_token_account,
        &ctx.accounts.usdc_mint,
        &ctx.accounts.rent_payer.to_account_info(),
        &ctx.accounts.token_program,
        "proposal_cancelled",
    )
}
