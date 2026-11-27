use anchor_lang::prelude::*;

/// Seeds: `[INTEGRATOR_APP_SEED.as_bytes(), marketplace.key().as_ref(), authority.key().as_ref()]` (`["integrator_app", marketplace, authority]`).
#[account]
#[derive(InitSpace)]
pub struct IntegratorApplication {
    pub marketplace: Pubkey,
    pub authority: Pubkey,
    pub sas_kyb_attestation: Option<Pubkey>,
    pub default_fee_bps: u16,
    pub rent_payer: Pubkey,
    pub bump: u8,
}

/// Seeds: `[INTEGRATOR_SEED.as_bytes(), marketplace.key().as_ref(), authority.key().as_ref()]` (`["integrator", marketplace, authority]`).
#[account]
#[derive(InitSpace)]
pub struct IntegratorProfile {
    pub marketplace: Pubkey,
    pub authority: Pubkey,
    pub sas_kyb_attestation: Option<Pubkey>,
    pub default_fee_bps: u16,
    pub fallback_resolutions: u32,
    pub is_active: bool,
    pub is_banned: bool,
    pub bump: u8,
}
