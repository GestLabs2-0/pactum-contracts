use anchor_lang::prelude::*;
use stayke_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    constants::ARBITER_APP_SEED,
    error::PactumCoreError,
    state::{ArbiterApplication, Marketplace},
};

#[derive(Accounts)]
pub struct ApplyAsArbiter<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + ArbiterApplication::INIT_SPACE,
        seeds = [ARBITER_APP_SEED.as_bytes(), marketplace.key().as_ref(), authority.key().as_ref()],
        bump,
    )]
    pub arbiter_application: Account<'info, ArbiterApplication>,

    #[account(constraint = marketplace.is_active @ PactumCoreError::MarketplaceInactive)]
    pub marketplace: Account<'info, Marketplace>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: Verified by checking owner == global_config.sas_program.
    pub sas_kyb_attestation: UncheckedAccount<'info>,

    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_apply_as_arbiter(
    ctx: Context<ApplyAsArbiter>,
    arbiter_fee: u64,
    sla_secs: i64,
) -> Result<()> {
    require!(sla_secs > 0, PactumCoreError::InvalidArbiterParams);

    let sas_program = ctx.accounts.global_config.sas_program;
    require!(
        sas_program != Pubkey::default() && ctx.accounts.sas_kyb_attestation.owner == &sas_program,
        PactumCoreError::InvalidSasProgram
    );

    let app = &mut ctx.accounts.arbiter_application;
    app.marketplace = ctx.accounts.marketplace.key();
    app.authority = ctx.accounts.authority.key();
    app.sas_kyb_attestation = ctx.accounts.sas_kyb_attestation.key();
    app.arbiter_fee = arbiter_fee;
    app.sla_secs = sla_secs;
    app.rent_payer = ctx.accounts.payer.key();
    app.bump = ctx.bumps.arbiter_application;
    Ok(())
}
