use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    constants::{ARBITER_SEED, CLIENT_SEED, MARKETPLACE_SEED, PROVIDER_SEED},
    cpi::accounts::CpiUpdateActiveDisputes,
    cpi::cpi_update_active_disputes,
    program::PactumCore,
    state::{ArbiterProfile, Client, DisputeParty, Marketplace, ParticipantRole, Provider},
};
use pactum_treasury::{
    constants::{ARBITER_VAULT_SEED, TREASURY_CONFIG_SEED},
    cpi::accounts::CpiSlashStake,
    cpi::cpi_slash_stake,
    program::PactumTreasury,
    state::TreasuryConfig,
};

use crate::{
    constants::{BPS_DENOMINATOR, CPI_AUTHORITY_SEED, DISPUTE_CONFIG_SEED, DISPUTE_SEED},
    error::PactumDisputesError,
    events::ArbiterReplacementAccepted,
    state::{Dispute, DisputeConfig, DisputeMode, DisputeStatus},
};

#[derive(Accounts)]
pub struct AcceptArbiterReplacement<'info> {
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

    #[account(
        mut,
        seeds = [ARBITER_SEED.as_bytes(), dispute.marketplace.as_ref(), old_arbiter.authority.as_ref()],
        bump = old_arbiter.bump,
        seeds::program = pactum_core::ID,
        constraint = dispute.assigned_arbiter == Some(old_arbiter.authority) @ PactumDisputesError::InvalidArbiter,
    )]
    pub old_arbiter: Box<Account<'info, ArbiterProfile>>,

    #[account(
        mut,
        seeds = [ARBITER_SEED.as_bytes(), dispute.marketplace.as_ref(), new_arbiter.authority.as_ref()],
        bump = new_arbiter.bump,
        seeds::program = pactum_core::ID,
        constraint = dispute.proposed_replacement_arbiter == Some(new_arbiter.authority) @ PactumDisputesError::NoReplacementProposed,
        constraint = new_arbiter.is_active @ PactumDisputesError::ArbiterInactive,
    )]
    pub new_arbiter: Box<Account<'info, ArbiterProfile>>,

    #[account(
        mut,
        seeds = [ARBITER_VAULT_SEED.as_bytes(), marketplace.key().as_ref()],
        bump,
        seeds::program = pactum_treasury::ID,
    )]
    pub arbiter_stake_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = new_arbiter_token_account.mint == global_config.usdc_mint @ PactumDisputesError::InvalidTokenAccount,
        constraint = new_arbiter_token_account.owner == new_arbiter.authority @ PactumDisputesError::InvalidTokenAccount,
    )]
    pub new_arbiter_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = usdc_mint.key() == global_config.usdc_mint @ PactumDisputesError::InvalidTokenAccount,
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        seeds = [TREASURY_CONFIG_SEED.as_bytes()],
        bump = treasury_config.bump,
        seeds::program = pactum_treasury::ID,
    )]
    pub treasury_config: Box<Account<'info, TreasuryConfig>>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: Disputes CPI authority PDA.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    /// CHECK: Treasury CPI authority PDA forwarded to `pactum-treasury`.
    pub treasury_cpi_authority: UncheckedAccount<'info>,

    pub signer: Signer<'info>,

    pub treasury_program: Program<'info, PactumTreasury>,
    pub core_program: Program<'info, PactumCore>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_accept_arbiter_replacement(ctx: Context<AcceptArbiterReplacement>) -> Result<()> {
    let dispute = &mut ctx.accounts.dispute;
    let proposed_by = dispute
        .proposed_by
        .ok_or(PactumDisputesError::NoReplacementProposed)?;
    let expected_acceptor = match proposed_by {
        DisputeParty::Client => ctx.accounts.provider.authority,
        DisputeParty::Provider => ctx.accounts.client.authority,
    };
    require_keys_eq!(
        ctx.accounts.signer.key(),
        expected_acceptor,
        PactumDisputesError::CounterpartyMustAccept
    );

    let slash_amount = u64::try_from(
        (ctx.accounts.old_arbiter.staked_amount as u128)
            .checked_mul(ctx.accounts.dispute_config.arbiter_sla_slash_bps as u128)
            .ok_or(PactumDisputesError::MathOverflow)?
            / BPS_DENOMINATOR,
    )
    .map_err(|_| PactumDisputesError::MathOverflow)?;

    let seeds: &[&[u8]] = &[CPI_AUTHORITY_SEED.as_bytes(), &[ctx.bumps.cpi_authority]];
    let signer = &[seeds];

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
                recipient_token_account: ctx.accounts.new_arbiter_token_account.to_account_info(),
                usdc_mint: ctx.accounts.usdc_mint.to_account_info(),
                treasury_cpi_authority: ctx.accounts.treasury_cpi_authority.to_account_info(),
                core_program: ctx.accounts.core_program.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            },
            signer,
        );
        cpi_slash_stake(slash_ctx, ParticipantRole::Arbiter, slash_amount)?;
    }

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

    let inc_ctx = CpiContext::new_with_signer(
        ctx.accounts.core_program.key(),
        CpiUpdateActiveDisputes {
            judge: None,
            arbiter_profile: Some(ctx.accounts.new_arbiter.to_account_info()),
            global_config: ctx.accounts.global_config.to_account_info(),
            cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
        },
        signer,
    );
    cpi_update_active_disputes(inc_ctx, ParticipantRole::Arbiter, 1)?;

    let now = Clock::get()?.unix_timestamp;
    let old_arbiter_key = ctx.accounts.old_arbiter.authority;
    let new_arbiter_key = ctx.accounts.new_arbiter.authority;

    dispute.assigned_arbiter = Some(new_arbiter_key);
    dispute.proposed_replacement_arbiter = None;
    dispute.proposed_by = None;
    dispute.phase_started_at = now;
    dispute.sla_deadline = now
        .checked_add(ctx.accounts.new_arbiter.sla_secs)
        .ok_or(PactumDisputesError::MathOverflow)?;

    emit!(ArbiterReplacementAccepted {
        dispute: dispute.key(),
        old_arbiter: old_arbiter_key,
        new_arbiter: new_arbiter_key,
        slash_amount,
    });
    Ok(())
}
