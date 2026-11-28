use anchor_lang::prelude::*;
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    constants::{ARBITER_SEED, CLIENT_SEED, PROVIDER_SEED},
    cpi::accounts::CpiUpdateActiveDisputes,
    cpi::cpi_update_active_disputes,
    program::PactumCore,
    state::{ArbiterProfile, Client, ParticipantRole, Provider},
};

use crate::{
    constants::{CPI_AUTHORITY_SEED, DISPUTE_CONFIG_SEED, DISPUTE_SEED, ROUND_INITIAL},
    error::PactumDisputesError,
    events::DisputeEscalated,
    state::{Dispute, DisputeConfig, DisputeMode, DisputeStatus},
};

#[derive(Accounts)]
pub struct EscalateDispute<'info> {
    #[account(
        mut,
        seeds = [DISPUTE_SEED.as_bytes(), dispute.agreement.as_ref()],
        bump = dispute.bump,
        constraint = dispute.status == DisputeStatus::OpenP2P @ PactumDisputesError::InvalidDisputeStatus,
    )]
    pub dispute: Box<Account<'info, Dispute>>,

    #[account(
        seeds = [DISPUTE_CONFIG_SEED.as_bytes()],
        bump = dispute_config.bump,
        constraint = dispute_config.is_initialized @ PactumDisputesError::ConfigNotInitialized,
    )]
    pub dispute_config: Box<Account<'info, DisputeConfig>>,

    #[account(
        seeds = [CLIENT_SEED.as_bytes(), dispute.marketplace.as_ref(), client.authority.as_ref()],
        bump = client.bump,
        seeds::program = pactum_core::ID,
        constraint = client.key() == dispute.client @ PactumDisputesError::InvalidPartyAccount,
    )]
    pub client: Box<Account<'info, Client>>,

    #[account(
        seeds = [PROVIDER_SEED.as_bytes(), dispute.marketplace.as_ref(), provider.authority.as_ref()],
        bump = provider.bump,
        seeds::program = pactum_core::ID,
        constraint = provider.key() == dispute.provider @ PactumDisputesError::InvalidPartyAccount,
    )]
    pub provider: Box<Account<'info, Provider>>,

    #[account(mut)]
    pub arbiter_profile: Option<Box<Account<'info, ArbiterProfile>>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: Disputes CPI authority PDA signing `cpi_update_active_disputes`.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    pub signer: Signer<'info>,
    pub counterparty_signer: Option<Signer<'info>>,

    pub core_program: Program<'info, PactumCore>,
}

pub fn handler_escalate_dispute(ctx: Context<EscalateDispute>) -> Result<()> {
    let dispute = &mut ctx.accounts.dispute;
    let signer_key = ctx.accounts.signer.key();
    let client_auth = ctx.accounts.client.authority;
    let provider_auth = ctx.accounts.provider.authority;

    let is_bilateral = if signer_key == client_auth {
        ctx.accounts
            .counterparty_signer
            .as_ref()
            .is_some_and(|s| s.key() == provider_auth)
    } else if signer_key == provider_auth {
        ctx.accounts
            .counterparty_signer
            .as_ref()
            .is_some_and(|s| s.key() == client_auth)
    } else {
        return err!(PactumDisputesError::Unauthorized);
    };

    let now = Clock::get()?.unix_timestamp;
    if !is_bilateral {
        let min_escalate_ts = dispute
            .phase_started_at
            .checked_add(ctx.accounts.dispute_config.p2p_window_secs)
            .ok_or(PactumDisputesError::MathOverflow)?;
        require!(
            now >= min_escalate_ts,
            PactumDisputesError::P2pWindowNotElapsed
        );
    }

    match dispute.mode {
        DisputeMode::Arbiter => {
            let expected_arbiter = dispute
                .assigned_arbiter
                .ok_or(PactumDisputesError::InvalidArbiter)?;
            let arbiter = ctx
                .accounts
                .arbiter_profile
                .as_ref()
                .ok_or(PactumDisputesError::InvalidArbiter)?;
            let (expected_pda, _) = Pubkey::find_program_address(
                &[
                    ARBITER_SEED.as_bytes(),
                    dispute.marketplace.as_ref(),
                    expected_arbiter.as_ref(),
                ],
                &pactum_core::ID,
            );
            require!(
                arbiter.key() == expected_pda || arbiter.key() == expected_arbiter,
                PactumDisputesError::InvalidArbiter
            );
            require!(arbiter.is_active, PactumDisputesError::ArbiterInactive);

            let sla_deadline = now
                .checked_add(arbiter.sla_secs)
                .ok_or(PactumDisputesError::MathOverflow)?;

            let seeds: &[&[u8]] = &[CPI_AUTHORITY_SEED.as_bytes(), &[ctx.bumps.cpi_authority]];
            let signer = &[seeds];
            let cpi_ctx = CpiContext::new_with_signer(
                ctx.accounts.core_program.key(),
                CpiUpdateActiveDisputes {
                    judge: None,
                    arbiter_profile: Some(arbiter.to_account_info()),
                    global_config: ctx.accounts.global_config.to_account_info(),
                    cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
                },
                signer,
            );
            cpi_update_active_disputes(cpi_ctx, ParticipantRole::Arbiter, 1)?;

            dispute.assigned_arbiter = Some(arbiter.authority);
            dispute.status = DisputeStatus::PendingArbiter;
            dispute.phase_started_at = now;
            dispute.sla_deadline = sla_deadline;
        }
        DisputeMode::DecentralizedJury => {
            dispute.status = DisputeStatus::VrfPending;
            dispute.round = ROUND_INITIAL;
            dispute.phase_started_at = now;
            dispute.sla_deadline = 0;
        }
    }

    emit!(DisputeEscalated {
        dispute: dispute.key(),
        agreement: dispute.agreement,
        mode: dispute.mode,
        sla_deadline: dispute.sla_deadline,
    });
    Ok(())
}
