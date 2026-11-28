use anchor_lang::prelude::*;
use anchor_spl::token_interface::{TokenAccount, TokenInterface};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    constants::{ARBITER_SEED, CLIENT_SEED, INTEGRATOR_SEED, MARKETPLACE_SEED, PROVIDER_SEED},
    cpi::accounts::CpiUpdateActiveDisputes,
    cpi::cpi_update_active_disputes,
    program::PactumCore,
    state::{
        ArbiterProfile, Client, DisputeMode as MarketDisputeMode, IntegratorProfile, Marketplace,
        ParticipantRole, Provider,
    },
};
use pactum_escrow::{
    program::PactumEscrow,
    state::{Agreement, DateRangeAgreement},
    SettleDisputedParams,
};
use pactum_treasury::{
    cpi::accounts::CpiSlashStake, cpi::cpi_slash_stake, program::PactumTreasury,
};

use crate::{
    constants::{
        BPS_DENOMINATOR, CPI_AUTHORITY_SEED, DISPUTE_CONFIG_SEED, DISPUTE_SEED, ROUND_INITIAL,
    },
    error::PactumDisputesError,
    events::DisputeFallbackToIntegrator,
    state::{Dispute, DisputeConfig, DisputeMode, DisputeOutcome, DisputeStatus},
    utils::{
        compute_verdict_payouts, extract_agreement_snapshot, invoke_record_dispute_result,
        invoke_settle_disputed_agreement, outcome_to_winner,
    },
};

