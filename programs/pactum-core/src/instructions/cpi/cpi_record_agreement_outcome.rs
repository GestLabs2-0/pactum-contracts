use anchor_lang::prelude::*;
use pactum_config::{assert_cpi_authority, AllowedCaller, GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    error::PactumCoreError,
    state::{AgreementOutcome, Client, ParticipantRole, Provider},
};

#[derive(Accounts)]
pub struct CpiRecordAgreementOutcome<'info> {
    #[account(mut)]
    pub client: Option<Account<'info, Client>>,

    #[account(mut)]
    pub provider: Option<Account<'info, Provider>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub cpi_authority: Signer<'info>,
}

pub fn handler_cpi_record_agreement_outcome(
    ctx: Context<CpiRecordAgreementOutcome>,
    role: ParticipantRole,
    outcome: AgreementOutcome,
) -> Result<()> {
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Escrow],
    )?;

    match role {
        ParticipantRole::Client => {
            let client = ctx
                .accounts
                .client
                .as_mut()
                .ok_or(PactumCoreError::MissingRoleAccount)?;
            match outcome {
                AgreementOutcome::Completed => {
                    client.completed_agreements = client
                        .completed_agreements
                        .checked_add(1)
                        .ok_or(PactumCoreError::MathOverflow)?;
                }
                AgreementOutcome::Cancelled => {
                    client.cancelled_agreements = client
                        .cancelled_agreements
                        .checked_add(1)
                        .ok_or(PactumCoreError::MathOverflow)?;
                }
            }
        }
        ParticipantRole::Provider => {
            let provider = ctx
                .accounts
                .provider
                .as_mut()
                .ok_or(PactumCoreError::MissingRoleAccount)?;
            match outcome {
                AgreementOutcome::Completed => {
                    provider.completed_agreements = provider
                        .completed_agreements
                        .checked_add(1)
                        .ok_or(PactumCoreError::MathOverflow)?;
                }
                AgreementOutcome::Cancelled => {
                    provider.cancelled_agreements = provider
                        .cancelled_agreements
                        .checked_add(1)
                        .ok_or(PactumCoreError::MathOverflow)?;
                }
            }
        }
        ParticipantRole::Judge | ParticipantRole::Arbiter => {
            return err!(PactumCoreError::InvalidRole);
        }
    }
    Ok(())
}
