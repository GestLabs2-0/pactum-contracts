use anchor_lang::prelude::*;

/// Seeds: `[USER_MAIN_SEED.as_bytes(), authority.key().as_ref()]` (`["user_main", authority]`).
#[account]
#[derive(InitSpace)]
pub struct UserMainAcc {
    pub authority: Pubkey,
    pub sas_attestation: Option<Pubkey>,
    pub is_banned: bool,
    pub bump: u8,
}

/// Optional local identity record (fallback/compatibility alongside SAS attestations).
/// Seeds: `[IDENTITY_SEED.as_bytes(), id_hash]` (`["identity", id_hash]`).
#[account]
#[derive(InitSpace)]
pub struct Identity {
    pub verified_at: i64,
    pub linked: bool,
    pub bump: u8,
}
