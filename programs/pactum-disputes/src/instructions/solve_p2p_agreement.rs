use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    constants::{CLIENT_SEED, PROVIDER_SEED},
    program::PactumCore,
    state::{Client, Provider},
};
use pactum_escrow::{
    constants::ESCROW_CONFIG_SEED,
    program::PactumEscrow,
    state::{Agreement, DateRangeAgreement, EscrowConfig},
    SettleDisputedParams,
};

use crate::{
    constants::{CPI_AUTHORITY_SEED, DISPUTE_SEED},
    error::PactumDisputesError,
    events::P2pAgreementSolved,
    state::{Dispute, DisputeOutcome, DisputeStatus},
    utils::{extract_agreement_snapshot, invoke_settle_disputed_agreement},
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct SolveP2pParams {
    pub restore_previous_status: bool,
    pub client_principal_payout: u64,
    pub provider_principal_payout: u64,
    pub mark_completed: bool,
}

#[derive(Accounts)]
pub struct SolveP2pAgreement<'info> {
    #[account(
        mut,
        seeds = [DISPUTE_SEED.as_bytes(), dispute.agreement.as_ref()],
        bump = dispute.bump,
        constraint = dispute.status == DisputeStatus::OpenP2P @ PactumDisputesError::InvalidDisputeStatus,
    )]
    pub dispute: Box<Account<'info, Dispute>>,

    #[account(
        mut,
        seeds = [CLIENT_SEED.as_bytes(), dispute.marketplace.as_ref(), client_authority.key().as_ref()],
        bump = client.bump,
        seeds::program = pactum_core::ID,
        constraint = client.key() == dispute.client @ PactumDisputesError::InvalidPartyAccount,
    )]
    pub client: Box<Account<'info, Client>>,

    #[account(
        mut,
        seeds = [PROVIDER_SEED.as_bytes(), dispute.marketplace.as_ref(), provider_authority.key().as_ref()],
        bump = provider.bump,
        seeds::program = pactum_core::ID,
        constraint = provider.key() == dispute.provider @ PactumDisputesError::InvalidPartyAccount,
    )]
    pub provider: Box<Account<'info, Provider>>,

    pub client_authority: Signer<'info>,
    pub provider_authority: Signer<'info>,

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

    /// CHECK: Disputes CPI authority PDA signing settle CPI into `pactum-escrow`.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    /// CHECK: Escrow CPI authority PDA forwarded to `pactum-escrow`.
    pub escrow_cpi_authority: UncheckedAccount<'info>,

    pub escrow_program: Program<'info, PactumEscrow>,
    pub core_program: Program<'info, PactumCore>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler_solve_p2p_agreement(
    ctx: Context<SolveP2pAgreement>,
    params: SolveP2pParams,
) -> Result<()> {
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

    let (client_payout, provider_payout, mark_completed) = if params.restore_previous_status {
        (0u64, 0u64, false)
    } else {
        let total_split = params
            .client_principal_payout
            .checked_add(params.provider_principal_payout)
            .ok_or(PactumDisputesError::MathOverflow)?;
        require!(
            total_split == snapshot.remaining_principal,
            PactumDisputesError::InvalidSettlementSplit
        );

        let c_payout = params
            .client_principal_payout
            .checked_add(snapshot.client_bond)
            .and_then(|v| v.checked_add(snapshot.jury_fee))
            .ok_or(PactumDisputesError::MathOverflow)?;
        let p_payout = params
            .provider_principal_payout
            .checked_add(snapshot.provider_bond)
            .and_then(|v| v.checked_add(snapshot.jury_fee))
            .ok_or(PactumDisputesError::MathOverflow)?;
        (c_payout, p_payout, params.mark_completed)
    };

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
            restore_previous_status: params.restore_previous_status,
            client_payout,
            provider_payout,
            fee_payout: 0,
            mark_completed,
        },
    )?;

    let dispute = &mut ctx.accounts.dispute;
    dispute.status = DisputeStatus::Resolved;
    dispute.outcome = Some(DisputeOutcome::P2PAgreement);

    emit!(P2pAgreementSolved {
        dispute: dispute.key(),
        agreement: dispute.agreement,
        restore_previous_status: params.restore_previous_status,
        client_payout,
        provider_payout,
    });
    Ok(())
}
