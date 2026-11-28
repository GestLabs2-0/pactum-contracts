use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    constants::{ARBITER_SEED, CLIENT_SEED, PROVIDER_SEED},
    cpi::accounts::{CpiRecordArbiterResolution, CpiUpdateActiveDisputes},
    cpi::{cpi_record_arbiter_resolution, cpi_update_active_disputes},
    program::PactumCore,
    state::{ArbiterProfile, Client, ParticipantRole, Provider},
};
use pactum_escrow::{
    constants::ESCROW_CONFIG_SEED,
    program::PactumEscrow,
    state::{Agreement, DateRangeAgreement, EscrowConfig},
    SettleDisputedParams,
};

use crate::{
    constants::{CPI_AUTHORITY_SEED, DISPUTE_CONFIG_SEED, DISPUTE_SEED},
    error::PactumDisputesError,
    events::DisputeResolvedByArbiter,
    state::{Dispute, DisputeConfig, DisputeMode, DisputeOutcome, DisputeStatus},
    utils::{
        compute_verdict_payouts, extract_agreement_snapshot, invoke_record_dispute_result,
        invoke_settle_disputed_agreement, outcome_to_winner,
    },
};

#[derive(Accounts)]
pub struct ResolveByArbiter<'info> {
    #[account(
        mut,
        seeds = [DISPUTE_SEED.as_bytes(), dispute.agreement.as_ref()],
        bump = dispute.bump,
        constraint = dispute.status == DisputeStatus::PendingArbiter @ PactumDisputesError::InvalidDisputeStatus,
        constraint = dispute.mode == DisputeMode::Arbiter @ PactumDisputesError::InvalidDisputeMode,
        constraint = dispute.assigned_arbiter == Some(arbiter_authority.key()) @ PactumDisputesError::InvalidArbiter,
    )]
    pub dispute: Box<Account<'info, Dispute>>,

    #[account(
        seeds = [DISPUTE_CONFIG_SEED.as_bytes()],
        bump = dispute_config.bump,
        constraint = dispute_config.is_initialized @ PactumDisputesError::ConfigNotInitialized,
    )]
    pub dispute_config: Box<Account<'info, DisputeConfig>>,

    #[account(
        mut,
        seeds = [ARBITER_SEED.as_bytes(), dispute.marketplace.as_ref(), arbiter_authority.key().as_ref()],
        bump = arbiter_profile.bump,
        seeds::program = pactum_core::ID,
        constraint = arbiter_profile.authority == arbiter_authority.key() @ PactumDisputesError::InvalidArbiter,
        constraint = arbiter_profile.is_active @ PactumDisputesError::ArbiterInactive,
    )]
    pub arbiter_profile: Box<Account<'info, ArbiterProfile>>,

    pub arbiter_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [CLIENT_SEED.as_bytes(), dispute.marketplace.as_ref(), client.authority.as_ref()],
        bump = client.bump,
        seeds::program = pactum_core::ID,
        constraint = client.key() == dispute.client @ PactumDisputesError::InvalidPartyAccount,
    )]
    pub client: Box<Account<'info, Client>>,

    #[account(
        mut,
        seeds = [PROVIDER_SEED.as_bytes(), dispute.marketplace.as_ref(), provider.authority.as_ref()],
        bump = provider.bump,
        seeds::program = pactum_core::ID,
        constraint = provider.key() == dispute.provider @ PactumDisputesError::InvalidPartyAccount,
    )]
    pub provider: Box<Account<'info, Provider>>,

    #[account(mut)]
    pub agreement: Option<Box<Account<'info, Agreement>>>,

    #[account(mut)]
    pub reservation: Option<Box<Account<'info, DateRangeAgreement>>>,

    #[account(mut)]
    pub escrow_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = client_token_account.mint == global_config.usdc_mint @ PactumDisputesError::InvalidTokenAccount,
    )]
    pub client_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = provider_token_account.mint == global_config.usdc_mint @ PactumDisputesError::InvalidTokenAccount,
    )]
    pub provider_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = arbiter_token_account.mint == global_config.usdc_mint @ PactumDisputesError::InvalidTokenAccount,
        constraint = arbiter_token_account.owner == arbiter_authority.key() @ PactumDisputesError::InvalidTokenAccount,
    )]
    pub arbiter_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub platform_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = usdc_mint.key() == global_config.usdc_mint @ PactumDisputesError::InvalidTokenAccount,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Verified in `pactum-escrow` against `agreement.rent_payer`.
    #[account(mut)]
    pub rent_payer: UncheckedAccount<'info>,

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

    /// CHECK: Disputes CPI authority PDA signing CPIs into `pactum-escrow` and `pactum-core`.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    /// CHECK: Escrow CPI authority PDA forwarded to `pactum-escrow`.
    pub escrow_cpi_authority: UncheckedAccount<'info>,

    pub escrow_program: Program<'info, PactumEscrow>,
    pub core_program: Program<'info, PactumCore>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_resolve_by_arbiter(
    ctx: Context<ResolveByArbiter>,
    outcome: DisputeOutcome,
) -> Result<()> {
    require!(
        outcome != DisputeOutcome::P2PAgreement,
        PactumDisputesError::InvalidDisputeStatus
    );

    let snapshot = extract_agreement_snapshot(
        &ctx.accounts.agreement,
        &ctx.accounts.reservation,
        ctx.accounts.dispute.marketplace,
    )?;
    require_keys_eq!(
        snapshot.key,
        ctx.accounts.dispute.agreement,
        PactumDisputesError::InvalidAgreementTarget
    );

    let (client_payout, provider_payout, fee_payout, mark_completed) = compute_verdict_payouts(
        &snapshot,
        &ctx.accounts.dispute,
        &ctx.accounts.dispute_config,
        outcome,
        ctx.accounts.arbiter_profile.arbiter_fee,
    )?;

    invoke_settle_disputed_agreement(
        &ctx.accounts.escrow_program,
        &ctx.accounts.escrow_config.to_account_info(),
        &ctx.accounts.global_config.to_account_info(),
        &ctx.accounts.cpi_authority.to_account_info(),
        ctx.bumps.cpi_authority,
        &ctx.accounts.client.to_account_info(),
        &ctx.accounts.provider.to_account_info(),
        ctx.accounts.agreement.as_ref().map(|a| a.to_account_info()),
        ctx.accounts
            .reservation
            .as_ref()
            .map(|r| r.to_account_info()),
        &ctx.accounts.escrow_vault.to_account_info(),
        &ctx.accounts.client_token_account.to_account_info(),
        &ctx.accounts.provider_token_account.to_account_info(),
        &ctx.accounts.platform_vault.to_account_info(),
        Some(ctx.accounts.arbiter_token_account.to_account_info()),
        &ctx.accounts.usdc_mint.to_account_info(),
        &ctx.accounts.rent_payer.to_account_info(),
        &ctx.accounts.escrow_cpi_authority.to_account_info(),
        &ctx.accounts.core_program.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        SettleDisputedParams {
            restore_previous_status: false,
            client_payout,
            provider_payout,
            fee_payout,
            mark_completed,
        },
    )?;

    invoke_record_dispute_result(
        &ctx.accounts.core_program,
        &ctx.accounts.client.to_account_info(),
        &ctx.accounts.provider.to_account_info(),
        &ctx.accounts.global_config.to_account_info(),
        &ctx.accounts.cpi_authority.to_account_info(),
        ctx.bumps.cpi_authority,
        outcome_to_winner(outcome),
    )?;

    let seeds: &[&[u8]] = &[CPI_AUTHORITY_SEED.as_bytes(), &[ctx.bumps.cpi_authority]];
    let signer = &[seeds];

    let res_ctx = CpiContext::new_with_signer(
        ctx.accounts.core_program.key(),
        CpiRecordArbiterResolution {
            arbiter_profile: ctx.accounts.arbiter_profile.to_account_info(),
            global_config: ctx.accounts.global_config.to_account_info(),
            cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
        },
        signer,
    );
    cpi_record_arbiter_resolution(res_ctx)?;

    let active_ctx = CpiContext::new_with_signer(
        ctx.accounts.core_program.key(),
        CpiUpdateActiveDisputes {
            judge: None,
            arbiter_profile: Some(ctx.accounts.arbiter_profile.to_account_info()),
            global_config: ctx.accounts.global_config.to_account_info(),
            cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
        },
        signer,
    );
    cpi_update_active_disputes(active_ctx, ParticipantRole::Arbiter, -1)?;

    let dispute = &mut ctx.accounts.dispute;
    dispute.status = DisputeStatus::Resolved;
    dispute.outcome = Some(outcome);

    emit!(DisputeResolvedByArbiter {
        dispute: dispute.key(),
        agreement: dispute.agreement,
        arbiter: ctx.accounts.arbiter_authority.key(),
        outcome,
        fee_payout,
    });
    Ok(())
}
