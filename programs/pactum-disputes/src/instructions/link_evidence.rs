use anchor_lang::prelude::*;
use pactum_core::state::{Client, DisputeParty, Provider};

use crate::{
    constants::DISPUTE_SEED,
    error::PactumDisputesError,
    events::EvidenceLinked,
    state::{Dispute, DisputeStatus},
};

#[derive(Accounts)]
pub struct LinkEvidence<'info> {
    #[account(
        mut,
        seeds = [DISPUTE_SEED.as_bytes(), dispute.agreement.as_ref()],
        bump = dispute.bump,
        constraint = dispute.status != DisputeStatus::Resolved @ PactumDisputesError::InvalidDisputeStatus,
    )]
    pub dispute: Account<'info, Dispute>,

    #[account(
        constraint = client.key() == dispute.client @ PactumDisputesError::InvalidPartyAccount,
    )]
    pub client: Account<'info, Client>,

    #[account(
        constraint = provider.key() == dispute.provider @ PactumDisputesError::InvalidPartyAccount,
    )]
    pub provider: Account<'info, Provider>,

    pub signer: Signer<'info>,
}

pub fn handler_link_evidence(ctx: Context<LinkEvidence>, evidence_hash: [u8; 32]) -> Result<()> {
    let dispute = &mut ctx.accounts.dispute;
    let signer_key = ctx.accounts.signer.key();

    let party = if signer_key == ctx.accounts.client.authority {
        require!(
            dispute.client_evidence.is_none(),
            PactumDisputesError::EvidenceAlreadySubmitted
        );
        dispute.client_evidence = Some(evidence_hash);
        DisputeParty::Client
    } else if signer_key == ctx.accounts.provider.authority {
        require!(
            dispute.provider_evidence.is_none(),
            PactumDisputesError::EvidenceAlreadySubmitted
        );
        dispute.provider_evidence = Some(evidence_hash);
        DisputeParty::Provider
    } else {
        return err!(PactumDisputesError::Unauthorized);
    };

    emit!(EvidenceLinked {
        dispute: dispute.key(),
        party,
        evidence_hash,
    });
    Ok(())
}
