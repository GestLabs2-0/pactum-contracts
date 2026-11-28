use anchor_lang::prelude::*;

#[event]
pub struct EscrowInitialized {
    pub authority: Pubkey,
    pub global_config: Pubkey,
}

#[event]
pub struct AgreementCreated {
    pub agreement: Pubkey,
    pub offer: Pubkey,
    pub client: Pubkey,
    pub provider: Pubkey,
    pub total_amount: u64,
}

#[event]
pub struct AgreementProposed {
    pub agreement: Pubkey,
    pub offer: Pubkey,
    pub client: Pubkey,
    pub provider: Pubkey,
    pub total_amount: u64,
}

#[event]
pub struct AgreementFunded {
    pub agreement: Pubkey,
    pub deadline_ts: i64,
}

#[event]
pub struct MilestoneSubmitted {
    pub agreement: Pubkey,
    pub milestone_index: u8,
    pub deliverable_hash: [u8; 32],
    pub submitted_at: i64,
}

#[event]
pub struct MilestoneReleased {
    pub agreement: Pubkey,
    pub milestone_index: u8,
    pub amount: u64,
    pub protocol_fee: u64,
    pub integrator_fee: u64,
    pub provider_payout: u64,
}

#[event]
pub struct AgreementCompleted {
    pub agreement: Pubkey,
    pub completed_at: i64,
}

#[event]
pub struct AgreementCancelled {
    pub agreement: Pubkey,
    pub reason: String,
}

#[event]
pub struct AgreementClosed {
    pub agreement: Pubkey,
    pub rent_payer: Pubkey,
}

#[event]
pub struct ReservationCreated {
    pub reservation: Pubkey,
    pub offer: Pubkey,
    pub client: Pubkey,
    pub provider: Pubkey,
    pub check_in: i64,
    pub check_out: i64,
    pub total_amount: u64,
}

#[event]
pub struct ReservationAccepted {
    pub reservation: Pubkey,
    pub updated_at: i64,
}

#[event]
pub struct ReservationStarted {
    pub reservation: Pubkey,
    pub updated_at: i64,
}

#[event]
pub struct ReservationCompleted {
    pub reservation: Pubkey,
    pub updated_at: i64,
}

#[event]
pub struct ReservationReleased {
    pub reservation: Pubkey,
    pub total_amount: u64,
    pub protocol_fee: u64,
    pub integrator_fee: u64,
    pub provider_payout: u64,
}

#[event]
pub struct ReservationCancelled {
    pub reservation: Pubkey,
    pub client_refund: u64,
    pub provider_payout: u64,
    pub protocol_fee: u64,
    pub integrator_fee: u64,
}

#[event]
pub struct ReservationClosed {
    pub reservation: Pubkey,
    pub rent_payer: Pubkey,
}

#[event]
pub struct CounterpartyReviewed {
    pub target_agreement: Pubkey,
    pub reviewer: Pubkey,
    pub rating: u8,
}

#[event]
pub struct DisputeFrozen {
    pub target_agreement: Pubkey,
}

#[event]
pub struct DisputeSettled {
    pub target_agreement: Pubkey,
    pub client_payout: u64,
    pub provider_payout: u64,
    pub fee_payout: u64,
}
