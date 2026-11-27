use anchor_lang::prelude::*;

/// Seeds: `[CORE_CONFIG_SEED.as_bytes()]` (`["core_config"]`).
#[account]
#[derive(InitSpace)]
pub struct CoreConfig {
    pub authority: Pubkey,
    pub global_config: Pubkey,
    pub bump: u8,
}
