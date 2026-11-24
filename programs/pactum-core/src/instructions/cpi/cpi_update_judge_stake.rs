use anchor_lang::prelude::*;
use stayke_config::{assert_cpi_authority, AllowedCaller, GlobalConfig, GLOBAL_CONFIG_SEED};

use super::helpers::apply_stake_delta;
use crate::state::Judge;

#[derive(Accounts)]
pub struct CpiUpdateJudgeStake<'info> {
    #[account(mut)]
    pub judge: Account<'info, Judge>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub cpi_authority: Signer<'info>,
}

pub fn handler_cpi_update_judge_stake(
    ctx: Context<CpiUpdateJudgeStake>,
    amount: u64,
    is_stake: bool,
    pool_id: u16,
    leaf_index: u32,
) -> Result<()> {
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Treasury],
    )?;

    let judge = &mut ctx.accounts.judge;
    judge.staked_amount = apply_stake_delta(judge.staked_amount, amount, is_stake)?;
    judge.pool_id = pool_id;
    judge.leaf_index = leaf_index;
    judge.is_active_in_pool = judge.staked_amount > 0;
    Ok(())
}
