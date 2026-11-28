use anchor_lang::prelude::*;
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    constants::{CLIENT_SEED, MARKETPLACE_SEED, PROVIDER_SEED},
    state::{Client, DisputeMode as MarketDisputeMode, DisputeParty, Marketplace, Provider},
};
use pactum_escrow::{
    constants::ESCROW_CONFIG_SEED,
    cpi::accounts::CpiFreezeAgreement,
    cpi::cpi_freeze_agreement,
    program::PactumEscrow,
    state::{Agreement, DateRangeAgreement, EscrowConfig},
};

use crate::{
    constants::{CPI_AUTHORITY_SEED, DISPUTE_CONFIG_SEED, DISPUTE_SEED, JURY_PANEL_SIZE},
    error::PactumDisputesError,
    events::DisputeOpened,
    state::{Dispute, DisputeConfig, DisputeMode, DisputeStatus},
    utils::extract_agreement_snapshot,
};

#[derive(Accounts)]
pub struct OpenDispute<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + Dispute::INIT_SPACE,
        seeds = [DISPUTE_SEED.as_bytes(), target_agreement.key().as_ref()],
        bump,
    )]
    pub dispute: Box<Account<'info, Dispute>>,

    /// CHECK: Verified in handler against `agreement` or `reservation` key.
    pub target_agreement: UncheckedAccount<'info>,

    #[account(mut)]
    pub agreement: Option<Box<Account<'info, Agreement>>>,

    #[account(mut)]
    pub reservation: Option<Box<Account<'info, DateRangeAgreement>>>,

    #[account(
        seeds = [MARKETPLACE_SEED.as_bytes(), marketplace.market_id.to_le_bytes().as_ref()],
        bump = marketplace.bump,
        seeds::program = pactum_core::ID,
    )]
    pub marketplace: Box<Account<'info, Marketplace>>,

    #[account(
        seeds = [CLIENT_SEED.as_bytes(), marketplace.key().as_ref(), client.authority.as_ref()],
        bump = client.bump,
        seeds::program = pactum_core::ID,
    )]
    pub client: Box<Account<'info, Client>>,

    #[account(
        seeds = [PROVIDER_SEED.as_bytes(), marketplace.key().as_ref(), provider.authority.as_ref()],
        bump = provider.bump,
        seeds::program = pactum_core::ID,
    )]
    pub provider: Box<Account<'info, Provider>>,

    #[account(
        seeds = [DISPUTE_CONFIG_SEED.as_bytes()],
        bump = dispute_config.bump,
        constraint = dispute_config.is_initialized @ PactumDisputesError::ConfigNotInitialized,
    )]
    pub dispute_config: Box<Account<'info, DisputeConfig>>,

    #[account(
        seeds = [ESCROW_CONFIG_SEED.as_bytes()],
        bump = escrow_config.bump,
        seeds::program = pactum_escrow::ID,
    )]
    pub escrow_config: Box<Account<'info, EscrowConfig>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: Disputes CPI authority PDA signing freeze CPI into `pactum-escrow`.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub signer: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub escrow_program: Program<'info, PactumEscrow>,
    pub system_program: Program<'info, System>,
}

pub fn handler_open_dispute(ctx: Context<OpenDispute>) -> Result<()> {
    let snapshot = extract_agreement_snapshot(
        &ctx.accounts.agreement,
        &ctx.accounts.reservation,
        ctx.accounts.marketplace.key(),
    )?;
    require_keys_eq!(
        ctx.accounts.target_agreement.key(),
        snapshot.key,
        PactumDisputesError::InvalidAgreementTarget
    );
    require_keys_eq!(
        snapshot.client,
        ctx.accounts.client.key(),
        PactumDisputesError::InvalidPartyAccount
    );
    require_keys_eq!(
        snapshot.provider,
        ctx.accounts.provider.key(),
        PactumDisputesError::InvalidPartyAccount
    );

    let signer_key = ctx.accounts.signer.key();
    let opened_by = if signer_key == ctx.accounts.client.authority {
        DisputeParty::Client
    } else if signer_key == ctx.accounts.provider.authority {
        DisputeParty::Provider
    } else {
        return err!(PactumDisputesError::Unauthorized);
    };

    let mode = match (ctx.accounts.marketplace.dispute_mode, snapshot.arbiter) {
        (MarketDisputeMode::ArbiterOnly, Some(_)) => DisputeMode::Arbiter,
        (MarketDisputeMode::ArbiterOnly, None) => {
            return err!(PactumDisputesError::IncompatibleDisputeMode)
        }
        (MarketDisputeMode::DecentralizedJury, None) => DisputeMode::DecentralizedJury,
        (MarketDisputeMode::DecentralizedJury, Some(_)) => {
            return err!(PactumDisputesError::IncompatibleDisputeMode)
        }
        (MarketDisputeMode::Hybrid, Some(_)) => DisputeMode::Arbiter,
        (MarketDisputeMode::Hybrid, None) => DisputeMode::DecentralizedJury,
    };

    let seeds: &[&[u8]] = &[CPI_AUTHORITY_SEED.as_bytes(), &[ctx.bumps.cpi_authority]];
    let signer = &[seeds];
    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.escrow_program.key(),
        CpiFreezeAgreement {
            escrow_config: ctx.accounts.escrow_config.to_account_info(),
            global_config: ctx.accounts.global_config.to_account_info(),
            cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
            agreement: ctx.accounts.agreement.as_ref().map(|a| a.to_account_info()),
            reservation: ctx
                .accounts
                .reservation
                .as_ref()
                .map(|r| r.to_account_info()),
        },
        signer,
    );
    cpi_freeze_agreement(cpi_ctx)?;

    let now = Clock::get()?.unix_timestamp;
    let dispute = &mut ctx.accounts.dispute;
    dispute.agreement = snapshot.key;
    dispute.marketplace = snapshot.marketplace;
    dispute.client = snapshot.client;
    dispute.provider = snapshot.provider;
    dispute.opened_by = opened_by;
    dispute.mode = mode;
    dispute.assigned_arbiter = snapshot.arbiter;
    dispute.proposed_replacement_arbiter = None;
    dispute.proposed_by = None;
    dispute.selected_leaves = [0u32; JURY_PANEL_SIZE];
    dispute.panel_size = 0;
    dispute.round = 0;
    dispute.runoff_choices = None;
    dispute.client_evidence = None;
    dispute.provider_evidence = None;
    dispute.status = DisputeStatus::OpenP2P;
    dispute.outcome = None;
    dispute.is_reservation = snapshot.is_reservation;
    dispute.created_at = now;
    dispute.phase_started_at = now;
    dispute.sla_deadline = 0;
    dispute.rent_payer = ctx.accounts.payer.key();
    dispute.bump = ctx.bumps.dispute;

    emit!(DisputeOpened {
        dispute: dispute.key(),
        agreement: dispute.agreement,
        marketplace: dispute.marketplace,
        opened_by,
        mode,
    });
    Ok(())
}
