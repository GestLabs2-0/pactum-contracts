use anchor_lang::prelude::*;
use pactum_config::{assert_cpi_authority, AllowedCaller, GlobalConfig, GLOBAL_CONFIG_SEED};

use super::helpers::apply_i8_delta;
use crate::{
    error::PactumCoreError,
    state::{ArbiterProfile, Judge, ParticipantRole},
};

#[derive(Accounts)]
pub struct CpiUpdateActiveDisputes<'info> {
    #[account(mut)]
    pub judge: Option<Account<'info, Judge>>,

    #[account(mut)]
    pub arbiter_profile: Option<Account<'info, ArbiterProfile>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub cpi_authority: Signer<'info>,
}

pub fn handler_cpi_update_active_disputes(
    ctx: Context<CpiUpdateActiveDisputes>,
    target_role: ParticipantRole,
    delta: i8,
) -> Result<()> {
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Disputes],
    )?;

    match target_role {
        ParticipantRole::Judge => {
            let judge = ctx
                .accounts
                .judge
                .as_mut()
                .ok_or(PactumCoreError::MissingRoleAccount)?;
            judge.active_disputes = apply_i8_delta(judge.active_disputes, delta)?;
        }
        ParticipantRole::Arbiter => {
            let arbiter = ctx
                .accounts
                .arbiter_profile
                .as_mut()
                .ok_or(PactumCoreError::MissingRoleAccount)?;
            arbiter.active_disputes = apply_i8_delta(arbiter.active_disputes, delta)?;
        }
        ParticipantRole::Client | ParticipantRole::Provider => {
            return err!(PactumCoreError::InvalidRole);
        }
    }
    Ok(())
}
