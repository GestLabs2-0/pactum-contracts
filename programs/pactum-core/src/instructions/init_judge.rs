use anchor_lang::prelude::*;
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

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

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: Optional SAS attestation verified when `judge_identity.policy == KycAndMarketplaceProof`.
    pub marketplace_attestation: Option<UncheckedAccount<'info>>,

    /// CHECK: Optional SAS credential verified when `judge_identity.policy == KycAndMarketplaceProof`.
    pub marketplace_credential: Option<UncheckedAccount<'info>>,

    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_init_judge(ctx: Context<InitJudge>) -> Result<()> {
    let marketplace_sas = ctx
        .accounts
        .marketplace_attestation
        .as_ref()
        .zip(ctx.accounts.marketplace_credential.as_ref())
        .map(|(a, c)| (a.as_ref(), c.as_ref()));

    ctx.accounts.marketplace.assert_role_eligible(
        &ctx.accounts.user_main,
        &ctx.accounts.marketplace.judge_identity,
        ctx.accounts.global_config.sas_program,
        marketplace_sas,
    )?;
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
