use anchor_lang::prelude::*;

/// Seeds: `[USER_MAIN_SEED.as_bytes(), authority.key().as_ref()]` (`["user_main", authority]`).
#[account]
#[derive(InitSpace)]
pub struct UserMainAcc {
    pub authority: Pubkey,
    pub sas_attestation: Option<Pubkey>,
    pub bump: u8,
}

/// Unique identity anchor preventing the same KYC/KYB hash from being linked to multiple wallets.
/// Seeds: `[IDENTITY_SEED.as_bytes(), id_hash.as_ref()]` (`["identity", id_hash]`).
#[account]
#[derive(InitSpace)]
pub struct Identity {
    pub authority: Pubkey,
    pub verified_at: i64,
    pub linked: bool,
    pub bump: u8,
}
