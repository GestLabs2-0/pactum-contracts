use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    constants::{CLIENT_SEED, MARKETPLACE_SEED, OFFER_SEED, PROVIDER_SEED},
    state::{Client, IntegratorProfile, Marketplace, Offer, PricingType, Provider},
};

use crate::{
    constants::{
        BOOKING_DAYS_SEED, DATE_RANGE_AGREEMENT_SEED, ESCROW_CONFIG_SEED, ESCROW_VAULT_SEED,
    },
    error::PactumEscrowError,
    instructions::CreateReservationParams,
    state::{BookingDays, DateRangeAgreement, EscrowConfig},
    utils::{init_reservation_state, reserve_days_cross_years, TimestampExt},
};

#[derive(Accounts)]
#[instruction(params: CreateReservationParams)]
pub struct CreateReservationCrossYear<'info> {
    pub client_authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [ESCROW_CONFIG_SEED.as_bytes()],
        bump = escrow_config.bump,
        constraint = escrow_config.is_initialized @ PactumEscrowError::EscrowNotInitialized,
    )]
    pub escrow_config: Box<Account<'info, EscrowConfig>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
        constraint = global_config.key() == escrow_config.global_config @ PactumEscrowError::EscrowNotInitialized,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        seeds = [MARKETPLACE_SEED.as_bytes(), marketplace.market_id.to_le_bytes().as_ref()],
        bump = marketplace.bump,
        seeds::program = pactum_core::ID,
        constraint = marketplace.is_active @ PactumEscrowError::MarketplaceInactive,
    )]
    pub marketplace: Box<Account<'info, Marketplace>>,

    #[account(
        seeds = [CLIENT_SEED.as_bytes(), marketplace.key().as_ref(), client_authority.key().as_ref()],
        bump = client.bump,
        seeds::program = pactum_core::ID,
        constraint = client.authority == client_authority.key() @ PactumEscrowError::Unauthorized,
        constraint = !client.is_banned @ PactumEscrowError::ParticipantBanned,
    )]
    pub client: Box<Account<'info, Client>>,

    #[account(
        seeds = [PROVIDER_SEED.as_bytes(), marketplace.key().as_ref(), provider.authority.as_ref()],
        bump = provider.bump,
        seeds::program = pactum_core::ID,
        constraint = !provider.is_banned @ PactumEscrowError::ParticipantBanned,
    )]
    pub provider: Box<Account<'info, Provider>>,

    #[account(
        seeds = [
            OFFER_SEED.as_bytes(),
            provider.key().as_ref(),
            offer.offer_id.to_le_bytes().as_ref(),
        ],
        bump = offer.bump,
        seeds::program = pactum_core::ID,
        constraint = offer.is_active @ PactumEscrowError::OfferInactive,
        constraint = offer.pricing_type == PricingType::DateRange @ PactumEscrowError::InvalidOfferPricingType,
    )]
    pub offer: Box<Account<'info, Offer>>,

    pub integrator: Option<Box<Account<'info, IntegratorProfile>>>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + BookingDays::INIT_SPACE,
        seeds = [
            BOOKING_DAYS_SEED.as_bytes(),
            offer.key().as_ref(),
            params.check_in.to_date()?.year.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub booking_days: Box<Account<'info, BookingDays>>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + BookingDays::INIT_SPACE,
        seeds = [
            BOOKING_DAYS_SEED.as_bytes(),
            offer.key().as_ref(),
            params.check_out.to_date()?.year.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub booking_days_next: Box<Account<'info, BookingDays>>,

    #[account(
        init,
        payer = payer,
        space = 8 + DateRangeAgreement::INIT_SPACE,
        seeds = [
            DATE_RANGE_AGREEMENT_SEED.as_bytes(),
            offer.key().as_ref(),
            client.key().as_ref(),
            params.check_in.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub reservation: Box<Account<'info, DateRangeAgreement>>,

    #[account(
        init,
        payer = payer,
        token::mint = usdc_mint,
        token::authority = escrow_vault,
        seeds = [ESCROW_VAULT_SEED.as_bytes(), reservation.key().as_ref()],
        bump,
    )]
    pub escrow_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = client_token_account.mint == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
    )]
    pub client_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = usdc_mint.key() == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handler_create_reservation_cross_year(
    ctx: Context<CreateReservationCrossYear>,
    params: CreateReservationParams,
) -> Result<()> {
    let check_in_date = params.check_in.to_date()?;
    let check_out_date = params.check_out.to_date()?;
    reserve_days_cross_years(
        &mut ctx.accounts.booking_days,
        &mut ctx.accounts.booking_days_next,
        &check_in_date,
        &check_out_date,
        ctx.bumps.booking_days,
        ctx.bumps.booking_days_next,
    )?;

    init_reservation_state(
        &mut ctx.accounts.reservation,
        &ctx.accounts.marketplace,
        &ctx.accounts.offer,
        &ctx.accounts.client,
        &ctx.accounts.provider,
        &ctx.accounts.integrator,
        &ctx.accounts.client_authority,
        &ctx.accounts.payer,
        &ctx.accounts.client_token_account,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.usdc_mint,
        &ctx.accounts.token_program,
        params.check_in,
        params.check_out,
        params.arbiter,
        ctx.bumps.escrow_vault,
        ctx.bumps.reservation,
    )
}
