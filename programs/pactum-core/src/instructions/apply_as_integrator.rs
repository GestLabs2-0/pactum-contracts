use anchor_lang::prelude::*;
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    constants::INTEGRATOR_APP_SEED,
    error::PactumCoreError,
    state::{verify_and_anchor_identity, IntegratorApplication, IntegratorPolicy, Marketplace},
};

#[derive(Accounts)]
pub struct ApplyAsIntegrator<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + IntegratorApplication::INIT_SPACE,
        seeds = [
            INTEGRATOR_APP_SEED.as_bytes(),
            marketplace.key().as_ref(),
            authority.key().as_ref(),
        ],
        bump,
    )]
    pub integrator_application: Account<'info, IntegratorApplication>,

    #[account(
        constraint = marketplace.is_active @ PactumCoreError::MarketplaceInactive,
        constraint = marketplace.integrator_policy != IntegratorPolicy::Permissionless @ PactumCoreError::InvalidIntegratorPolicy,
    )]
    pub marketplace: Account<'info, Marketplace>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: Optional anti-Sybil Identity PDA (`["identity", id_hash]`), required when `KybAndCommittee`.
    #[account(mut)]
    pub identity: Option<UncheckedAccount<'info>>,

    /// CHECK: Optional SAS KYB attestation, verified when `KybAndCommittee`.
    pub sas_kyb_attestation: Option<UncheckedAccount<'info>>,

    /// CHECK: Optional SAS credential, verified when `KybAndCommittee`.
    pub sas_credential: Option<UncheckedAccount<'info>>,

    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_apply_as_integrator(
    ctx: Context<ApplyAsIntegrator>,
    default_fee_bps: u16,
) -> Result<()> {
    require!(
        default_fee_bps <= 10_000,
        PactumCoreError::InvalidIntegratorParams
    );

    let sas_kyb_attestation =
        if ctx.accounts.marketplace.integrator_policy == IntegratorPolicy::KybAndCommittee {
            let attestation_acc = ctx
                .accounts
                .sas_kyb_attestation
                .as_ref()
                .ok_or(PactumCoreError::InvalidAttestationAcc)?;
            let credential_acc = ctx
                .accounts
                .sas_credential
                .as_ref()
                .ok_or(PactumCoreError::InvalidCredentialAcc)?;
            let identity_acc = ctx
                .accounts
                .identity
                .as_ref()
                .ok_or(PactumCoreError::InvalidIntegratorParams)?;

            let expected_credential = ctx
                .accounts
                .marketplace
                .kyb_credential
                .unwrap_or(ctx.accounts.global_config.sas_credential);

            verify_and_anchor_identity(
                &attestation_acc.to_account_info(),
                &credential_acc.to_account_info(),
                &identity_acc.to_account_info(),
                &ctx.accounts.payer.to_account_info(),
                &ctx.accounts.system_program.to_account_info(),
                ctx.accounts.global_config.sas_program,
                expected_credential,
                ctx.accounts.authority.key(),
                true,
            )?;

            Some(attestation_acc.key())
        } else {
            None
        };

    let app = &mut ctx.accounts.integrator_application;
    app.marketplace = ctx.accounts.marketplace.key();
    app.authority = ctx.accounts.authority.key();
    app.sas_kyb_attestation = sas_kyb_attestation;
    app.default_fee_bps = default_fee_bps;
    app.rent_payer = ctx.accounts.payer.key();
    app.bump = ctx.bumps.integrator_application;
    Ok(())
}
