use anchor_lang::prelude::*;

/// Seeds: `[TREASURY_CONFIG_SEED.as_bytes()]` (`["treasury_config"]`).
#[account]
#[derive(InitSpace)]
pub struct TreasuryConfig {
    pub authority: Pubkey,
    pub global_config: Pubkey,
    pub is_initialized: bool,
    pub bump: u8,
}
