use anchor_lang::prelude::*;
use pactum_core::{
    constants::{ARBITER_SEED, CLIENT_SEED, PROVIDER_SEED},
    state::{ArbiterProfile, Client, DisputeParty, Provider},
};

use crate::{
    constants::DISPUTE_SEED,
    error::PactumDisputesError,
    events::ArbiterReplacementProposed,
    state::{Dispute, DisputeMode, DisputeStatus},
};

#[derive(Accounts)]
pub struct ProposeArbiterReplacement<'info> {
    #[account(
        mut,
        seeds = [DISPUTE_SEED.as_bytes(), dispute.agreement.as_ref()],
        bump = dispute.bump,
        constraint = dispute.status == DisputeStatus::PendingArbiter @ PactumDisputesError::InvalidDisputeStatus,
        constraint = dispute.mode == DisputeMode::Arbiter @ PactumDisputesError::InvalidDisputeMode,
    )]
    pub dispute: Box<Account<'info, Dispute>>,

    #[account(
        seeds = [CLIENT_SEED.as_bytes(), dispute.marketplace.as_ref(), client.authority.as_ref()],
        bump = client.bump,
        seeds::program = pactum_core::ID,
        constraint = client.key() == dispute.client @ PactumDisputesError::InvalidPartyAccount,
    )]
    pub client: Box<Account<'info, Client>>,

    #[account(
        seeds = [PROVIDER_SEED.as_bytes(), dispute.marketplace.as_ref(), provider.authority.as_ref()],
        bump = provider.bump,
        seeds::program = pactum_core::ID,
        constraint = provider.key() == dispute.provider @ PactumDisputesError::InvalidPartyAccount,
    )]
    pub provider: Box<Account<'info, Provider>>,

    #[account(
        seeds = [ARBITER_SEED.as_bytes(), dispute.marketplace.as_ref(), current_arbiter.authority.as_ref()],
        bump = current_arbiter.bump,
        seeds::program = pactum_core::ID,
        constraint = dispute.assigned_arbiter == Some(current_arbiter.authority) @ PactumDisputesError::InvalidArbiter,
    )]
    pub current_arbiter: Box<Account<'info, ArbiterProfile>>,

    #[account(
        seeds = [ARBITER_SEED.as_bytes(), dispute.marketplace.as_ref(), new_arbiter.authority.as_ref()],
        bump = new_arbiter.bump,
        seeds::program = pactum_core::ID,
        constraint = new_arbiter.is_active @ PactumDisputesError::ArbiterInactive,
        constraint = new_arbiter.authority != current_arbiter.authority @ PactumDisputesError::InvalidArbiter,
    )]
    pub new_arbiter: Box<Account<'info, ArbiterProfile>>,

    pub signer: Signer<'info>,
}

pub fn handler_propose_arbiter_replacement(ctx: Context<ProposeArbiterReplacement>) -> Result<()> {
    let dispute = &mut ctx.accounts.dispute;
    let signer_key = ctx.accounts.signer.key();

    let proposed_by = if signer_key == ctx.accounts.client.authority {
        DisputeParty::Client
    } else if signer_key == ctx.accounts.provider.authority {
        DisputeParty::Provider
    } else {
        return err!(PactumDisputesError::Unauthorized);
    };

    let now = Clock::get()?.unix_timestamp;
    require!(
        now > dispute.sla_deadline || !ctx.accounts.current_arbiter.is_active,
        PactumDisputesError::SlaNotBreached
    );

    dispute.proposed_replacement_arbiter = Some(ctx.accounts.new_arbiter.authority);
    dispute.proposed_by = Some(proposed_by);

    emit!(ArbiterReplacementProposed {
        dispute: dispute.key(),
        proposed_by,
        new_arbiter: ctx.accounts.new_arbiter.authority,
    });
    Ok(())
}
