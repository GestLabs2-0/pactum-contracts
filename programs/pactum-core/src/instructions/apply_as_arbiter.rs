use anchor_lang::prelude::*;
use solana_sha256_hasher::hash;
use stayke_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    constants::{ARBITER_APP_SEED, IDENTITY_SEED},
    error::PactumCoreError,
    state::{verify_sas_attestation, ArbiterApplication, Identity, Marketplace},
};

#[derive(Accounts)]
#[instruction(arbiter_fee: u64, sla_secs: i64, id_hash: [u8; 32])]
pub struct ApplyAsArbiter<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + ArbiterApplication::INIT_SPACE,
        seeds = [ARBITER_APP_SEED.as_bytes(), marketplace.key().as_ref(), authority.key().as_ref()],
        bump,
    )]
    pub arbiter_application: Account<'info, ArbiterApplication>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + Identity::INIT_SPACE,
        seeds = [IDENTITY_SEED.as_bytes(), id_hash.as_ref()],
        bump,
    )]
    pub identity: Account<'info, Identity>,

    #[account(constraint = marketplace.is_active @ PactumCoreError::MarketplaceInactive)]
    pub marketplace: Account<'info, Marketplace>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: Verified by `verify_sas_attestation` against `global_config.sas_program`.
    pub sas_kyb_attestation: UncheckedAccount<'info>,

    /// CHECK: Verified by `verify_sas_attestation` against `marketplace.kyb_credential` (or `global_config.sas_credential`).
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
    id_hash: [u8; 32],
) -> Result<()> {
    require!(sla_secs > 0, PactumCoreError::InvalidArbiterParams);

    let expected_credential = ctx
        .accounts
        .marketplace
        .kyb_credential
        .unwrap_or(ctx.accounts.global_config.sas_credential);

    let (attestation, now) = verify_sas_attestation(
        &ctx.accounts.sas_kyb_attestation,
        &ctx.accounts.sas_credential,
        ctx.accounts.global_config.sas_program,
        expected_credential,
        ctx.accounts.authority.key(),
    )?;

    require!(
        !attestation.data.is_empty() && hash(&attestation.data).to_bytes() == id_hash,
        PactumCoreError::InvalidAttestationData
    );

    let identity = &mut ctx.accounts.identity;
    if identity.linked {
        require!(
            identity.authority == ctx.accounts.authority.key(),
            PactumCoreError::IdentityAlreadyClaimed
        );
    } else {
        identity.authority = ctx.accounts.authority.key();
        identity.verified_at = now;
        identity.linked = true;
        identity.bump = ctx.bumps.identity;
    }

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
