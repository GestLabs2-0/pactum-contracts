use anchor_lang::prelude::*;

/// Seeds: `[TREASURY_CONFIG_SEED.as_bytes()]` (`["treasury_config"]`).
#[account]
#[derive(InitSpace)]
pub struct TreasuryConfig {
    pub authority: Pubkey,
    pub global_config: Pubkey,
    pub stake_vault: Pubkey,
    pub treasury_bump: u8,
    pub is_initialized: bool,
    pub bump: u8,
}
