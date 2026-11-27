use anchor_lang::prelude::*;

use crate::{
    constants::MARKETPLACE_SEED,
    error::PactumCoreError,
    events::MarketplaceCreated,
    state::{DisputeMode, Marketplace, StorageBackend},
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CreateMarketplaceParams {
    pub committee: Pubkey,
    pub dispute_mode: DisputeMode,
    pub storage_backend: StorageBackend,
    pub kyb_credential: Option<Pubkey>,
    pub requires_identity: bool,
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
        params.committee != Pubkey::default(),
        PactumCoreError::InvalidMarketplaceConfig
    );
    if params.max_judge_stake > 0 {
        require!(
            params.min_judge_stake <= params.max_judge_stake,
            PactumCoreError::InvalidMarketplaceConfig
        );
    }

    let marketplace = &mut ctx.accounts.marketplace;
    marketplace.market_id = market_id;
    marketplace.committee = params.committee;
    marketplace.dispute_mode = params.dispute_mode;
    marketplace.storage_backend = params.storage_backend;
    marketplace.kyb_credential = params.kyb_credential;
    marketplace.requires_identity = params.requires_identity;
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
