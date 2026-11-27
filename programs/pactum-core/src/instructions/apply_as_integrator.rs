use anchor_lang::{
    prelude::*,
    system_program::{create_account, CreateAccount},
};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use solana_sha256_hasher::hash;

use crate::{
    constants::{IDENTITY_SEED, INTEGRATOR_APP_SEED},
    error::PactumCoreError,
    state::{
        verify_sas_attestation, Identity, IntegratorApplication, IntegratorPolicy, Marketplace,
    },
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
    id_hash: Option<[u8; 32]>,
) -> Result<()> {
    require!(
        default_fee_bps <= 10_000,
        PactumCoreError::InvalidIntegratorParams
    );

    let sas_kyb_attestation = if ctx.accounts.marketplace.integrator_policy
        == IntegratorPolicy::KybAndCommittee
    {
        let id_hash = id_hash.ok_or(PactumCoreError::InvalidIntegratorParams)?;
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

        let (attestation, now) = verify_sas_attestation(
            attestation_acc,
            credential_acc,
            ctx.accounts.global_config.sas_program,
            expected_credential,
            ctx.accounts.authority.key(),
        )?;

        require!(
            !attestation.data.is_empty() && hash(&attestation.data).to_bytes() == id_hash,
            PactumCoreError::InvalidAttestationData
        );

        let (expected_identity, identity_bump) =
            Pubkey::find_program_address(&[IDENTITY_SEED.as_bytes(), id_hash.as_ref()], &crate::ID);
        require_keys_eq!(
            identity_acc.key(),
            expected_identity,
            PactumCoreError::InvalidIntegratorParams
        );

        if identity_acc.data_is_empty() {
            let space = 8 + Identity::INIT_SPACE;
            let lamports = Rent::get()?.minimum_balance(space);
            let signer_seeds: &[&[&[u8]]] =
                &[&[IDENTITY_SEED.as_bytes(), id_hash.as_ref(), &[identity_bump]]];
            let cpi_ctx = CpiContext::new_with_signer(
                ctx.accounts.system_program.key(),
                CreateAccount {
                    from: ctx.accounts.payer.to_account_info(),
                    to: identity_acc.to_account_info(),
                },
                signer_seeds,
            );
            create_account(cpi_ctx, lamports, space as u64, &crate::ID)?;

            let identity = Identity {
                authority: ctx.accounts.authority.key(),
                verified_at: now,
                linked: true,
                bump: identity_bump,
            };
            let mut data = identity_acc.try_borrow_mut_data()?;
            let mut writer: &mut [u8] = &mut data;
            identity.try_serialize(&mut writer)?;
        } else {
            let data = identity_acc.try_borrow_data()?;
            let identity = Identity::try_deserialize(&mut &data[..])?;
            require!(
                identity.authority == ctx.accounts.authority.key(),
                PactumCoreError::IdentityAlreadyClaimed
            );
        }

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