#[derive(Accounts)]
pub struct FallbackToIntegrator<'info> {
    #[account(
        mut,
        seeds = [DISPUTE_SEED.as_bytes(), dispute.agreement.as_ref()],
        bump = dispute.bump,
        constraint = dispute.status == DisputeStatus::PendingArbiter @ PactumDisputesError::InvalidDisputeStatus,
        constraint = dispute.mode == DisputeMode::Arbiter @ PactumDisputesError::InvalidDisputeMode,
    )]
    pub dispute: Box<Account<'info, Dispute>>,

    #[account(
        seeds = [DISPUTE_CONFIG_SEED.as_bytes()],
        bump = dispute_config.bump,
        constraint = dispute_config.is_initialized @ PactumDisputesError::ConfigNotInitialized,
    )]
    pub dispute_config: Box<Account<'info, DisputeConfig>>,

    #[account(
        seeds = [MARKETPLACE_SEED.as_bytes(), marketplace.market_id.to_le_bytes().as_ref()],
        bump = marketplace.bump,
        seeds::program = pactum_core::ID,
        constraint = marketplace.key() == dispute.marketplace @ PactumDisputesError::InvalidPartyAccount,
    )]
    pub marketplace: Box<Account<'info, Marketplace>>,

    #[account(
        mut,
        seeds = [ARBITER_SEED.as_bytes(), dispute.marketplace.as_ref(), old_arbiter.authority.as_ref()],
        bump = old_arbiter.bump,
        seeds::program = pactum_core::ID,
        constraint = dispute.assigned_arbiter == Some(old_arbiter.authority) @ PactumDisputesError::InvalidArbiter,
    )]
    pub old_arbiter: Box<Account<'info, ArbiterProfile>>,

    pub integrator_profile: Option<Box<Account<'info, IntegratorProfile>>>,
    pub integrator_authority: Option<Signer<'info>>,

    #[account(mut)]
    pub integrator_token_account: Option<Box<InterfaceAccount<'info, TokenAccount>>>,

    /// CHECK: Verified by `pactum-treasury` `cpi_slash_stake`.
    #[account(mut)]
    pub arbiter_stake_vault: UncheckedAccount<'info>,

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

    /// CHECK: Verified by `pactum-escrow` `cpi_settle_disputed_agreement`.
    #[account(mut)]
    pub escrow_vault: UncheckedAccount<'info>,

    /// CHECK: Verified by `pactum-escrow` `cpi_settle_disputed_agreement`.
    #[account(mut)]
    pub client_token_account: UncheckedAccount<'info>,

    /// CHECK: Verified by `pactum-escrow` `cpi_settle_disputed_agreement`.
    #[account(mut)]
    pub provider_token_account: UncheckedAccount<'info>,

    /// CHECK: Verified by `pactum-escrow` `cpi_settle_disputed_agreement`.
    #[account(mut)]
    pub platform_vault: UncheckedAccount<'info>,

    /// CHECK: Verified by `pactum-escrow` / `pactum-treasury`.
    pub usdc_mint: UncheckedAccount<'info>,

    /// CHECK: Verified in `pactum-escrow` against `agreement.rent_payer`.
    #[account(mut)]
    pub rent_payer: UncheckedAccount<'info>,

    /// CHECK: Verified by `pactum-escrow` `cpi_settle_disputed_agreement`.
    pub escrow_config: UncheckedAccount<'info>,

    /// CHECK: Verified by `pactum-treasury` `cpi_slash_stake`.
    pub treasury_config: UncheckedAccount<'info>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: Disputes CPI authority PDA.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    /// CHECK: Escrow CPI authority PDA forwarded to `pactum-escrow`.
    pub escrow_cpi_authority: UncheckedAccount<'info>,

    /// CHECK: Treasury CPI authority PDA forwarded to `pactum-treasury`.
    pub treasury_cpi_authority: UncheckedAccount<'info>,

    pub escrow_program: Program<'info, PactumEscrow>,
    pub treasury_program: Program<'info, PactumTreasury>,
    pub core_program: Program<'info, PactumCore>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_fallback_to_integrator(
    ctx: Context<FallbackToIntegrator>,
    outcome: Option<DisputeOutcome>,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        now > ctx.accounts.dispute.sla_deadline || !ctx.accounts.old_arbiter.is_active,
        PactumDisputesError::SlaNotBreached
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

    let seeds: &[&[u8]] = &[CPI_AUTHORITY_SEED.as_bytes(), &[ctx.bumps.cpi_authority]];
    let signer = &[seeds];

    let dec_ctx = CpiContext::new_with_signer(
        ctx.accounts.core_program.key(),
        CpiUpdateActiveDisputes {
            judge: None,
            arbiter_profile: Some(ctx.accounts.old_arbiter.to_account_info()),
            global_config: ctx.accounts.global_config.to_account_info(),
            cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
        },
        signer,
    );
    cpi_update_active_disputes(dec_ctx, ParticipantRole::Arbiter, -1)?;

    let old_arbiter_key = ctx.accounts.old_arbiter.authority;

    if let Some(expected_integrator) = snapshot.integrator {
        let int_profile = ctx
            .accounts
            .integrator_profile
            .as_ref()
            .ok_or(PactumDisputesError::InvalidIntegrator)?;
        let int_auth = ctx
            .accounts
            .integrator_authority
            .as_ref()
            .ok_or(PactumDisputesError::Unauthorized)?;
        let int_ta = ctx
            .accounts
            .integrator_token_account
            .as_ref()
            .ok_or(PactumDisputesError::InvalidTokenAccount)?;

        let (expected_pda, _) = Pubkey::find_program_address(
            &[
                INTEGRATOR_SEED.as_bytes(),
                ctx.accounts.dispute.marketplace.as_ref(),
                int_auth.key().as_ref(),
            ],
            &pactum_core::ID,
        );
        require!(
            int_profile.key() == expected_pda
                && (int_profile.key() == expected_integrator
                    || int_auth.key() == expected_integrator)
                && int_profile.is_active
                && !int_profile.is_banned,
            PactumDisputesError::InvalidIntegrator
        );
        require_keys_eq!(
            int_ta.owner,
            int_auth.key(),
            PactumDisputesError::InvalidTokenAccount
        );
        require_keys_eq!(
            int_ta.mint,
            ctx.accounts.global_config.usdc_mint,
            PactumDisputesError::InvalidTokenAccount
        );

        let chosen_outcome = outcome.ok_or(PactumDisputesError::InvalidDisputeStatus)?;
        require!(
            chosen_outcome != DisputeOutcome::P2PAgreement,
            PactumDisputesError::InvalidDisputeStatus
        );

        let slash_amount = u64::try_from(
            (ctx.accounts.old_arbiter.staked_amount as u128)
                .checked_mul(ctx.accounts.dispute_config.arbiter_sla_slash_bps as u128)
                .ok_or(PactumDisputesError::MathOverflow)?
                / BPS_DENOMINATOR,
        )
        .map_err(|_| PactumDisputesError::MathOverflow)?;

        if slash_amount > 0 {
            let slash_ctx = CpiContext::new_with_signer(
                ctx.accounts.treasury_program.key(),
                CpiSlashStake {
                    cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
                    config: ctx.accounts.treasury_config.to_account_info(),
                    global_config: ctx.accounts.global_config.to_account_info(),
                    marketplace: ctx.accounts.marketplace.to_account_info(),
                    judge: None,
                    arbiter_profile: Some(ctx.accounts.old_arbiter.to_account_info()),
                    stake_vault: ctx.accounts.arbiter_stake_vault.to_account_info(),
                    recipient_token_account: int_ta.to_account_info(),
                    usdc_mint: ctx.accounts.usdc_mint.to_account_info(),
                    treasury_cpi_authority: ctx.accounts.treasury_cpi_authority.to_account_info(),
                    core_program: ctx.accounts.core_program.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                },
                signer,
            );
            cpi_slash_stake(slash_ctx, ParticipantRole::Arbiter, slash_amount)?;
        }

        let (client_payout, provider_payout, fee_payout, mark_completed) = compute_verdict_payouts(
            &snapshot,
            &ctx.accounts.dispute,
            &ctx.accounts.dispute_config,
            chosen_outcome,
            0,
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
            None,
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
            outcome_to_winner(chosen_outcome),
        )?;

        let dispute = &mut ctx.accounts.dispute;
        dispute.status = DisputeStatus::Resolved;
        dispute.outcome = Some(chosen_outcome);

        emit!(DisputeFallbackToIntegrator {
            dispute: dispute.key(),
            old_arbiter: old_arbiter_key,
            integrator: Some(int_auth.key()),
            slash_amount,
            fell_back_to_jury: false,
        });
        return Ok(());
    }

    if ctx.accounts.marketplace.dispute_mode == MarketDisputeMode::Hybrid {
        let dispute = &mut ctx.accounts.dispute;
        dispute.mode = DisputeMode::DecentralizedJury;
        dispute.status = DisputeStatus::VrfPending;
        dispute.assigned_arbiter = None;
        dispute.proposed_replacement_arbiter = None;
        dispute.proposed_by = None;
        dispute.round = ROUND_INITIAL;
        dispute.phase_started_at = now;
        dispute.sla_deadline = 0;

        emit!(DisputeFallbackToIntegrator {
            dispute: dispute.key(),
            old_arbiter: old_arbiter_key,
            integrator: None,
            slash_amount: 0,
            fell_back_to_jury: true,
        });
        return Ok(());
    }

    // Extreme-exit fallback for ArbiterOnly without integrator: 50/50 split with 0 fee.
    let (client_payout, provider_payout, fee_payout, mark_completed) = compute_verdict_payouts(
        &snapshot,
        &ctx.accounts.dispute,
        &ctx.accounts.dispute_config,
        DisputeOutcome::NoFault,
        0,
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
        None,
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
        None,
    )?;

    let dispute = &mut ctx.accounts.dispute;
    dispute.status = DisputeStatus::Resolved;
    dispute.outcome = Some(DisputeOutcome::NoFault);

    emit!(DisputeFallbackToIntegrator {
        dispute: dispute.key(),
        old_arbiter: old_arbiter_key,
        integrator: None,
        slash_amount: 0,
        fell_back_to_jury: false,
    });
    Ok(())
}
