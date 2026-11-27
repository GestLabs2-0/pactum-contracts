use anchor_lang::prelude::*;

/// Seeds: `[GLOBAL_CONFIG_SEED.as_bytes()]` (`["global_config"]`).
#[account]
#[derive(InitSpace)]
pub struct GlobalConfig {
    pub authority: Pubkey,
    pub usdc_mint: Pubkey,
    pub platform_vault: Pubkey,
    pub platform_vault_bump: u8,
    pub protocol_fee_bps: u16,
    pub max_protocol_fee: u64,
    pub dispute_escalation_fee: u64,
    pub sas_program: Pubkey,
    pub sas_credential: Pubkey,
    pub core_program: Pubkey,
    pub escrow_program: Pubkey,
    pub disputes_program: Pubkey,
    pub treasury_program: Pubkey,
    pub is_initialized: bool,
    pub bump: u8,
}
