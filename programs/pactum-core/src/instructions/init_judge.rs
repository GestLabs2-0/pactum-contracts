use anchor_lang::prelude::*;

use crate::{
    constants::{JUDGE_SEED, USER_MAIN_SEED},
    error::PactumCoreError,
    state::{DisputeMode, Judge, Marketplace, UserMainAcc},
};

#[derive(Accounts)]
pub struct InitJudge<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + Judge::INIT_SPACE,
        seeds = [JUDGE_SEED.as_bytes(), marketplace.key().as_ref(), user_main.key().as_ref()],
        bump,
    )]
    pub judge: Account<'info, Judge>,

    pub marketplace: Account<'info, Marketplace>,

    #[account(
        seeds = [USER_MAIN_SEED.as_bytes(), authority.key().as_ref()],
        bump = user_main.bump,
        has_one = authority @ PactumCoreError::Unauthorized,
    )]
    pub user_main: Account<'info, UserMainAcc>,

    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_init_judge(ctx: Context<InitJudge>) -> Result<()> {
    ctx.accounts
        .marketplace
        .assert_participant_eligible(&ctx.accounts.user_main)?;
    require!(
        ctx.accounts.marketplace.dispute_mode != DisputeMode::ArbiterOnly,
        PactumCoreError::JuryNotEnabled
    );

    let judge = &mut ctx.accounts.judge;
    judge.marketplace = ctx.accounts.marketplace.key();
    judge.user_main = ctx.accounts.user_main.key();
    judge.staked_amount = 0;
    judge.pool_id = 0;
    judge.leaf_index = 0;
    judge.is_active_in_pool = false;
    judge.active_disputes = 0;
    judge.cases_voted = 0;
    judge.cases_in_consensus = 0;
    judge.is_banned = false;
    judge.bump = ctx.bumps.judge;
    Ok(())
}
