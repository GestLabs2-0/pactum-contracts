use anchor_lang::prelude::*;
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    constants::{DISPUTE_CONFIG_SEED, MAX_BPS},
    error::PactumDisputesError,
    events::DisputeConfigInitialized,
    state::DisputeConfig,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct InitializeDisputeConfigParams {
    pub p2p_window_secs: i64,
    pub vote_window_secs: i64,
    pub reveal_window_secs: i64,
    pub arbiter_sla_slash_bps: u16,
    pub no_fault_fee_bps: u16,
}

#[derive(Accounts)]
pub struct InitializeDisputeConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + DisputeConfig::INIT_SPACE,
        seeds = [DISPUTE_CONFIG_SEED.as_bytes()],
        bump,
    )]
    pub dispute_config: Account<'info, DisputeConfig>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
        constraint = global_config.is_initialized @ PactumDisputesError::ConfigNotInitialized,
        constraint = authority.key() == global_config.authority @ PactumDisputesError::Unauthorized,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_dispute_config(
    ctx: Context<InitializeDisputeConfig>,
    params: InitializeDisputeConfigParams,
) -> Result<()> {
    require!(
        params.p2p_window_secs > 0
            && params.vote_window_secs > 0
            && params.reveal_window_secs > 0
            && params.arbiter_sla_slash_bps <= MAX_BPS
            && params.no_fault_fee_bps <= MAX_BPS,
        PactumDisputesError::InvalidConfig
    );

    let cfg = &mut ctx.accounts.dispute_config;
    cfg.authority = ctx.accounts.authority.key();
    cfg.global_config = ctx.accounts.global_config.key();
    cfg.p2p_window_secs = params.p2p_window_secs;
    cfg.vote_window_secs = params.vote_window_secs;
    cfg.reveal_window_secs = params.reveal_window_secs;
    cfg.arbiter_sla_slash_bps = params.arbiter_sla_slash_bps;
    cfg.no_fault_fee_bps = params.no_fault_fee_bps;
    cfg.is_initialized = true;
    cfg.bump = ctx.bumps.dispute_config;

    emit!(DisputeConfigInitialized {
        authority: cfg.authority,
        global_config: cfg.global_config,
        p2p_window_secs: cfg.p2p_window_secs,
        vote_window_secs: cfg.vote_window_secs,
        reveal_window_secs: cfg.reveal_window_secs,
        arbiter_sla_slash_bps: cfg.arbiter_sla_slash_bps,
        no_fault_fee_bps: cfg.no_fault_fee_bps,
    });
    Ok(())
}
