use anchor_lang::prelude::*;
use pactum_core::state::CancellationPolicy;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum DateRangeStatus {
    Pending,
    Accepted,
    Active,
    Completed,
    Released,
    Disputed,
    Cancelled,
}

/// Seeds: `[DATE_RANGE_AGREEMENT_SEED.as_bytes(), offer.key().as_ref(), client.key().as_ref(), &check_in.to_le_bytes()]`
/// (`["date_range_agreement", offer, client, check_in]`).
#[account]
#[derive(InitSpace)]
pub struct DateRangeAgreement {
    pub marketplace: Pubkey,
    pub offer: Pubkey,
    pub client: Pubkey,
    pub provider: Pubkey,
    pub integrator: Option<Pubkey>,
    pub integrator_fee_bps: u16,
    pub arbiter: Option<Pubkey>,
    pub jury_fee: u64,
    pub check_in: i64,
    pub check_out: i64,
    pub total_amount: u64,
    pub provider_bond: u64,
    pub client_bond: u64,
    pub cancellation_policy: CancellationPolicy,
    pub provider_cancellation_penalty_pct: u8,
    pub content_ref: [u8; 32],
    pub status: DateRangeStatus,
    pub previous_status: Option<DateRangeStatus>,
    pub updated_at: i64,
    pub client_reviewed: bool,
    pub provider_reviewed: bool,
    pub rent_payer: Pubkey,
    pub escrow_vault_bump: u8,
    pub bump: u8,
}

/// Seeds: `[BOOKING_DAYS_SEED.as_bytes(), offer.key().as_ref(), &year.to_le_bytes()]`
/// (`["booking_days", offer, year]`).
#[account]
#[derive(InitSpace)]
pub struct BookingDays {
    pub occupied_days: [u32; 12],
    pub year: u32,
    pub bump: u8,
}
