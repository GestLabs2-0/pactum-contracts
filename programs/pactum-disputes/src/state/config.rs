use anchor_lang::prelude::*;

/// Seeds: `[DISPUTE_CONFIG_SEED.as_bytes()]` (`["dispute_config"]`).
#[account]
#[derive(InitSpace)]
pub struct DisputeConfig {
    pub authority: Pubkey,
    pub global_config: Pubkey,
    pub p2p_window_secs: i64,
    pub vote_window_secs: i64,
    pub reveal_window_secs: i64,
    pub arbiter_sla_slash_bps: u16,
    pub no_fault_fee_bps: u16,
    pub is_initialized: bool,
    pub bump: u8,
}
