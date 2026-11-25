use anchor_lang::prelude::*;

use crate::{
    constants::OFFER_SEED,
    error::PactumCoreError,
    state::{Offer, PricingType, Provider, UserMainAcc},
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct UpdateOfferParams {
    pub base_price: Option<Option<u64>>,
    pub provider_bond: Option<u64>,
    pub required_client_bond: Option<u64>,
    pub default_arbiter: Option<Option<Pubkey>>,
    pub content_ref: Option<[u8; 32]>,
    pub state_hash: Option<[u8; 32]>,
    pub is_active: Option<bool>,
}

#[derive(Accounts)]
pub struct UpdateOffer<'info> {
    #[account(
        mut,
        seeds = [
            OFFER_SEED.as_bytes(),
            provider.key().as_ref(),
            offer.offer_id.to_le_bytes().as_ref(),
        ],
        bump = offer.bump,
    )]
    pub offer: Account<'info, Offer>,

    #[account(
        has_one = user_main,
        constraint = !provider.is_banned @ PactumCoreError::ParticipantLocallyBanned,
    )]
    pub provider: Account<'info, Provider>,

    #[account(
        has_one = authority @ PactumCoreError::Unauthorized,
        constraint = !user_main.is_banned @ PactumCoreError::UserGloballyBanned,
    )]
    pub user_main: Account<'info, UserMainAcc>,

    pub authority: Signer<'info>,
}

pub fn handler_update_offer(ctx: Context<UpdateOffer>, params: UpdateOfferParams) -> Result<()> {
    let offer = &mut ctx.accounts.offer;

    if let Some(base_price) = params.base_price {
        if offer.pricing_type == PricingType::Fixed {
            let price = base_price.ok_or(PactumCoreError::InvalidOfferParams)?;
            require!(price > 0, PactumCoreError::InvalidOfferParams);
        }
        offer.base_price = base_price;
    }
    if let Some(provider_bond) = params.provider_bond {
        offer.provider_bond = provider_bond;
    }
    if let Some(required_client_bond) = params.required_client_bond {
        offer.required_client_bond = required_client_bond;
    }
    if let Some(default_arbiter) = params.default_arbiter {
        offer.default_arbiter = default_arbiter;
    }
    if let Some(content_ref) = params.content_ref {
        offer.content_ref = content_ref;
    }
    if let Some(state_hash) = params.state_hash {
        offer.state_hash = state_hash;
    }
    if let Some(is_active) = params.is_active {
        offer.is_active = is_active;
    }

    Ok(())
}
