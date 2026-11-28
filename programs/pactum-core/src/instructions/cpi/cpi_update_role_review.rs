use anchor_lang::prelude::*;
use pactum_config::{assert_cpi_authority, AllowedCaller, GlobalConfig, GLOBAL_CONFIG_SEED};

use crate::{
    error::PactumCoreError,
    events::RoleReviewRecorded,
    state::{Client, ParticipantRole, Provider},
};

#[derive(Accounts)]
pub struct CpiUpdateRoleReview<'info> {
    #[account(mut)]
    pub client: Option<Account<'info, Client>>,

    #[account(mut)]
    pub provider: Option<Account<'info, Provider>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub cpi_authority: Signer<'info>,
}

pub fn handler_cpi_update_role_review(
    ctx: Context<CpiUpdateRoleReview>,
    role: ParticipantRole,
    score: u8,
) -> Result<()> {
    assert_cpi_authority(
        &ctx.accounts.global_config,
        &ctx.accounts.cpi_authority.key(),
        &[AllowedCaller::Escrow],
    )?;
    require!((1..=5).contains(&score), PactumCoreError::InvalidScore);

    let (marketplace, authority, total_reviews) = match role {
        ParticipantRole::Client => {
            let client = ctx
                .accounts
                .client
                .as_mut()
                .ok_or(PactumCoreError::MissingRoleAccount)?;
            client.total_reviews = client
                .total_reviews
                .checked_add(1)
                .ok_or(PactumCoreError::MathOverflow)?;
            client.total_score = client
                .total_score
                .checked_add(score as u64)
                .ok_or(PactumCoreError::MathOverflow)?;
            (client.marketplace, client.authority, client.total_reviews)
        }
        ParticipantRole::Provider => {
            let provider = ctx
                .accounts
                .provider
                .as_mut()
                .ok_or(PactumCoreError::MissingRoleAccount)?;
            provider.total_reviews = provider
                .total_reviews
                .checked_add(1)
                .ok_or(PactumCoreError::MathOverflow)?;
            provider.total_score = provider
                .total_score
                .checked_add(score as u64)
                .ok_or(PactumCoreError::MathOverflow)?;
            (
                provider.marketplace,
                provider.authority,
                provider.total_reviews,
            )
        }
        ParticipantRole::Judge | ParticipantRole::Arbiter | ParticipantRole::Integrator => {
            return err!(PactumCoreError::InvalidRole);
        }
    };

    emit!(RoleReviewRecorded {
        marketplace,
        authority,
        role,
        score,
        total_reviews,
    });
    Ok(())
}
