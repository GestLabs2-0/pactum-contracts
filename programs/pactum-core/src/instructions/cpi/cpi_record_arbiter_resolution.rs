use anchor_lang::prelude::*;
use stayke_config::{assert_cpi_authority, AllowedCaller, GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{error::PactumCoreError, state::ArbiterProfile};

#[derive(Accounts)]
pub struct CpiRecordArbiterResolution<'info> {
    #[account(mut)]
    pub arbiter_profile: Account<'info, ArbiterProfile>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub cpi_authority: Signer<'info>,
}

pub fn handler_cpi_record_arbiter_resolution(
    ctx: Context<CpiRecordArbiterResolution>,
) -> Result<()> {
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Disputes],
    )?;

    let arbiter = &mut ctx.accounts.arbiter_profile;
    arbiter.cases_resolved = arbiter
        .cases_resolved
        .checked_add(1)
        .ok_or(PactumCoreError::MathOverflow)?;
    Ok(())
}
