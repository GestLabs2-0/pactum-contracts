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

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum IdentityPolicy {
    None,
    InitialKyc,
    KycAndMarketplaceProof,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub struct RoleIdentityConfig {
    pub policy: IdentityPolicy,
    pub credential: Option<Pubkey>,
}

impl RoleIdentityConfig {
    pub fn none() -> Self {
        Self {
            policy: IdentityPolicy::None,
            credential: None,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.policy == IdentityPolicy::KycAndMarketplaceProof {
            let cred = self
                .credential
                .ok_or(crate::error::PactumCoreError::InvalidMarketplaceConfig)?;
            require!(
                cred != Pubkey::default(),
                crate::error::PactumCoreError::InvalidMarketplaceConfig
            );
        }
        Ok(())
    }
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
    /// Whether clients must hold a base Pactum-ally SAS KYC attestation
    pub client_requires_kyc: bool,
    /// Identity policy and optional marketplace-specific SAS credential for providers
    pub provider_identity: RoleIdentityConfig,
    /// Identity policy and optional marketplace-specific SAS credential for judges
    pub judge_identity: RoleIdentityConfig,
    /// Min judge stake has the purpose to stablish a entry barrier for every judge
    pub min_judge_stake: u64,
    /// Max judge stake has the purpose to avoid 51% attacks
    pub max_judge_stake: u64,
    /// Min stake per arbiter
    pub min_arbiter_stake: u64,
    /// Min jury fee to stablish for an agreement which requires a jury
    pub min_jury_fee: u64,
    /// Percentage (0..=100) of provider bond slashed on late provider cancellation
    pub provider_cancellation_penalty_pct: u8,
    /// How many pools of judges are there
    pub pool_count: u16,
    /// Is market active?
    pub is_active: bool,
    pub bump: u8,
}

impl Marketplace {
    /// Verifies that the marketplace is active and a client satisfies ban and base KYC rules.
    pub fn assert_client_eligible(&self, user_main: &crate::state::UserMainAcc) -> Result<()> {
        let cfg = RoleIdentityConfig {
            policy: if self.client_requires_kyc {
                IdentityPolicy::InitialKyc
            } else {
                IdentityPolicy::None
            },
            credential: None,
        };
        self.assert_role_eligible(user_main, &cfg, Pubkey::default(), None)
    }

    /// Verifies that the marketplace is active and a participant satisfies ban and role-specific
    /// identity requirements (base KYC and/or marketplace-specific SAS credential).
    pub fn assert_role_eligible(
        &self,
        user_main: &crate::state::UserMainAcc,
        role_cfg: &RoleIdentityConfig,
        sas_program: Pubkey,
        marketplace_sas: Option<(&AccountInfo, &AccountInfo)>,
    ) -> Result<()> {
        require!(
            self.is_active,
            crate::error::PactumCoreError::MarketplaceInactive
        );
        require!(
            !user_main.is_banned,
            crate::error::PactumCoreError::UserGloballyBanned
        );
        match role_cfg.policy {
            IdentityPolicy::None => Ok(()),
            IdentityPolicy::InitialKyc => {
                require!(
                    user_main.sas_attestation.is_some(),
                    crate::error::PactumCoreError::IdentityRequired
                );
                Ok(())
            }
            IdentityPolicy::KycAndMarketplaceProof => {
                require!(
                    user_main.sas_attestation.is_some(),
                    crate::error::PactumCoreError::IdentityRequired
                );
                let expected_cred = role_cfg
                    .credential
                    .ok_or(crate::error::PactumCoreError::InvalidMarketplaceConfig)?;
                let (att_info, cred_info) =
                    marketplace_sas.ok_or(crate::error::PactumCoreError::IdentityRequired)?;
                crate::state::verify_sas_attestation(
                    att_info,
                    cred_info,
                    sas_program,
                    expected_cred,
                    user_main.authority,
                )?;
                Ok(())
            }
        }
    }
}
