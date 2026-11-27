use anchor_lang::prelude::*;
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use solana_sha256_hasher::hash;

use crate::{
    constants::{IDENTITY_SEED, USER_MAIN_SEED},
    error::PactumCoreError,
    state::{verify_sas_attestation, Identity, UserMainAcc},
};

#[derive(Accounts)]
#[instruction(id_hash: [u8; 32])]
pub struct LinkSasIdentity<'info> {
    #[account(
        mut,
        seeds = [USER_MAIN_SEED.as_bytes(), authority.key().as_ref()],
        bump = user_main.bump,
        has_one = authority @ PactumCoreError::Unauthorized,
        constraint = !user_main.is_banned @ PactumCoreError::UserGloballyBanned,
    )]
    pub user_main: Account<'info, UserMainAcc>,

    #[account(
        init,
        payer = payer,
        space = 8 + Identity::INIT_SPACE,
        seeds = [IDENTITY_SEED.as_bytes(), id_hash.as_ref()],
        bump,
    )]
    pub identity: Account<'info, Identity>,

    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: Verified by `verify_sas_attestation` against `global_config.sas_program`.
    pub sas_attestation: UncheckedAccount<'info>,

    /// CHECK: Verified by `verify_sas_attestation` against `global_config.sas_credential`.
    pub sas_credential: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_link_sas_identity(ctx: Context<LinkSasIdentity>, id_hash: [u8; 32]) -> Result<()> {
    let (attestation, now) = verify_sas_attestation(
        &ctx.accounts.sas_attestation,
        &ctx.accounts.sas_credential,
        ctx.accounts.global_config.sas_program,
        ctx.accounts.global_config.sas_credential,
        ctx.accounts.authority.key(),
    )?;

    require!(
        !attestation.data.is_empty() && hash(&attestation.data).to_bytes() == id_hash,
        PactumCoreError::InvalidAttestationData
    );

    let identity = &mut ctx.accounts.identity;
    identity.authority = ctx.accounts.authority.key();
    identity.verified_at = now;
    identity.linked = true;
    identity.bump = ctx.bumps.identity;

    ctx.accounts.user_main.sas_attestation = Some(ctx.accounts.sas_attestation.key());
    Ok(())
}
