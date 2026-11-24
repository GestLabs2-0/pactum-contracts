use anchor_lang::prelude::*;
use stayke_config::{assert_cpi_authority, AllowedCaller, GlobalConfig, GLOBAL_CONFIG_SEED};

use super::helpers::apply_stake_delta;
use crate::state::ArbiterProfile;

#[derive(Accounts)]
pub struct CpiUpdateArbiterStake<'info> {
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

pub fn handler_cpi_update_arbiter_stake(
    ctx: Context<CpiUpdateArbiterStake>,
    amount: u64,
    is_stake: bool,
) -> Result<()> {
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Treasury],
    )?;

    let arbiter = &mut ctx.accounts.arbiter_profile;
    arbiter.staked_amount = apply_stake_delta(arbiter.staked_amount, amount, is_stake)?;
    Ok(())
}
