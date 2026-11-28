use anchor_lang::prelude::*;
use pactum_core::state::DisputeParty;

use crate::constants::JURY_PANEL_SIZE;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum DisputeMode {
    Arbiter,
    DecentralizedJury,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum DisputeStatus {
    OpenP2P,
    PendingArbiter,
    VrfPending,
    Voting,
    Resolved,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum VoteChoice {
    ClientWins,
    ProviderWins,
    NoFault,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum DisputeOutcome {
    ClientWins,
    ProviderWins,
    NoFault,
    P2PAgreement,
}

/// Seeds: `[DISPUTE_SEED.as_bytes(), agreement.as_ref()]` (`["dispute", agreement]`).
#[account]
#[derive(InitSpace)]
pub struct Dispute {
    pub agreement: Pubkey,
    pub marketplace: Pubkey,
    pub client: Pubkey,
    pub provider: Pubkey,
    pub opened_by: DisputeParty,
    pub mode: DisputeMode,
    pub assigned_arbiter: Option<Pubkey>,
    pub proposed_replacement_arbiter: Option<Pubkey>,
    pub proposed_by: Option<DisputeParty>,
    pub selected_leaves: [u32; JURY_PANEL_SIZE],
    pub panel_size: u8,
    pub round: u8,
    pub runoff_choices: Option<[VoteChoice; 2]>,
    pub client_evidence: Option<[u8; 32]>,
    pub provider_evidence: Option<[u8; 32]>,
    pub status: DisputeStatus,
    pub outcome: Option<DisputeOutcome>,
    pub is_reservation: bool,
    pub created_at: i64,
    pub phase_started_at: i64,
    pub sla_deadline: i64,
    pub rent_payer: Pubkey,
    pub bump: u8,
}
