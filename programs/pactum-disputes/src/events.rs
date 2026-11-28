use anchor_lang::prelude::*;
use pactum_core::state::DisputeParty;

use crate::state::{DisputeMode, DisputeOutcome, VoteChoice};

#[event]
pub struct DisputeConfigInitialized {
    pub authority: Pubkey,
    pub global_config: Pubkey,
    pub p2p_window_secs: i64,
    pub vote_window_secs: i64,
    pub reveal_window_secs: i64,
    pub arbiter_sla_slash_bps: u16,
    pub no_fault_fee_bps: u16,
}

#[event]
pub struct DisputeConfigUpdated {
    pub authority: Pubkey,
    pub p2p_window_secs: i64,
    pub vote_window_secs: i64,
    pub reveal_window_secs: i64,
    pub arbiter_sla_slash_bps: u16,
    pub no_fault_fee_bps: u16,
}

#[event]
pub struct DisputeOpened {
    pub dispute: Pubkey,
    pub agreement: Pubkey,
    pub marketplace: Pubkey,
    pub opened_by: DisputeParty,
    pub mode: DisputeMode,
}

#[event]
pub struct EvidenceLinked {
    pub dispute: Pubkey,
    pub party: DisputeParty,
    pub evidence_hash: [u8; 32],
}

#[event]
pub struct P2pAgreementSolved {
    pub dispute: Pubkey,
    pub agreement: Pubkey,
    pub restore_previous_status: bool,
    pub client_payout: u64,
    pub provider_payout: u64,
}

#[event]
pub struct DisputeEscalated {
    pub dispute: Pubkey,
    pub agreement: Pubkey,
    pub mode: DisputeMode,
    pub sla_deadline: i64,
}

#[event]
pub struct DisputeResolvedByArbiter {
    pub dispute: Pubkey,
    pub agreement: Pubkey,
    pub arbiter: Pubkey,
    pub outcome: DisputeOutcome,
    pub fee_payout: u64,
}

#[event]
pub struct ArbiterReplacementProposed {
    pub dispute: Pubkey,
    pub proposed_by: DisputeParty,
    pub new_arbiter: Pubkey,
}

#[event]
pub struct ArbiterReplacementAccepted {
    pub dispute: Pubkey,
    pub old_arbiter: Pubkey,
    pub new_arbiter: Pubkey,
    pub slash_amount: u64,
}

#[event]
pub struct DisputeFallbackToIntegrator {
    pub dispute: Pubkey,
    pub old_arbiter: Pubkey,
    pub integrator: Option<Pubkey>,
    pub slash_amount: u64,
    pub fell_back_to_jury: bool,
}

#[event]
pub struct JuryRunoffTriggered {
    pub dispute: Pubkey,
    pub round: u8,
    pub choices: [VoteChoice; 2],
}

#[event]
pub struct JuryTallied {
    pub dispute: Pubkey,
    pub round: u8,
    pub outcome: DisputeOutcome,
    pub fee_payout: u64,
}

#[event]
pub struct DisputeClosed {
    pub dispute: Pubkey,
    pub agreement: Pubkey,
    pub rent_payer: Pubkey,
}
