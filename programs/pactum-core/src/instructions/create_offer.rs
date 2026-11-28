use anchor_lang::prelude::*;

use crate::{
    constants::{OFFER_SEED, PROVIDER_SEED, USER_MAIN_SEED},
    error::PactumCoreError,
    events::OfferCreated,
    state::{
        CancellationPolicy, DisputeMode, Marketplace, Offer, PricingType, Provider, UserMainAcc,
    },
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CreateOfferParams {
    pub base_price: Option<u64>,
    pub pricing_type: PricingType,
    pub approval_window_secs: i64,
    pub provider_bond: u64,
    pub required_client_bond: u64,
    pub jury_fee: u64,
    pub default_arbiter: Option<Pubkey>,
    pub cancellation_policy: Option<CancellationPolicy>,
    pub content_ref: [u8; 32],
    pub state_hash: [u8; 32],
}

#[derive(Accounts)]
pub struct CreateOffer<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + Offer::INIT_SPACE,
        seeds = [
            OFFER_SEED.as_bytes(),
            provider.key().as_ref(),
            provider.offer_count.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub offer: Account<'info, Offer>,

    #[account(
        mut,
        seeds = [PROVIDER_SEED.as_bytes(), marketplace.key().as_ref(), user_main.key().as_ref()],
        bump = provider.bump,
        has_one = marketplace,
        has_one = user_main,
        constraint = !provider.is_banned @ PactumCoreError::ParticipantLocallyBanned,
    )]
    pub provider: Account<'info, Provider>,

    #[account(
        seeds = [USER_MAIN_SEED.as_bytes(), authority.key().as_ref()],
        bump = user_main.bump,
        has_one = authority @ PactumCoreError::Unauthorized,
    )]
    pub user_main: Account<'info, UserMainAcc>,

    #[account(constraint = marketplace.is_active @ PactumCoreError::MarketplaceInactive)]
    pub marketplace: Account<'info, Marketplace>,

    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_create_offer(ctx: Context<CreateOffer>, params: CreateOfferParams) -> Result<()> {
    require!(
        params.approval_window_secs > 0,
        PactumCoreError::InvalidOfferParams
    );
    if params.pricing_type == PricingType::Fixed {
        let price = params
            .base_price
            .ok_or(PactumCoreError::InvalidOfferParams)?;
        require!(price > 0, PactumCoreError::InvalidOfferParams);
    }
    if ctx.accounts.marketplace.dispute_mode != DisputeMode::ArbiterOnly {
        require!(
            params.jury_fee >= ctx.accounts.marketplace.min_jury_fee,
            PactumCoreError::InvalidOfferParams
        );
    }
    if let Some(policy) = params.cancellation_policy {
        policy.validate()?;
    }

    let provider = &mut ctx.accounts.provider;
    let offer_id = provider.offer_count;
    provider.offer_count = offer_id
        .checked_add(1)
        .ok_or(PactumCoreError::OfferCountOverflow)?;

    let offer = &mut ctx.accounts.offer;
    offer.offer_id = offer_id;
    offer.base_price = params.base_price;
    offer.pricing_type = params.pricing_type;
    offer.approval_window_secs = params.approval_window_secs;
    offer.provider_bond = params.provider_bond;
    offer.required_client_bond = params.required_client_bond;
    offer.jury_fee = params.jury_fee;
    offer.default_arbiter = params.default_arbiter;
    offer.cancellation_policy = params.cancellation_policy;
    offer.content_ref = params.content_ref;
    offer.state_hash = params.state_hash;
    offer.is_active = true;
    offer.rent_payer = ctx.accounts.payer.key();
    offer.bump = ctx.bumps.offer;

    emit!(OfferCreated {
        offer: offer.key(),
        provider: provider.key(),
        marketplace: ctx.accounts.marketplace.key(),
        offer_id,
    });
    Ok(())
}
