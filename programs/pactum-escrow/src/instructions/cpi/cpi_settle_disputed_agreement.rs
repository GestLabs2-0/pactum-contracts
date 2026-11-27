use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use pactum_config::{assert_cpi_authority, AllowedCaller, GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    program::PactumCore,
    state::{AgreementOutcome, Client, Provider},
};

use crate::{
    constants::{CPI_AUTHORITY_SEED, ESCROW_CONFIG_SEED, ESCROW_VAULT_SEED},
    error::PactumEscrowError,
    events::DisputeSettled,
    state::{Agreement, AgreementStatus, DateRangeAgreement, DateRangeStatus, EscrowConfig},
    utils::{close_vault, cpi_record_outcomes, transfer_from_vault},
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct SettleDisputedParams {
    pub restore_previous_status: bool,
    pub client_payout: u64,
    pub provider_payout: u64,
    pub fee_payout: u64,
    pub mark_completed: bool,
}

#[derive(Accounts)]
pub struct CpiSettleDisputedAgreement<'info> {
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

    /// CHECK: Verified via `assert_cpi_authority(..., &[AllowedCaller::Disputes])`.
    pub cpi_authority: Signer<'info>,

    #[account(mut)]
    pub client: Box<Account<'info, Client>>,

    #[account(mut)]
    pub provider: Box<Account<'info, Provider>>,

    #[account(mut)]
    pub agreement: Option<Box<Account<'info, Agreement>>>,

    #[account(mut)]
    pub reservation: Option<Box<Account<'info, DateRangeAgreement>>>,

    #[account(mut)]
    pub escrow_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = client_token_account.mint == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
    )]
    pub client_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = provider_token_account.mint == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
    )]
    pub provider_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = platform_vault.key() == global_config.platform_vault @ PactumEscrowError::InvalidPlatformVault,
    )]
    pub platform_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub fee_destination: Option<Box<InterfaceAccount<'info, TokenAccount>>>,

    #[account(
        constraint = usdc_mint.key() == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Verified against `agreement.rent_payer` / `reservation.rent_payer` in handler.
    #[account(mut)]
    pub rent_payer: UncheckedAccount<'info>,

    /// CHECK: Escrow CPI authority PDA signing outcome CPIs into `pactum-core`.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub escrow_cpi_authority: UncheckedAccount<'info>,

    pub core_program: Program<'info, PactumCore>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_cpi_settle_disputed_agreement(
    ctx: Context<CpiSettleDisputedAgreement>,
    params: SettleDisputedParams,
) -> Result<()> {
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Disputes],
    )?;

    let now = Clock::get()?.unix_timestamp;
    let (target_key, vault_bump, expected_rent_payer, expected_client, expected_provider) =
        match (&mut ctx.accounts.agreement, &mut ctx.accounts.reservation) {
            (Some(agreement), None) => {
                require!(
                    agreement.status == AgreementStatus::Disputed,
                    PactumEscrowError::InvalidAgreementStatus
                );
                if params.restore_previous_status {
                    let prev = agreement
                        .previous_status
                        .take()
                        .ok_or(PactumEscrowError::MissingPreviousStatus)?;
                    agreement.status = prev;
                    return Ok(());
                }
                agreement.status = if params.mark_completed {
                    AgreementStatus::Completed
                } else {
                    AgreementStatus::Cancelled
                };
                agreement.completed_at = now;
                (
                    agreement.key(),
                    agreement.escrow_vault_bump,
                    agreement.rent_payer,
                    agreement.client,
                    agreement.provider,
                )
            }
            (None, Some(reservation)) => {
                require!(
                    reservation.status == DateRangeStatus::Disputed,
                    PactumEscrowError::InvalidDateRangeStatus
                );
                if params.restore_previous_status {
                    let prev = reservation
                        .previous_status
                        .take()
                        .ok_or(PactumEscrowError::MissingPreviousStatus)?;
                    reservation.status = prev;
                    return Ok(());
                }
                reservation.status = if params.mark_completed {
                    DateRangeStatus::Released
                } else {
                    DateRangeStatus::Cancelled
                };
                reservation.updated_at = now;
                (
                    reservation.key(),
                    reservation.escrow_vault_bump,
                    reservation.rent_payer,
                    reservation.client,
                    reservation.provider,
                )
            }
            _ => return err!(PactumEscrowError::InvalidAgreementStatus),
        };

    require_keys_eq!(
        ctx.accounts.rent_payer.key(),
        expected_rent_payer,
        PactumEscrowError::InvalidRentPayer
    );
    require_keys_eq!(
        ctx.accounts.client.key(),
        expected_client,
        PactumEscrowError::Unauthorized
    );
    require_keys_eq!(
        ctx.accounts.provider.key(),
        expected_provider,
        PactumEscrowError::Unauthorized
    );

    let (expected_vault, _) = Pubkey::find_program_address(
        &[ESCROW_VAULT_SEED.as_bytes(), target_key.as_ref()],
        &crate::ID,
    );
    require_keys_eq!(
        ctx.accounts.escrow_vault.key(),
        expected_vault,
        PactumEscrowError::InvalidTokenMint
    );

    let total_payout = params
        .client_payout
        .checked_add(params.provider_payout)
        .ok_or(PactumEscrowError::MathOverflow)?
        .checked_add(params.fee_payout)
        .ok_or(PactumEscrowError::MathOverflow)?;
    require!(
        total_payout <= ctx.accounts.escrow_vault.amount,
        PactumEscrowError::InvalidSettlementSplit
    );

    transfer_from_vault(
        &ctx.accounts.token_program,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.client_token_account,
        &ctx.accounts.usdc_mint,
        target_key,
        vault_bump,
        params.client_payout,
    )?;
    transfer_from_vault(
        &ctx.accounts.token_program,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.provider_token_account,
        &ctx.accounts.usdc_mint,
        target_key,
        vault_bump,
        params.provider_payout,
    )?;
    if params.fee_payout > 0 {
        let dest = ctx
            .accounts
            .fee_destination
            .as_deref()
            .unwrap_or(&ctx.accounts.platform_vault);
        require!(
            dest.mint == ctx.accounts.global_config.usdc_mint,
            PactumEscrowError::InvalidTokenMint
        );
        transfer_from_vault(
            &ctx.accounts.token_program,
            &ctx.accounts.escrow_vault,
            dest,
            &ctx.accounts.usdc_mint,
            target_key,
            vault_bump,
            params.fee_payout,
        )?;
    }

    close_vault(
        &ctx.accounts.token_program,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.rent_payer.to_account_info(),
        target_key,
        vault_bump,
    )?;

    let outcome = if params.mark_completed {
        AgreementOutcome::Completed
    } else {
        AgreementOutcome::Cancelled
    };
    cpi_record_outcomes(
        &ctx.accounts.core_program,
        &ctx.accounts.client,
        &ctx.accounts.provider,
        &ctx.accounts.global_config,
        &ctx.accounts.escrow_cpi_authority.to_account_info(),
        ctx.bumps.escrow_cpi_authority,
        outcome,
    )?;

    emit!(DisputeSettled {
        target_agreement: target_key,
        client_payout: params.client_payout,
        provider_payout: params.provider_payout,
        fee_payout: params.fee_payout,
    });
    Ok(())
}
