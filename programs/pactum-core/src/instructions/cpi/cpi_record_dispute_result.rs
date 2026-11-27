use anchor_lang::prelude::*;
use stayke_config::{assert_cpi_authority, AllowedCaller, GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    error::PactumCoreError,
    events::DisputeResultRecorded,
    state::{Client, DisputeParty, Provider},
};

#[derive(Accounts)]
pub struct CpiRecordDisputeResult<'info> {
    #[account(
        mut,
        constraint = client.marketplace == provider.marketplace @ PactumCoreError::Unauthorized,
    )]
    pub client: Account<'info, Client>,

    #[account(mut)]
    pub provider: Account<'info, Provider>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub cpi_authority: Signer<'info>,
}

pub fn handler_cpi_record_dispute_result(
    ctx: Context<CpiRecordDisputeResult>,
    winner: Option<DisputeParty>,
) -> Result<()> {
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Disputes],
    )?;

    let client = &mut ctx.accounts.client;
    let provider = &mut ctx.accounts.provider;

    match winner {
        Some(DisputeParty::Client) => {
            client.disputes_won = client
                .disputes_won
                .checked_add(1)
                .ok_or(PactumCoreError::MathOverflow)?;
            provider.disputes_lost = provider
                .disputes_lost
                .checked_add(1)
                .ok_or(PactumCoreError::MathOverflow)?;
        }
        Some(DisputeParty::Provider) => {
            provider.disputes_won = provider
                .disputes_won
                .checked_add(1)
                .ok_or(PactumCoreError::MathOverflow)?;
            client.disputes_lost = client
                .disputes_lost
                .checked_add(1)
                .ok_or(PactumCoreError::MathOverflow)?;
        }
        None => {}
    }

    emit!(DisputeResultRecorded {
        marketplace: client.marketplace,
        client: client.key(),
        provider: provider.key(),
        winner,
    });
    Ok(())
}
