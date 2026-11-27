use anchor_lang::prelude::*;
use pactum_config::{assert_cpi_authority, AllowedCaller, GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    constants::ESCROW_CONFIG_SEED,
    error::PactumEscrowError,
    events::DisputeFrozen,
    state::{Agreement, AgreementStatus, DateRangeAgreement, DateRangeStatus, EscrowConfig},
};

#[derive(Accounts)]
pub struct CpiFreezeAgreement<'info> {
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
    pub agreement: Option<Box<Account<'info, Agreement>>>,

    #[account(mut)]
    pub reservation: Option<Box<Account<'info, DateRangeAgreement>>>,
}

pub fn handler_cpi_freeze_agreement(ctx: Context<CpiFreezeAgreement>) -> Result<()> {
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Disputes],
    )?;

    let target_agreement = match (&mut ctx.accounts.agreement, &mut ctx.accounts.reservation) {
        (Some(agreement), None) => {
            require!(
                matches!(
                    agreement.status,
                    AgreementStatus::Funded | AgreementStatus::Submitted
                ),
                PactumEscrowError::InvalidAgreementStatus
            );
            agreement.previous_status = Some(agreement.status);
            agreement.status = AgreementStatus::Disputed;
            agreement.key()
        }
        (None, Some(reservation)) => {
            require!(
                matches!(
                    reservation.status,
                    DateRangeStatus::Accepted
                        | DateRangeStatus::Active
                        | DateRangeStatus::Completed
                ),
                PactumEscrowError::InvalidDateRangeStatus
            );
            reservation.previous_status = Some(reservation.status);
            reservation.status = DateRangeStatus::Disputed;
            reservation.key()
        }
        _ => return err!(PactumEscrowError::InvalidAgreementStatus),
    };

    emit!(DisputeFrozen { target_agreement });
    Ok(())
}
