use anchor_lang::prelude::*;

use crate::{
    constants::{CLIENT_SEED, INTEGRATOR_SEED, JUDGE_SEED, MARKETPLACE_SEED, PROVIDER_SEED},
    error::PactumCoreError,
    events::LocalBanUpdated,
    state::{Client, IntegratorProfile, Judge, Marketplace, ParticipantRole, Provider},
};

#[derive(Accounts)]
pub struct SetLocalBan<'info> {
    #[account(
        seeds = [MARKETPLACE_SEED.as_bytes(), marketplace.market_id.to_le_bytes().as_ref()],
        bump = marketplace.bump,
        has_one = committee @ PactumCoreError::Unauthorized,
    )]
    pub marketplace: Account<'info, Marketplace>,

    pub committee: Signer<'info>,

    /// CHECK: Target participant authority wallet.
    pub target: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [CLIENT_SEED.as_bytes(), marketplace.key().as_ref(), target.key().as_ref()],
        bump = client.bump,
    )]
    pub client: Option<Account<'info, Client>>,

    #[account(
        mut,
        seeds = [PROVIDER_SEED.as_bytes(), marketplace.key().as_ref(), target.key().as_ref()],
        bump = provider.bump,
    )]
    pub provider: Option<Account<'info, Provider>>,

    #[account(
        mut,
        seeds = [JUDGE_SEED.as_bytes(), marketplace.key().as_ref(), target.key().as_ref()],
        bump = judge.bump,
    )]
    pub judge: Option<Account<'info, Judge>>,

    #[account(
        mut,
        seeds = [INTEGRATOR_SEED.as_bytes(), marketplace.key().as_ref(), target.key().as_ref()],
        bump = integrator.bump,
    )]
    pub integrator: Option<Account<'info, IntegratorProfile>>,
}

pub fn handler_set_local_ban(
    ctx: Context<SetLocalBan>,
    role: ParticipantRole,
    banned: bool,
) -> Result<()> {
    match role {
        ParticipantRole::Client => {
            let client = ctx
                .accounts
                .client
                .as_mut()
                .ok_or(PactumCoreError::MissingRoleAccount)?;
            client.is_banned = banned;
        }
        ParticipantRole::Provider => {
            let provider = ctx
                .accounts
                .provider
                .as_mut()
                .ok_or(PactumCoreError::MissingRoleAccount)?;
            provider.is_banned = banned;
        }
        ParticipantRole::Judge => {
            let judge = ctx
                .accounts
                .judge
                .as_mut()
                .ok_or(PactumCoreError::MissingRoleAccount)?;
            judge.is_banned = banned;
        }
        ParticipantRole::Integrator => {
            let integrator = ctx
                .accounts
                .integrator
                .as_mut()
                .ok_or(PactumCoreError::MissingRoleAccount)?;
            integrator.is_banned = banned;
        }
        ParticipantRole::Arbiter => return err!(PactumCoreError::InvalidRole),
    }

    emit!(LocalBanUpdated {
        marketplace: ctx.accounts.marketplace.key(),
        authority: ctx.accounts.target.key(),
        role,
        is_banned: banned,
    });
    Ok(())
}
