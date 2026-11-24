use anchor_lang::prelude::*;
use stayke_config::{GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{constants::USER_MAIN_SEED, error::PactumCoreError, state::UserMainAcc};

#[derive(Accounts)]
pub struct LinkSasIdentity<'info> {
    #[account(
        mut,
        seeds = [USER_MAIN_SEED.as_bytes(), authority.key().as_ref()],
        bump = user_main.bump,
        has_one = authority @ PactumCoreError::Unauthorized,
        constraint = !user_main.is_banned @ PactumCoreError::UserGloballyBanned,
    )]
    pub user_main: Account<'info, UserMainAcc>,

    pub authority: Signer<'info>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = stayke_config::ID,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: Verified by checking owner == global_config.sas_program.
    pub sas_attestation: UncheckedAccount<'info>,
}

pub fn handler_link_sas_identity(ctx: Context<LinkSasIdentity>) -> Result<()> {
    let sas_program = ctx.accounts.global_config.sas_program;
    require!(
        sas_program != Pubkey::default() && ctx.accounts.sas_attestation.owner == &sas_program,
        PactumCoreError::InvalidSasProgram
    );

    ctx.accounts.user_main.sas_attestation = Some(ctx.accounts.sas_attestation.key());
    Ok(())
}
