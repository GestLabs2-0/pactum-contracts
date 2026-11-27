use anchor_lang::prelude::*;
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    cpi::{accounts::CpiUpdateRoleReview, cpi_update_role_review},
    program::PactumCore,
    state::{Client, ParticipantRole, Provider},
};

use crate::{
    constants::{CPI_AUTHORITY_SEED, ESCROW_CONFIG_SEED, MAX_REVIEW_RATING, MIN_REVIEW_RATING},
    error::PactumEscrowError,
    events::CounterpartyReviewed,
    state::{Agreement, AgreementStatus, DateRangeAgreement, DateRangeStatus, EscrowConfig},
};

#[derive(Accounts)]
pub struct ReviewCounterparty<'info> {
    pub reviewer: Signer<'info>,

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

    #[account(mut)]
    pub client: Box<Account<'info, Client>>,

    #[account(mut)]
    pub provider: Box<Account<'info, Provider>>,

    #[account(mut)]
    pub agreement: Option<Box<Account<'info, Agreement>>>,

    #[account(mut)]
    pub reservation: Option<Box<Account<'info, DateRangeAgreement>>>,

    /// CHECK: Escrow CPI authority PDA signing review CPIs into `pactum-core`.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub core_program: Program<'info, PactumCore>,
}

pub fn handler_review_counterparty(ctx: Context<ReviewCounterparty>, rating: u8) -> Result<()> {
    require!(
        (MIN_REVIEW_RATING..=MAX_REVIEW_RATING).contains(&rating),
        PactumEscrowError::InvalidRating
    );

    let reviewer = ctx.accounts.reviewer.key();
    let is_client = reviewer == ctx.accounts.client.authority;
    let is_provider = reviewer == ctx.accounts.provider.authority;
    require!(is_client || is_provider, PactumEscrowError::Unauthorized);

    let target_key = match (&mut ctx.accounts.agreement, &mut ctx.accounts.reservation) {
        (Some(agreement), None) => {
            require!(
                agreement.status == AgreementStatus::Completed,
                PactumEscrowError::InvalidAgreementStatus
            );
            require_keys_eq!(
                agreement.client,
                ctx.accounts.client.key(),
                PactumEscrowError::Unauthorized
            );
            require_keys_eq!(
                agreement.provider,
                ctx.accounts.provider.key(),
                PactumEscrowError::Unauthorized
            );
            if is_client {
                require!(
                    !agreement.client_reviewed,
                    PactumEscrowError::AlreadyReviewed
                );
                agreement.client_reviewed = true;
            } else {
                require!(
                    !agreement.provider_reviewed,
                    PactumEscrowError::AlreadyReviewed
                );
                agreement.provider_reviewed = true;
            }
            agreement.key()
        }
        (None, Some(reservation)) => {
            require!(
                reservation.status == DateRangeStatus::Released,
                PactumEscrowError::InvalidDateRangeStatus
            );
            require_keys_eq!(
                reservation.client,
                ctx.accounts.client.key(),
                PactumEscrowError::Unauthorized
            );
            require_keys_eq!(
                reservation.provider,
                ctx.accounts.provider.key(),
                PactumEscrowError::Unauthorized
            );
            if is_client {
                require!(
                    !reservation.client_reviewed,
                    PactumEscrowError::AlreadyReviewed
                );
                reservation.client_reviewed = true;
            } else {
                require!(
                    !reservation.provider_reviewed,
                    PactumEscrowError::AlreadyReviewed
                );
                reservation.provider_reviewed = true;
            }
            reservation.key()
        }
        _ => return err!(PactumEscrowError::InvalidAgreementStatus),
    };

    let seeds: &[&[u8]] = &[CPI_AUTHORITY_SEED.as_bytes(), &[ctx.bumps.cpi_authority]];
    let signer = &[seeds];

    let (target_role, cpi_accounts) = if is_client {
        (
            ParticipantRole::Provider,
            CpiUpdateRoleReview {
                client: None,
                provider: Some(ctx.accounts.provider.to_account_info()),
                global_config: ctx.accounts.global_config.to_account_info(),
                cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
            },
        )
    } else {
        (
            ParticipantRole::Client,
            CpiUpdateRoleReview {
                client: Some(ctx.accounts.client.to_account_info()),
                provider: None,
                global_config: ctx.accounts.global_config.to_account_info(),
                cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
            },
        )
    };

    let cpi_ctx =
        CpiContext::new_with_signer(ctx.accounts.core_program.key(), cpi_accounts, signer);
    cpi_update_role_review(cpi_ctx, target_role, rating)?;

    emit!(CounterpartyReviewed {
        target_agreement: target_key,
        reviewer: ctx.accounts.reviewer.key(),
        rating,
    });
    Ok(())
}
