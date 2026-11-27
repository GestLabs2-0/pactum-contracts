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

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum IntegratorPolicy {
    Permissionless,
    CommitteeApproval,
    KybAndCommittee,
}

/// Seeds: `[MARKETPLACE_SEED.as_bytes(), market_id.to_le_bytes().as_ref()]` (`["marketplace", market_id]`).
#[account]
#[derive(InitSpace)]
pub struct Marketplace {
    pub market_id: u64,
    #[max_len(30)]
    pub name: String,
    /// Committee pubkey controlling operating market kyb
    pub committee: Pubkey,
    /// Dispute mode each option allow markets to follow different dispute modes:
    /// to handle high risk operations with centralized enterprises in a decentralized environment you follow arbiter
    /// if only a decentralized jury is required, then DecentralizedJury will work quite well
    /// Is possible to use both modes using hybrid
    pub dispute_mode: DisputeMode,
    /// Which type or storage is gonna be used. That refers to every data structure derived by this market:
    /// - Offers
    /// - Agreements
    pub storage_backend: StorageBackend,
    pub integrator_policy: IntegratorPolicy,
    /// Credential under which every identity will be published following SAS
    pub kyb_credential: Option<Pubkey>,
    /// Does this market requires an identity?
    pub requires_identity: bool,
    /// Min judge stake has the purpose to stablish a entry barrier for every judge
    pub min_judge_stake: u64,
    /// Max judge stake has the purpose to avoid 51% attacks
    pub max_judge_stake: u64,
    /// Min stake per arbiter
    pub min_arbiter_stake: u64,
    /// Min jury fee to stablish for an agreement which requires a jury
    pub min_jury_fee: u64,
    /// How many pools of judges are there
    pub pool_count: u16,
    /// Is market active?
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
