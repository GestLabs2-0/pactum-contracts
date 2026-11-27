use anchor_lang::prelude::*;

/// Seeds: `[ARBITER_APP_SEED.as_bytes(), marketplace.key().as_ref(), authority.key().as_ref()]` (`["arbiter_app", marketplace, authority]`).
#[account]
#[derive(InitSpace)]
pub struct ArbiterApplication {
    pub marketplace: Pubkey,
    pub authority: Pubkey,
    pub sas_kyb_attestation: Pubkey,
    pub arbiter_fee: u64,
    pub sla_secs: i64,
    pub rent_payer: Pubkey,
    pub bump: u8,
}

/// Seeds: `[ARBITER_SEED.as_bytes(), marketplace.key().as_ref(), authority.key().as_ref()]` (`["arbiter", marketplace, authority]`).
#[account]
#[derive(InitSpace)]
pub struct ArbiterProfile {
    pub marketplace: Pubkey,
    pub authority: Pubkey,
    pub sas_kyb_attestation: Pubkey,
    pub arbiter_fee: u64,
    pub staked_amount: u64,
    pub sla_secs: i64,
    pub active_disputes: u32,
    pub cases_resolved: u32,
    pub is_active: bool,
    pub bump: u8,
}
