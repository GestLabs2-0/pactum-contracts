use anchor_lang::{prelude::*, solana_program::clock::Clock};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    constants::USER_MAIN_SEED, error::PactumCoreError, state::UserMainAcc, Attestation,
    ATTESTATION_DISCRIMINATOR,
};

#[derive(Accounts)]
pub struct UnlinkSasIdentity<'info> {
    #[account(
        mut,
        seeds = [USER_MAIN_SEED.as_bytes(), authority.key().as_ref()],
        bump = user_main.bump,
        has_one = authority @ PactumCoreError::Unauthorized,
    )]
    pub user_main: Account<'info, UserMainAcc>,

    pub authority: Signer<'info>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: Verified against `user_main.sas_attestation`; must be closed or expired.
    #[account(
        constraint = user_main.sas_attestation == Some(sas_attestation.key()) @ PactumCoreError::NoLinkedAttestation,
    )]
    pub sas_attestation: UncheckedAccount<'info>,
}

pub fn handler_unlink_sas_identity(ctx: Context<UnlinkSasIdentity>) -> Result<()> {
    let sas_program = ctx.accounts.global_config.sas_program;
    let attestation_info = &ctx.accounts.sas_attestation;

    let is_closed = attestation_info.data_is_empty() || attestation_info.owner != &sas_program;
    if !is_closed {
        let raw = attestation_info.try_borrow_data()?;
        require!(
            raw.len() > 1 && raw[0] == ATTESTATION_DISCRIMINATOR,
            PactumCoreError::InvalidAttestationAcc
        );
        let attestation = Attestation::deserialize(&mut &raw.as_ref()[1..])?;
        let now = Clock::get()?.unix_timestamp;
        require!(
            attestation.expiry != 0 && attestation.expiry < now,
            PactumCoreError::AttestationStillActive
        );
    }

    ctx.accounts.user_main.sas_attestation = None;
    Ok(())
}
