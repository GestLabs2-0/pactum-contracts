use anchor_lang::prelude::*;

use crate::{
    constants::{GLOBAL_CONFIG_SEED, MAX_PROTOCOL_FEE_BPS},
    error::PactumConfigError,
    events::ConfigUpdated,
    state::GlobalConfig,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct UpdateConfigParams {
    pub protocol_fee_bps: Option<u16>,
    pub max_protocol_fee: Option<u64>,
    pub dispute_escalation_fee: Option<u64>,
    pub sas_program: Option<Pubkey>,
    pub sas_credential: Option<Pubkey>,
}

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        constraint = global_config.is_initialized @ PactumConfigError::InvalidGlobalConfig,
        constraint = global_config.authority == authority.key() @ PactumConfigError::Unauthorized,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub authority: Signer<'info>,
}

pub fn handler_update_config(ctx: Context<UpdateConfig>, params: UpdateConfigParams) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config;

    if let Some(protocol_fee_bps) = params.protocol_fee_bps {
        require!(
            protocol_fee_bps < MAX_PROTOCOL_FEE_BPS,
            PactumConfigError::InvalidFeeBps
        );
        global_config.protocol_fee_bps = protocol_fee_bps;
    }
    if let Some(max_protocol_fee) = params.max_protocol_fee {
        global_config.max_protocol_fee = max_protocol_fee;
    }
    if let Some(dispute_escalation_fee) = params.dispute_escalation_fee {
        global_config.dispute_escalation_fee = dispute_escalation_fee;
    }
    if let Some(sas_program) = params.sas_program {
        global_config.sas_program = sas_program;
    }
    if let Some(sas_credential) = params.sas_credential {
        global_config.sas_credential = sas_credential;
    }

    emit!(ConfigUpdated {
        authority: global_config.authority,
        protocol_fee_bps: global_config.protocol_fee_bps,
        max_protocol_fee: global_config.max_protocol_fee,
        dispute_escalation_fee: global_config.dispute_escalation_fee,
    });
    Ok(())
}
