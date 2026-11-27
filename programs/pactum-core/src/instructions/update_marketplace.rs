use anchor_lang::prelude::*;

use crate::{
    constants::{MARKETPLACE_SEED, MAX_MARKETPLACE_NAME_LEN},
    error::PactumCoreError,
    events::MarketplaceUpdated,
    state::{IntegratorPolicy, Marketplace},
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct UpdateMarketplaceParams {
    pub name: Option<String>,
    pub min_judge_stake: Option<u64>,
    pub max_judge_stake: Option<u64>,
    pub min_arbiter_stake: Option<u64>,
    pub min_jury_fee: Option<u64>,
    pub integrator_policy: Option<IntegratorPolicy>,
    pub kyb_credential: Option<Option<Pubkey>>,
    pub is_active: Option<bool>,
}

#[derive(Accounts)]
pub struct UpdateMarketplace<'info> {
    #[account(
        mut,
        seeds = [MARKETPLACE_SEED.as_bytes(), marketplace.market_id.to_le_bytes().as_ref()],
        bump = marketplace.bump,
        has_one = committee @ PactumCoreError::Unauthorized,
    )]
    pub marketplace: Account<'info, Marketplace>,

    pub committee: Signer<'info>,
}

pub fn handler_update_marketplace(
    ctx: Context<UpdateMarketplace>,
    params: UpdateMarketplaceParams,
) -> Result<()> {
    let marketplace = &mut ctx.accounts.marketplace;

    if let Some(name) = params.name {
        require!(
            !name.trim().is_empty() && name.len() <= MAX_MARKETPLACE_NAME_LEN,
            PactumCoreError::InvalidMarketplaceConfig
        );
        marketplace.name = name;
    }

    if let Some(min_judge_stake) = params.min_judge_stake {
        marketplace.min_judge_stake = min_judge_stake;
    }
    if let Some(max_judge_stake) = params.max_judge_stake {
        marketplace.max_judge_stake = max_judge_stake;
    }
    if marketplace.max_judge_stake > 0 {
        require!(
            marketplace.min_judge_stake <= marketplace.max_judge_stake,
            PactumCoreError::InvalidMarketplaceConfig
        );
    }
    if let Some(min_arbiter_stake) = params.min_arbiter_stake {
        marketplace.min_arbiter_stake = min_arbiter_stake;
    }
    if let Some(min_jury_fee) = params.min_jury_fee {
        marketplace.min_jury_fee = min_jury_fee;
    }
    if let Some(integrator_policy) = params.integrator_policy {
        marketplace.integrator_policy = integrator_policy;
    }
    if let Some(kyb_credential) = params.kyb_credential {
        marketplace.kyb_credential = kyb_credential;
    }

    if let Some(is_active) = params.is_active {
        marketplace.is_active = is_active;
    }

    emit!(MarketplaceUpdated {
        marketplace: marketplace.key(),
        committee: marketplace.committee,
    });
    Ok(())
}
