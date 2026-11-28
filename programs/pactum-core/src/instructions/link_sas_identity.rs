use anchor_lang::prelude::*;
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    constants::USER_MAIN_SEED,
    error::PactumCoreError,
    state::{verify_and_anchor_identity, UserMainAcc},
};

#[derive(Accounts)]
pub struct LinkSasIdentity<'info> {
    #[account(
        mut,
        seeds = [USER_MAIN_SEED.as_bytes(), authority.key().as_ref()],
        bump = user_main.bump,
        has_one = authority @ PactumCoreError::Unauthorized,
    )]
    pub user_main: Account<'info, UserMainAcc>,

    /// CHECK: Verified and created in `verify_and_anchor_identity` from `sha256(attestation.data)`.
    #[account(mut)]
    pub identity: UncheckedAccount<'info>,

    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: Verified by `verify_and_anchor_identity` against `global_config.sas_program`.
    pub sas_attestation: UncheckedAccount<'info>,

    /// CHECK: Verified by `verify_and_anchor_identity` against `global_config.sas_credential`.
    pub sas_credential: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_link_sas_identity(ctx: Context<LinkSasIdentity>) -> Result<()> {
    verify_and_anchor_identity(
        &ctx.accounts.sas_attestation.to_account_info(),
        &ctx.accounts.sas_credential.to_account_info(),
        &ctx.accounts.identity.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.accounts.global_config.sas_program,
        ctx.accounts.global_config.sas_credential,
        ctx.accounts.authority.key(),
        false,
    )?;

    ctx.accounts.user_main.sas_attestation = Some(ctx.accounts.sas_attestation.key());
    Ok(())
}
