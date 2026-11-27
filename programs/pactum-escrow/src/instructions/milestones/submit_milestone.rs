use anchor_lang::prelude::*;
use pactum_core::{constants::PROVIDER_SEED, state::Provider};

use crate::{
    constants::AGREEMENT_SEED,
    error::PactumEscrowError,
    events::MilestoneSubmitted,
    state::{Agreement, AgreementStatus},
};

#[derive(Accounts)]
pub struct SubmitMilestone<'info> {
    pub provider_authority: Signer<'info>,

    #[account(
        seeds = [
            PROVIDER_SEED.as_bytes(),
            provider.marketplace.as_ref(),
            provider_authority.key().as_ref(),
        ],
        bump = provider.bump,
        seeds::program = pactum_core::ID,
        constraint = provider.authority == provider_authority.key() @ PactumEscrowError::Unauthorized,
        constraint = !provider.is_banned @ PactumEscrowError::ParticipantBanned,
    )]
    pub provider: Box<Account<'info, Provider>>,

    #[account(
        mut,
        seeds = [
            AGREEMENT_SEED.as_bytes(),
            agreement.offer.as_ref(),
            agreement.client.as_ref(),
            agreement.nonce.to_le_bytes().as_ref(),
        ],
        bump = agreement.bump,
        has_one = provider @ PactumEscrowError::Unauthorized,
        constraint = agreement.status == AgreementStatus::Funded @ PactumEscrowError::InvalidAgreementStatus,
    )]
    pub agreement: Box<Account<'info, Agreement>>,
}

pub fn handler_submit_milestone(
    ctx: Context<SubmitMilestone>,
    deliverable_hash: [u8; 32],
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let agreement = &mut ctx.accounts.agreement;
    require!(
        now <= agreement.current_deadline_ts,
        PactumEscrowError::DeadlinePassed
    );

    agreement.deliverable_hash = deliverable_hash;
    agreement.submitted_at = now;
    agreement.status = AgreementStatus::Submitted;

    emit!(MilestoneSubmitted {
        agreement: agreement.key(),
        milestone_index: agreement.completed_milestones,
        deliverable_hash,
        submitted_at: now,
    });
    Ok(())
}
