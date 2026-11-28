use anchor_lang::prelude::*;

/// Seeds: `[ESCROW_CONFIG_SEED.as_bytes()]` (`["escrow_config"]`).
#[account]
#[derive(InitSpace)]
pub struct EscrowConfig {
    pub authority: Pubkey,
    pub global_config: Pubkey,
    pub is_initialized: bool,
    pub bump: u8,
}
