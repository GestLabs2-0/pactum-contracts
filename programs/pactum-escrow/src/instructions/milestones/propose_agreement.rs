use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    constants::{CLIENT_SEED, MARKETPLACE_SEED, OFFER_SEED, PROVIDER_SEED},
    state::{Client, IntegratorProfile, Marketplace, Offer, PricingType, Provider},
};

use crate::{
    constants::{AGREEMENT_SEED, ESCROW_CONFIG_SEED, ESCROW_VAULT_SEED, MAX_MILESTONES},
    error::PactumEscrowError,
    events::AgreementProposed,
    state::{Agreement, AgreementStatus, EscrowConfig},
    utils::{resolve_integrator, transfer_from_user},
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ProposeAgreementParams {
    pub nonce: u64,
    pub milestone_amounts: Vec<u64>,
    pub step_duration_secs: i64,
    pub deal_contract_hash: Option<[u8; 32]>,
    pub arbiter: Option<Pubkey>,
}

#[derive(Accounts)]
#[instruction(params: ProposeAgreementParams)]
pub struct ProposeAgreement<'info> {
    pub provider_authority: Signer<'info>,

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
        seeds = [PROVIDER_SEED.as_bytes(), marketplace.key().as_ref(), provider_authority.key().as_ref()],
        bump = provider.bump,
        seeds::program = pactum_core::ID,
        constraint = provider.authority == provider_authority.key() @ PactumEscrowError::Unauthorized,
        constraint = !provider.is_banned @ PactumEscrowError::ParticipantBanned,
    )]
    pub provider: Box<Account<'info, Provider>>,

    #[account(
        seeds = [CLIENT_SEED.as_bytes(), marketplace.key().as_ref(), client.authority.as_ref()],
        bump = client.bump,
        seeds::program = pactum_core::ID,
        constraint = !client.is_banned @ PactumEscrowError::ParticipantBanned,
    )]
    pub client: Box<Account<'info, Client>>,

    #[account(
        seeds = [
            OFFER_SEED.as_bytes(),
            provider.key().as_ref(),
            offer.offer_id.to_le_bytes().as_ref(),
        ],
        bump = offer.bump,
        seeds::program = pactum_core::ID,
        constraint = offer.is_active @ PactumEscrowError::OfferInactive,
        constraint = offer.pricing_type == PricingType::Custom @ PactumEscrowError::InvalidOfferPricingType,
    )]
    pub offer: Box<Account<'info, Offer>>,

    pub integrator: Option<Box<Account<'info, IntegratorProfile>>>,

    #[account(
        init,
        payer = payer,
        space = Agreement::space(params.milestone_amounts.len()),
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
        constraint = provider_token_account.mint == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
    )]
    pub provider_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = usdc_mint.key() == global_config.usdc_mint @ PactumEscrowError::InvalidTokenMint,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handler_propose_agreement(
    ctx: Context<ProposeAgreement>,
    params: ProposeAgreementParams,
) -> Result<()> {
    require!(
        !params.milestone_amounts.is_empty()
            && params.milestone_amounts.len() <= MAX_MILESTONES
            && params.step_duration_secs > 0,
        PactumEscrowError::InvalidMilestones
    );

    let mut total_amount: u64 = 0;
    for &amt in &params.milestone_amounts {
        require!(amt > 0, PactumEscrowError::InvalidMilestones);
        total_amount = total_amount
            .checked_add(amt)
            .ok_or(PactumEscrowError::MathOverflow)?;
    }

    let offer = &ctx.accounts.offer;
    let (integrator, integrator_fee_bps) =
        resolve_integrator(&ctx.accounts.integrator, ctx.accounts.marketplace.key())?;

    if offer.provider_bond > 0 {
        transfer_from_user(
            &ctx.accounts.token_program,
            &ctx.accounts.provider_token_account,
            &ctx.accounts.escrow_vault,
            &ctx.accounts.usdc_mint,
            &ctx.accounts.provider_authority,
            offer.provider_bond,
        )?;
    }

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
    agreement.client_bond = offer.required_client_bond;
    agreement.released_amount = 0;
    agreement.milestone_amounts = params.milestone_amounts;
    agreement.completed_milestones = 0;
    agreement.step_duration_secs = params.step_duration_secs;
    agreement.current_deadline_ts = 0;
    agreement.approval_window_secs = offer.approval_window_secs;
    agreement.created_at = Clock::get()?.unix_timestamp;
    agreement.submitted_at = 0;
    agreement.completed_at = 0;
    agreement.deliverable_hash = [0u8; 32];
    agreement.content_ref = offer.content_ref;
    agreement.deal_contract_hash = params.deal_contract_hash;
    agreement.status = AgreementStatus::Proposed;
    agreement.previous_status = None;
    agreement.client_reviewed = false;
    agreement.provider_reviewed = false;
    agreement.rent_payer = ctx.accounts.payer.key();
    agreement.escrow_vault_bump = ctx.bumps.escrow_vault;
    agreement.bump = ctx.bumps.agreement;

    emit!(AgreementProposed {
        agreement: agreement.key(),
        offer: offer.key(),
        client: ctx.accounts.client.key(),
        provider: ctx.accounts.provider.key(),
        total_amount,
    });
    Ok(())
}
