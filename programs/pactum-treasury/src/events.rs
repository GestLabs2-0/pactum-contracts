use anchor_lang::prelude::*;
use pactum_core::state::ParticipantRole;

#[event]
pub struct TreasuryInitialized {
    pub authority: Pubkey,
    pub global_config: Pubkey,
    pub stake_vault: Pubkey,
}

#[event]
pub struct StakeUpdated {
    pub marketplace: Pubkey,
    pub target: Pubkey,
    pub role: ParticipantRole,
    pub amount: u64,
    pub is_stake: bool,
    pub new_total: u64,
}

#[event]
pub struct StakeSlashed {
    pub target: Pubkey,
    pub role: ParticipantRole,
    pub amount: u64,
    pub recipient: Pubkey,
}
