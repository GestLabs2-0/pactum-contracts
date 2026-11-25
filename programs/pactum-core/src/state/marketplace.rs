use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum DisputeMode {
    ArbiterOnly,
    DecentralizedJury,
    Hybrid,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum StorageBackend {
    Arweave,
    Irys,
    Ipfs,
    Custom,
}

/// Seeds: `[MARKETPLACE_SEED.as_bytes(), market_id.to_le_bytes().as_ref()]` (`["marketplace", market_id]`).
#[account]
#[derive(InitSpace)]
pub struct Marketplace {
    pub market_id: u64,
    pub committee: Pubkey,
    pub dispute_mode: DisputeMode,
    pub storage_backend: StorageBackend,
    pub kyb_credential: Option<Pubkey>,
    pub requires_identity: bool,
    pub min_judge_stake: u64,
    pub max_judge_stake: u64,
    pub min_arbiter_stake: u64,
    pub min_jury_fee: u64,
    pub pool_count: u16,
    pub is_active: bool,
    pub bump: u8,
}

impl Marketplace {
    /// Verifies that the marketplace is active and the user satisfies ban and identity requirements.
    pub fn assert_participant_eligible(&self, user_main: &crate::state::UserMainAcc) -> Result<()> {
        require!(
            self.is_active,
            crate::error::PactumCoreError::MarketplaceInactive
        );
        require!(
            !user_main.is_banned,
            crate::error::PactumCoreError::UserGloballyBanned
        );
        if self.requires_identity {
            require!(
                user_main.sas_attestation.is_some(),
                crate::error::PactumCoreError::IdentityRequired
            );
        }
        Ok(())
    }
}
