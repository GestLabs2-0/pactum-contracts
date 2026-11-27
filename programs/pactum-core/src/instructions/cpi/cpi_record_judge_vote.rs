use anchor_lang::prelude::*;
use stayke_config::{assert_cpi_authority, AllowedCaller, GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{error::PactumCoreError, state::Judge};

#[derive(Accounts)]
pub struct CpiRecordJudgeVote<'info> {
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

pub fn handler_cpi_record_judge_vote(
    ctx: Context<CpiRecordJudgeVote>,
    in_consensus: bool,
) -> Result<()> {
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Disputes],
    )?;

    let judge = &mut ctx.accounts.judge;
    judge.cases_voted = judge
        .cases_voted
        .checked_add(1)
        .ok_or(PactumCoreError::MathOverflow)?;
    if in_consensus {
        judge.cases_in_consensus = judge
            .cases_in_consensus
            .checked_add(1)
            .ok_or(PactumCoreError::MathOverflow)?;
    }
    Ok(())
}
