use anchor_lang::prelude::*;
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    constants::{DISPUTE_CONFIG_SEED, MAX_BPS},
    error::PactumDisputesError,
    events::DisputeConfigUpdated,
    state::DisputeConfig,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct UpdateDisputeConfigParams {
    pub p2p_window_secs: Option<i64>,
    pub vote_window_secs: Option<i64>,
    pub reveal_window_secs: Option<i64>,
    pub arbiter_sla_slash_bps: Option<u16>,
    pub no_fault_fee_bps: Option<u16>,
}

#[derive(Accounts)]
pub struct UpdateDisputeConfig<'info> {
    #[account(
        mut,
        seeds = [DISPUTE_CONFIG_SEED.as_bytes()],
        bump = dispute_config.bump,
        constraint = dispute_config.is_initialized @ PactumDisputesError::ConfigNotInitialized,
        has_one = authority @ PactumDisputesError::Unauthorized,
    )]
    pub dispute_config: Account<'info, DisputeConfig>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
        constraint = global_config.key() == dispute_config.global_config @ PactumDisputesError::ConfigNotInitialized,
        constraint = authority.key() == global_config.authority @ PactumDisputesError::Unauthorized,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub authority: Signer<'info>,
}

pub fn handler_update_dispute_config(
    ctx: Context<UpdateDisputeConfig>,
    params: UpdateDisputeConfigParams,
) -> Result<()> {
    let cfg = &mut ctx.accounts.dispute_config;

    if let Some(v) = params.p2p_window_secs {
        require!(v > 0, PactumDisputesError::InvalidConfig);
        cfg.p2p_window_secs = v;
    }
    if let Some(v) = params.vote_window_secs {
        require!(v > 0, PactumDisputesError::InvalidConfig);
        cfg.vote_window_secs = v;
    }
    if let Some(v) = params.reveal_window_secs {
        require!(v > 0, PactumDisputesError::InvalidConfig);
        cfg.reveal_window_secs = v;
    }
    if let Some(v) = params.arbiter_sla_slash_bps {
        require!(v <= MAX_BPS, PactumDisputesError::InvalidConfig);
        cfg.arbiter_sla_slash_bps = v;
    }
    if let Some(v) = params.no_fault_fee_bps {
        require!(v <= MAX_BPS, PactumDisputesError::InvalidConfig);
        cfg.no_fault_fee_bps = v;
    }

    emit!(DisputeConfigUpdated {
        authority: cfg.authority,
        p2p_window_secs: cfg.p2p_window_secs,
        vote_window_secs: cfg.vote_window_secs,
        reveal_window_secs: cfg.reveal_window_secs,
        arbiter_sla_slash_bps: cfg.arbiter_sla_slash_bps,
        no_fault_fee_bps: cfg.no_fault_fee_bps,
    });
    Ok(())
}
