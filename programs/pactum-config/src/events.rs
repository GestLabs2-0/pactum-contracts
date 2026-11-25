use anchor_lang::prelude::*;

#[event]
pub struct ConfigInitialized {
    pub authority: Pubkey,
    pub usdc_mint: Pubkey,
    pub protocol_fee_bps: u16,
    pub max_protocol_fee: u64,
    pub dispute_escalation_fee: u64,
}

#[event]
pub struct ConfigUpdated {
    pub authority: Pubkey,
    pub protocol_fee_bps: u16,
    pub max_protocol_fee: u64,
    pub dispute_escalation_fee: u64,
}

#[event]
pub struct FeesWithdrawn {
    pub authority: Pubkey,
    pub destination: Pubkey,
    pub amount: u64,
}
