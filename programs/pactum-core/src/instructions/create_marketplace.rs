use anchor_lang::prelude::*;

use crate::{
    constants::{MARKETPLACE_SEED, MAX_MARKETPLACE_NAME_LEN},
    error::PactumCoreError,
    events::MarketplaceCreated,
    state::{DisputeMode, IntegratorPolicy, Marketplace, RoleIdentityConfig, StorageBackend},
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CreateMarketplaceParams {
    pub name: String,
    pub committee: Pubkey,
    pub dispute_mode: DisputeMode,
    pub storage_backend: StorageBackend,
    pub integrator_policy: IntegratorPolicy,
    pub kyb_credential: Option<Pubkey>,
    pub client_requires_kyc: bool,
    pub provider_identity: RoleIdentityConfig,
    pub judge_identity: RoleIdentityConfig,
    pub min_judge_stake: u64,
    pub max_judge_stake: u64,
    pub min_arbiter_stake: u64,
    pub min_jury_fee: u64,
}

#[derive(Accounts)]
#[instruction(market_id: u64)]
pub struct CreateMarketplace<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + Marketplace::INIT_SPACE,
        seeds = [MARKETPLACE_SEED.as_bytes(), market_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub marketplace: Account<'info, Marketplace>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_create_marketplace(
    ctx: Context<CreateMarketplace>,
    market_id: u64,
    params: CreateMarketplaceParams,
) -> Result<()> {
    require!(
        !params.name.trim().is_empty() && params.name.len() <= MAX_MARKETPLACE_NAME_LEN,
        PactumCoreError::InvalidMarketplaceConfig
    );
    require!(
        params.committee != Pubkey::default(),
        PactumCoreError::InvalidMarketplaceConfig
    );
    if params.max_judge_stake > 0 {
        require!(
            params.min_judge_stake <= params.max_judge_stake,
            PactumCoreError::InvalidMarketplaceConfig
        );
    }

    params.provider_identity.validate()?;
    params.judge_identity.validate()?;

    let marketplace = &mut ctx.accounts.marketplace;
    marketplace.market_id = market_id;
    marketplace.name = params.name;
    marketplace.committee = params.committee;
    marketplace.dispute_mode = params.dispute_mode;
    marketplace.storage_backend = params.storage_backend;
    marketplace.integrator_policy = params.integrator_policy;
    marketplace.kyb_credential = params.kyb_credential;
    marketplace.client_requires_kyc = params.client_requires_kyc;
    marketplace.provider_identity = params.provider_identity;
    marketplace.judge_identity = params.judge_identity;
    marketplace.min_judge_stake = params.min_judge_stake;
    marketplace.max_judge_stake = params.max_judge_stake;
    marketplace.min_arbiter_stake = params.min_arbiter_stake;
    marketplace.min_jury_fee = params.min_jury_fee;
    marketplace.pool_count = 0;
    marketplace.is_active = true;
    marketplace.bump = ctx.bumps.marketplace;

    emit!(MarketplaceCreated {
        marketplace: marketplace.key(),
        market_id,
        committee: params.committee,
    });
    Ok(())
}
