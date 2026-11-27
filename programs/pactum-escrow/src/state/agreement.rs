use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum AgreementStatus {
    Pending,
    Proposed,
    Funded,
    Submitted,
    Completed,
    Disputed,
    Cancelled,
}

/// Seeds: `[AGREEMENT_SEED.as_bytes(), offer.key().as_ref(), client.key().as_ref(), &nonce.to_le_bytes()]`
/// (`["agreement", offer, client, nonce]`).
#[account]
pub struct Agreement {
    pub nonce: u64,
    pub offer: Pubkey,
    pub client: Pubkey,
    pub provider: Pubkey,
    pub integrator: Option<Pubkey>,
    pub integrator_fee_bps: u16,
    pub arbiter: Option<Pubkey>,
    pub jury_fee: u64,
    pub total_amount: u64,
    pub provider_bond: u64,
    pub client_bond: u64,
    pub released_amount: u64,
    pub milestone_amounts: Vec<u64>,
    pub completed_milestones: u8,
    pub step_duration_secs: i64,
    pub current_deadline_ts: i64,
    pub approval_window_secs: i64,
    pub submitted_at: i64,
    pub completed_at: i64,
    pub deliverable_hash: [u8; 32],
    pub state_hash: [u8; 32],
    pub deal_contract_hash: Option<[u8; 32]>,
    pub status: AgreementStatus,
    pub previous_status: Option<AgreementStatus>,
    pub client_reviewed: bool,
    pub provider_reviewed: bool,
    pub rent_payer: Pubkey,
    pub escrow_vault_bump: u8,
    pub bump: u8,
}

impl Agreement {
    /// Exact Borsh serialized size including the 8-byte Anchor discriminator.
    pub fn space(milestone_count: usize) -> usize {
        8 // discriminator
        + 8 // nonce
        + 32 // offer
        + 32 // client
        + 32 // provider
        + 1 + 32 // integrator: Option<Pubkey>
        + 2 // integrator_fee_bps
        + 1 + 32 // arbiter: Option<Pubkey>
        + 8 // jury_fee
        + 8 // total_amount
        + 8 // provider_bond
        + 8 // client_bond
        + 8 // released_amount
        + 4 + (milestone_count * 8) // milestone_amounts: Vec<u64>
        + 1 // completed_milestones
        + 8 // step_duration_secs
        + 8 // current_deadline_ts
        + 8 // approval_window_secs
        + 8 // submitted_at
        + 8 // completed_at
        + 32 // deliverable_hash
        + 32 // state_hash
        + 1 + 32 // deal_contract_hash: Option<[u8; 32]>
        + 1 // status
        + 1 + 1 // previous_status: Option<AgreementStatus>
        + 1 // client_reviewed
        + 1 // provider_reviewed
        + 32 // rent_payer
        + 1 // escrow_vault_bump
        + 1 // bump
    }
}
