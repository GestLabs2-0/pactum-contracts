use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    constants::{CLIENT_SEED, MARKETPLACE_SEED, OFFER_SEED, PROVIDER_SEED, USER_MAIN_SEED},
    state::{Client, IntegratorProfile, Marketplace, Offer, PricingType, Provider, UserMainAcc},
};

use crate::{
    constants::{AGREEMENT_SEED, ESCROW_CONFIG_SEED, ESCROW_VAULT_SEED},
    error::PactumEscrowError,
    events::AgreementCreated,
    state::{Agreement, AgreementStatus, EscrowConfig},
    utils::{resolve_integrator, transfer_from_user},
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CreateFixedAgreementParams {
    pub nonce: u64,
    pub step_duration_secs: i64,
    pub deal_contract_hash: Option<[u8; 32]>,
    pub arbiter: Option<Pubkey>,
}

#[derive(Accounts)]
#[instruction(params: CreateFixedAgreementParams)]
pub struct CreateFixedAgreement<'info> {
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
        seeds = [USER_MAIN_SEED.as_bytes(), client_authority.key().as_ref()],
        bump = client_user_main.bump,
        seeds::program = pactum_core::ID,
    )]
    pub client_user_main: Box<Account<'info, UserMainAcc>>,

    #[account(
        seeds = [CLIENT_SEED.as_bytes(), marketplace.key().as_ref(), client_user_main.key().as_ref()],
        bump = client.bump,
        seeds::program = pactum_core::ID,
        constraint = !client.is_banned @ PactumEscrowError::ParticipantBanned,
    )]
    pub client: Box<Account<'info, Client>>,

    #[account(
        seeds = [USER_MAIN_SEED.as_bytes(), provider_user_main.authority.as_ref()],
        bump = provider_user_main.bump,
        seeds::program = pactum_core::ID,
    )]
    pub provider_user_main: Box<Account<'info, UserMainAcc>>,

    #[account(
        seeds = [PROVIDER_SEED.as_bytes(), marketplace.key().as_ref(), provider_user_main.key().as_ref()],
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
        constraint = offer.pricing_type == PricingType::Fixed @ PactumEscrowError::InvalidOfferPricingType,
    )]
    pub offer: Box<Account<'info, Offer>>,

    pub integrator: Option<Box<Account<'info, IntegratorProfile>>>,

    #[account(
        init,
        payer = payer,
        space = Agreement::space(1),
        seeds = [
            AGREEMENT_SEED.as_bytes(),
            offer.key().as_ref(),
            client.key().as_ref(),
            params.nonce.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub agreement: Box<Account<'info, Agreement>>,

    #[account(
        init,
        payer = payer,
        token::mint = usdc_mint,
        token::authority = escrow_vault,
        seeds = [ESCROW_VAULT_SEED.as_bytes(), agreement.key().as_ref()],
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

pub fn handler_create_fixed_agreement(
    ctx: Context<CreateFixedAgreement>,
    params: CreateFixedAgreementParams,
) -> Result<()> {
    require!(
        params.step_duration_secs > 0,
        PactumEscrowError::InvalidMilestones
    );
    let offer = &ctx.accounts.offer;
    let total_amount = offer
        .base_price
        .ok_or(PactumEscrowError::InvalidOfferParams)?;
    require!(total_amount > 0, PactumEscrowError::ZeroAmount);

    let (integrator, integrator_fee_bps) =
        resolve_integrator(&ctx.accounts.integrator, ctx.accounts.marketplace.key())?;

    let client_bond = offer.required_client_bond;
    let deposit_amount = total_amount
        .checked_add(client_bond)
        .ok_or(PactumEscrowError::MathOverflow)?;

    transfer_from_user(
        &ctx.accounts.token_program,
        &ctx.accounts.client_token_account,
        &ctx.accounts.escrow_vault,
        &ctx.accounts.usdc_mint,
        &ctx.accounts.client_authority,
        deposit_amount,
    )?;

    let agreement = &mut ctx.accounts.agreement;
    agreement.nonce = params.nonce;
    agreement.offer = offer.key();
    agreement.client = ctx.accounts.client.key();
    agreement.provider = ctx.accounts.provider.key();
    agreement.integrator = integrator;
    agreement.integrator_fee_bps = integrator_fee_bps;
    agreement.arbiter = params.arbiter.or(offer.default_arbiter);
    agreement.jury_fee = offer.jury_fee;
    agreement.total_amount = total_amount;
    agreement.provider_bond = offer.provider_bond;
    agreement.client_bond = client_bond;
    agreement.released_amount = 0;
    agreement.milestone_amounts = vec![total_amount];
    agreement.completed_milestones = 0;
    agreement.step_duration_secs = params.step_duration_secs;
    agreement.current_deadline_ts = 0;
    agreement.approval_window_secs = offer.approval_window_secs;
    agreement.created_at = Clock::get()?.unix_timestamp;
    agreement.submitted_at = 0;
    agreement.completed_at = 0;
    agreement.deliverable_hash = [0u8; 32];
    agreement.state_hash = offer.state_hash;
    agreement.deal_contract_hash = params.deal_contract_hash;
    agreement.status = AgreementStatus::Pending;
    agreement.previous_status = None;
    agreement.client_reviewed = false;
    agreement.provider_reviewed = false;
    agreement.rent_payer = ctx.accounts.payer.key();
    agreement.escrow_vault_bump = ctx.bumps.escrow_vault;
    agreement.bump = ctx.bumps.agreement;

    emit!(AgreementCreated {
        agreement: agreement.key(),
        offer: offer.key(),
        client: ctx.accounts.client.key(),
        provider: ctx.accounts.provider.key(),
        total_amount,
    });
    Ok(())
}
