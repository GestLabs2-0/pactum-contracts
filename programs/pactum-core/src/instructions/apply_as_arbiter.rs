use anchor_lang::prelude::*;
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    constants::ARBITER_APP_SEED,
    error::PactumCoreError,
    state::{verify_and_anchor_identity, ArbiterApplication, Marketplace},
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

    /// CHECK: Verified and created in `verify_and_anchor_identity` from `sha256(attestation.data)`.
    #[account(mut)]
    pub identity: UncheckedAccount<'info>,

    #[account(constraint = marketplace.is_active @ PactumCoreError::MarketplaceInactive)]
    pub marketplace: Account<'info, Marketplace>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: Verified by `verify_and_anchor_identity` against `global_config.sas_program`.
    pub sas_kyb_attestation: UncheckedAccount<'info>,

    /// CHECK: Verified by `verify_and_anchor_identity` against `marketplace.kyb_credential` (or `global_config.sas_credential`).
    pub sas_credential: UncheckedAccount<'info>,

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

    let expected_credential = ctx
        .accounts
        .marketplace
        .kyb_credential
        .unwrap_or(ctx.accounts.global_config.sas_credential);

    verify_and_anchor_identity(
        &ctx.accounts.sas_kyb_attestation.to_account_info(),
        &ctx.accounts.sas_credential.to_account_info(),
        &ctx.accounts.identity.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.accounts.global_config.sas_program,
        expected_credential,
        ctx.accounts.authority.key(),
        true,
    )?;

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
