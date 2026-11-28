use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    close_account, transfer_checked, CloseAccount, Mint, TokenAccount, TokenInterface,
    TransferChecked,
};

use crate::{
    constants::{
        BPS_DENOMINATOR, CPI_AUTHORITY_SEED, DEFAULT_CANCELLATION_WINDOW_HOURS,
        DEFAULT_CLIENT_REFUND_PCT, ESCROW_VAULT_SEED, PERCENTAGE_DENOMINATOR, SECONDS_PER_DAY,
        SECONDS_PER_HOUR,
    },
    error::PactumEscrowError,
    events::{
        AgreementCancelled, AgreementCompleted, MilestoneReleased, ReservationCancelled,
        ReservationCreated,
    },
    state::{Agreement, AgreementStatus, BookingDays, DateRangeAgreement, DateRangeStatus},
};
use pactum_config::GlobalConfig;
use pactum_core::{
    cpi::{accounts::CpiRecordAgreementOutcome, cpi_record_agreement_outcome},
    program::PactumCore,
    state::{
        AgreementOutcome, CancellationPolicy, Client, IntegratorProfile, Marketplace, Offer,
        ParticipantRole, Provider,
    },
};

/// Refunds `total_amount + client_bond` to `client_token_account`, closes `escrow_vault` to
/// `rent_payer`, and emits `AgreementCancelled`.
#[allow(clippy::too_many_arguments)]
pub fn refund_and_close_pending_agreement<'info>(
    agreement: &Account<'info, Agreement>,
    escrow_vault: &InterfaceAccount<'info, TokenAccount>,
    client_token_account: &InterfaceAccount<'info, TokenAccount>,
    usdc_mint: &InterfaceAccount<'info, Mint>,
    rent_payer: &AccountInfo<'info>,
    token_program: &Interface<'info, TokenInterface>,
    reason: &str,
) -> Result<()> {
    let refund = agreement
        .total_amount
        .checked_add(agreement.client_bond)
        .ok_or(PactumEscrowError::MathOverflow)?;

    transfer_from_vault(
        token_program,
        escrow_vault,
        client_token_account,
        usdc_mint,
        agreement.key(),
        agreement.escrow_vault_bump,
        refund,
    )?;

    close_vault(
        token_program,
        escrow_vault,
        rent_payer,
        agreement.key(),
        agreement.escrow_vault_bump,
    )?;

    emit!(AgreementCancelled {
        agreement: agreement.key(),
        reason: reason.to_string(),
    });
    Ok(())
}

/// Refunds `provider_bond` (if > 0) to `provider_token_account`, closes `escrow_vault` to
/// `rent_payer`, and emits `AgreementCancelled`.
#[allow(clippy::too_many_arguments)]
pub fn refund_and_close_proposed_agreement<'info>(
    agreement: &Account<'info, Agreement>,
    escrow_vault: &InterfaceAccount<'info, TokenAccount>,
    provider_token_account: &InterfaceAccount<'info, TokenAccount>,
    usdc_mint: &InterfaceAccount<'info, Mint>,
    rent_payer: &AccountInfo<'info>,
    token_program: &Interface<'info, TokenInterface>,
    reason: &str,
) -> Result<()> {
    if agreement.provider_bond > 0 {
        transfer_from_vault(
            token_program,
            escrow_vault,
            provider_token_account,
            usdc_mint,
            agreement.key(),
            agreement.escrow_vault_bump,
            agreement.provider_bond,
        )?;
    }

    close_vault(
        token_program,
        escrow_vault,
        rent_payer,
        agreement.key(),
        agreement.escrow_vault_bump,
    )?;

    emit!(AgreementCancelled {
        agreement: agreement.key(),
        reason: reason.to_string(),
    });
    Ok(())
}

/// Transfers `integrator_fee` from `escrow_vault` to `integrator_token_account` when `integrator_fee > 0`.
#[allow(clippy::too_many_arguments)]
pub fn pay_integrator_if_needed<'info>(
    expected_integrator: Option<Pubkey>,
    integrator_fee: u64,
    integrator: &Option<Box<Account<'info, IntegratorProfile>>>,
    integrator_token_account: &Option<Box<InterfaceAccount<'info, TokenAccount>>>,
    escrow_vault: &InterfaceAccount<'info, TokenAccount>,
    usdc_mint: &InterfaceAccount<'info, Mint>,
    token_program: &Interface<'info, TokenInterface>,
    usdc_mint_key: Pubkey,
    agreement_key: Pubkey,
    vault_bump: u8,
) -> Result<()> {
    if integrator_fee == 0 {
        return Ok(());
    }
    let expected_key = expected_integrator.ok_or(PactumEscrowError::InvalidIntegratorAccount)?;
    let int_profile = integrator
        .as_ref()
        .ok_or(PactumEscrowError::InvalidIntegratorAccount)?;
    require!(
        int_profile.key() == expected_key,
        PactumEscrowError::InvalidIntegratorAccount
    );
    let int_ta = integrator_token_account
        .as_ref()
        .ok_or(PactumEscrowError::InvalidIntegratorAccount)?;
    require!(
        int_ta.mint == usdc_mint_key,
        PactumEscrowError::InvalidTokenMint
    );
    require!(
        int_ta.owner == int_profile.authority,
        PactumEscrowError::InvalidIntegratorAccount
    );

    transfer_from_vault(
        token_program,
        escrow_vault,
        int_ta,
        usdc_mint,
        agreement_key,
        vault_bump,
        integrator_fee,
    )
}

/// Records `outcome` for both `Client` and `Provider` via CPI into `pactum-core`.
#[allow(clippy::too_many_arguments)]
pub fn cpi_record_outcomes<'info>(
    core_program: &Program<'info, PactumCore>,
    client: &Account<'info, Client>,
    provider: &Account<'info, Provider>,
    global_config: &Account<'info, GlobalConfig>,
    cpi_authority: &AccountInfo<'info>,
    cpi_authority_bump: u8,
    outcome: AgreementOutcome,
) -> Result<()> {
    let seeds: &[&[u8]] = &[CPI_AUTHORITY_SEED.as_bytes(), &[cpi_authority_bump]];
    let signer = &[seeds];

    let client_ctx = CpiContext::new_with_signer(
        core_program.key(),
        CpiRecordAgreementOutcome {
            client: Some(client.to_account_info()),
            provider: None,
            global_config: global_config.to_account_info(),
            cpi_authority: cpi_authority.clone(),
        },
        signer,
    );
    cpi_record_agreement_outcome(client_ctx, ParticipantRole::Client, outcome)?;

    let provider_ctx = CpiContext::new_with_signer(
        core_program.key(),
        CpiRecordAgreementOutcome {
            client: None,
            provider: Some(provider.to_account_info()),
            global_config: global_config.to_account_info(),
            cpi_authority: cpi_authority.clone(),
        },
        signer,
    );
    cpi_record_agreement_outcome(provider_ctx, ParticipantRole::Provider, outcome)
}

/// Releases the current milestone (used by `approve_milestone` and `claim_milestone`).
#[allow(clippy::too_many_arguments)]
pub fn release_current_milestone<'info>(
    agreement: &mut Account<'info, Agreement>,
    global_config: &Account<'info, GlobalConfig>,
    client: &Account<'info, Client>,
    provider: &Account<'info, Provider>,
    integrator: &Option<Box<Account<'info, IntegratorProfile>>>,
    escrow_vault: &InterfaceAccount<'info, TokenAccount>,
    platform_vault: &InterfaceAccount<'info, TokenAccount>,
    provider_token_account: &InterfaceAccount<'info, TokenAccount>,
    client_token_account: &InterfaceAccount<'info, TokenAccount>,
    integrator_token_account: &Option<Box<InterfaceAccount<'info, TokenAccount>>>,
    usdc_mint: &InterfaceAccount<'info, Mint>,
    rent_payer: &AccountInfo<'info>,
    cpi_authority: &AccountInfo<'info>,
    cpi_authority_bump: u8,
    core_program: &Program<'info, PactumCore>,
    token_program: &Interface<'info, TokenInterface>,
) -> Result<()> {
    let idx = usize::from(agreement.completed_milestones);
    let amount = *agreement
        .milestone_amounts
        .get(idx)
        .ok_or(PactumEscrowError::InvalidMilestones)?;
    let fees = calculate_fees(
        amount,
        global_config.protocol_fee_bps,
        global_config.max_protocol_fee,
        agreement.integrator.is_some(),
        agreement.integrator_fee_bps,
    )?;

    let agreement_key = agreement.key();
    let vault_bump = agreement.escrow_vault_bump;

    transfer_from_vault(
        token_program,
        escrow_vault,
        platform_vault,
        usdc_mint,
        agreement_key,
        vault_bump,
        fees.protocol_fee,
    )?;
    pay_integrator_if_needed(
        agreement.integrator,
        fees.integrator_fee,
        integrator,
        integrator_token_account,
        escrow_vault,
        usdc_mint,
        token_program,
        global_config.usdc_mint,
        agreement_key,
        vault_bump,
    )?;
    transfer_from_vault(
        token_program,
        escrow_vault,
        provider_token_account,
        usdc_mint,
        agreement_key,
        vault_bump,
        fees.provider_payout,
    )?;

    agreement.released_amount = agreement
        .released_amount
        .checked_add(amount)
        .ok_or(PactumEscrowError::MathOverflow)?;
    let released_index = agreement.completed_milestones;
    agreement.completed_milestones = agreement
        .completed_milestones
        .checked_add(1)
        .ok_or(PactumEscrowError::MathOverflow)?;

    emit!(MilestoneReleased {
        agreement: agreement_key,
        milestone_index: released_index,
        amount,
        protocol_fee: fees.protocol_fee,
        integrator_fee: fees.integrator_fee,
        provider_payout: fees.provider_payout,
    });

    if usize::from(agreement.completed_milestones) < agreement.milestone_amounts.len() {
        agreement.status = AgreementStatus::Funded;
        agreement.current_deadline_ts = agreement
            .current_deadline_ts
            .checked_add(agreement.step_duration_secs)
            .ok_or(PactumEscrowError::MathOverflow)?;
        return Ok(());
    }

    // Final milestone settlement: return bonds, close vault, record outcomes.
    transfer_from_vault(
        token_program,
        escrow_vault,
        provider_token_account,
        usdc_mint,
        agreement_key,
        vault_bump,
        agreement.provider_bond,
    )?;
    transfer_from_vault(
        token_program,
        escrow_vault,
        client_token_account,
        usdc_mint,
        agreement_key,
        vault_bump,
        agreement.client_bond,
    )?;
    close_vault(
        token_program,
        escrow_vault,
        rent_payer,
        agreement_key,
        vault_bump,
    )?;

    let now = Clock::get()?.unix_timestamp;
    agreement.status = AgreementStatus::Completed;
    agreement.completed_at = now;

    cpi_record_outcomes(
        core_program,
        client,
        provider,
        global_config,
        cpi_authority,
        cpi_authority_bump,
        AgreementOutcome::Completed,
    )?;

    emit!(AgreementCompleted {
        agreement: agreement_key,
        completed_at: now,
    });
    Ok(())
}

/// Initializes a `DateRangeAgreement` and deposits `total_amount + client_bond` into `escrow_vault`.
#[allow(clippy::too_many_arguments)]
pub fn init_reservation_state<'info>(
    reservation: &mut Account<'info, DateRangeAgreement>,
    marketplace: &Account<'info, Marketplace>,
    offer: &Account<'info, Offer>,
    client: &Account<'info, Client>,
    provider: &Account<'info, Provider>,
    integrator: &Option<Box<Account<'info, IntegratorProfile>>>,
    client_authority: &Signer<'info>,
    payer: &Signer<'info>,
    client_token_account: &InterfaceAccount<'info, TokenAccount>,
    escrow_vault: &InterfaceAccount<'info, TokenAccount>,
    usdc_mint: &InterfaceAccount<'info, Mint>,
    token_program: &Interface<'info, TokenInterface>,
    check_in: i64,
    check_out: i64,
    arbiter: Option<Pubkey>,
    escrow_vault_bump: u8,
    bump: u8,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(check_in > now, PactumEscrowError::InvalidCheckInOut);
    let nights = count_nights(check_in, check_out)?;
    let nightly_rate = offer
        .base_price
        .ok_or(PactumEscrowError::InvalidOfferParams)?;
    require!(nightly_rate > 0, PactumEscrowError::ZeroAmount);
    let total_amount = nights
        .checked_mul(nightly_rate)
        .ok_or(PactumEscrowError::MathOverflow)?;
    let deposit = total_amount
        .checked_add(offer.required_client_bond)
        .ok_or(PactumEscrowError::MathOverflow)?;

    let (integrator_key, integrator_fee_bps) = resolve_integrator(integrator, marketplace.key())?;

    transfer_from_user(
        token_program,
        client_token_account,
        escrow_vault,
        usdc_mint,
        client_authority,
        deposit,
    )?;

    let policy = offer.cancellation_policy.unwrap_or(CancellationPolicy {
        window_hours: DEFAULT_CANCELLATION_WINDOW_HOURS,
        client_refund_pct: DEFAULT_CLIENT_REFUND_PCT,
    });

    reservation.marketplace = marketplace.key();
    reservation.offer = offer.key();
    reservation.client = client.key();
    reservation.provider = provider.key();
    reservation.integrator = integrator_key;
    reservation.integrator_fee_bps = integrator_fee_bps;
    reservation.arbiter = arbiter.or(offer.default_arbiter);
    reservation.jury_fee = offer.jury_fee;
    reservation.check_in = check_in;
    reservation.check_out = check_out;
    reservation.total_amount = total_amount;
    reservation.provider_bond = offer.provider_bond;
    reservation.client_bond = offer.required_client_bond;
    reservation.cancellation_policy = policy;
    reservation.provider_cancellation_penalty_pct = marketplace.provider_cancellation_penalty_pct;
    reservation.state_hash = offer.state_hash;
    reservation.status = DateRangeStatus::Pending;
    reservation.previous_status = None;
    reservation.updated_at = now;
    reservation.client_reviewed = false;
    reservation.provider_reviewed = false;
    reservation.rent_payer = payer.key();
    reservation.escrow_vault_bump = escrow_vault_bump;
    reservation.bump = bump;

    emit!(ReservationCreated {
        reservation: reservation.key(),
        offer: offer.key(),
        client: client.key(),
        provider: provider.key(),
        check_in,
        check_out,
        total_amount,
    });
    Ok(())
}

/// Refunds `total_amount + client_bond` for a `Pending` reservation, closes `escrow_vault`, and marks `Cancelled`.
#[allow(clippy::too_many_arguments)]
pub fn close_pending_reservation<'info>(
    reservation: &mut Account<'info, DateRangeAgreement>,
    escrow_vault: &InterfaceAccount<'info, TokenAccount>,
    client_token_account: &InterfaceAccount<'info, TokenAccount>,
    usdc_mint: &InterfaceAccount<'info, Mint>,
    rent_payer: &AccountInfo<'info>,
    token_program: &Interface<'info, TokenInterface>,
) -> Result<()> {
    let refund = reservation
        .total_amount
        .checked_add(reservation.client_bond)
        .ok_or(PactumEscrowError::MathOverflow)?;
    let key = reservation.key();
    let vault_bump = reservation.escrow_vault_bump;

    transfer_from_vault(
        token_program,
        escrow_vault,
        client_token_account,
        usdc_mint,
        key,
        vault_bump,
        refund,
    )?;
    close_vault(token_program, escrow_vault, rent_payer, key, vault_bump)?;

    reservation.status = DateRangeStatus::Cancelled;
    reservation.updated_at = Clock::get()?.unix_timestamp;

    emit!(ReservationCancelled {
        reservation: key,
        client_refund: refund,
        provider_payout: 0,
        protocol_fee: 0,
        integrator_fee: 0,
    });
    Ok(())
}

/// Executes client-initiated cancellation of an `Accepted` reservation before `check_in`.
#[allow(clippy::too_many_arguments)]
pub fn settle_client_cancel_reservation<'info>(
    reservation: &mut Account<'info, DateRangeAgreement>,
    global_config: &Account<'info, GlobalConfig>,
    client: &Account<'info, Client>,
    provider: &Account<'info, Provider>,
    integrator: &Option<Box<Account<'info, IntegratorProfile>>>,
    escrow_vault: &InterfaceAccount<'info, TokenAccount>,
    platform_vault: &InterfaceAccount<'info, TokenAccount>,
    client_token_account: &InterfaceAccount<'info, TokenAccount>,
    provider_token_account: &InterfaceAccount<'info, TokenAccount>,
    integrator_token_account: &Option<Box<InterfaceAccount<'info, TokenAccount>>>,
    usdc_mint: &InterfaceAccount<'info, Mint>,
    rent_payer: &AccountInfo<'info>,
    cpi_authority: &AccountInfo<'info>,
    cpi_authority_bump: u8,
    core_program: &Program<'info, PactumCore>,
    token_program: &Interface<'info, TokenInterface>,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        now < reservation.check_in,
        PactumEscrowError::DeadlinePassed
    );

    let window_secs = i64::from(reservation.cancellation_policy.window_hours)
        .checked_mul(SECONDS_PER_HOUR)
        .ok_or(PactumEscrowError::MathOverflow)?;
    let time_until_check_in = reservation
        .check_in
        .checked_sub(now)
        .ok_or(PactumEscrowError::MathOverflow)?;
    let inside_window = time_until_check_in <= window_secs;

    let (client_refund, protocol_fee, integrator_fee, provider_payout) = if !inside_window {
        (reservation.total_amount, 0, 0, 0)
    } else {
        let refund_u128 = u128::from(reservation.total_amount)
            .checked_mul(u128::from(
                reservation.cancellation_policy.client_refund_pct,
            ))
            .ok_or(PactumEscrowError::MathOverflow)?
            .checked_div(PERCENTAGE_DENOMINATOR)
            .ok_or(PactumEscrowError::MathOverflow)?;
        let client_refund =
            u64::try_from(refund_u128).map_err(|_| PactumEscrowError::MathOverflow)?;
        let remainder = reservation
            .total_amount
            .checked_sub(client_refund)
            .ok_or(PactumEscrowError::MathOverflow)?;
        let fees = calculate_fees(
            remainder,
            global_config.protocol_fee_bps,
            global_config.max_protocol_fee,
            reservation.integrator.is_some(),
            reservation.integrator_fee_bps,
        )?;
        (
            client_refund,
            fees.protocol_fee,
            fees.integrator_fee,
            fees.provider_payout,
        )
    };

    let key = reservation.key();
    let vault_bump = reservation.escrow_vault_bump;
    let total_client = client_refund
        .checked_add(reservation.client_bond)
        .ok_or(PactumEscrowError::MathOverflow)?;
    let total_provider = provider_payout
        .checked_add(reservation.provider_bond)
        .ok_or(PactumEscrowError::MathOverflow)?;

    transfer_from_vault(
        token_program,
        escrow_vault,
        client_token_account,
        usdc_mint,
        key,
        vault_bump,
        total_client,
    )?;
    transfer_from_vault(
        token_program,
        escrow_vault,
        platform_vault,
        usdc_mint,
        key,
        vault_bump,
        protocol_fee,
    )?;
    pay_integrator_if_needed(
        reservation.integrator,
        integrator_fee,
        integrator,
        integrator_token_account,
        escrow_vault,
        usdc_mint,
        token_program,
        global_config.usdc_mint,
        key,
        vault_bump,
    )?;
    transfer_from_vault(
        token_program,
        escrow_vault,
        provider_token_account,
        usdc_mint,
        key,
        vault_bump,
        total_provider,
    )?;
    close_vault(token_program, escrow_vault, rent_payer, key, vault_bump)?;

    reservation.status = DateRangeStatus::Cancelled;
    reservation.updated_at = now;

    cpi_record_outcomes(
        core_program,
        client,
        provider,
        global_config,
        cpi_authority,
        cpi_authority_bump,
        AgreementOutcome::Cancelled,
    )?;

    emit!(ReservationCancelled {
        reservation: key,
        client_refund,
        provider_payout,
        protocol_fee,
        integrator_fee,
    });
    Ok(())
}

/// Executes provider-initiated cancellation of an `Accepted` reservation before `check_in`.
#[allow(clippy::too_many_arguments)]
pub fn settle_provider_cancel_reservation<'info>(
    reservation: &mut Account<'info, DateRangeAgreement>,
    global_config: &Account<'info, GlobalConfig>,
    client: &Account<'info, Client>,
    provider: &Account<'info, Provider>,
    escrow_vault: &InterfaceAccount<'info, TokenAccount>,
    client_token_account: &InterfaceAccount<'info, TokenAccount>,
    provider_token_account: &InterfaceAccount<'info, TokenAccount>,
    usdc_mint: &InterfaceAccount<'info, Mint>,
    rent_payer: &AccountInfo<'info>,
    cpi_authority: &AccountInfo<'info>,
    cpi_authority_bump: u8,
    core_program: &Program<'info, PactumCore>,
    token_program: &Interface<'info, TokenInterface>,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        now < reservation.check_in,
        PactumEscrowError::DeadlinePassed
    );

    let window_secs = i64::from(reservation.cancellation_policy.window_hours)
        .checked_mul(SECONDS_PER_HOUR)
        .ok_or(PactumEscrowError::MathOverflow)?;
    let time_until_check_in = reservation
        .check_in
        .checked_sub(now)
        .ok_or(PactumEscrowError::MathOverflow)?;
    let inside_window = time_until_check_in <= window_secs;

    let slash = if inside_window && reservation.provider_bond > 0 {
        let slash_u128 = u128::from(reservation.provider_bond)
            .checked_mul(u128::from(reservation.provider_cancellation_penalty_pct))
            .ok_or(PactumEscrowError::MathOverflow)?
            .checked_div(PERCENTAGE_DENOMINATOR)
            .ok_or(PactumEscrowError::MathOverflow)?;
        u64::try_from(slash_u128).map_err(|_| PactumEscrowError::MathOverflow)?
    } else {
        0
    };
    let provider_refund = reservation
        .provider_bond
        .checked_sub(slash)
        .ok_or(PactumEscrowError::MathOverflow)?;
    let total_client = reservation
        .total_amount
        .checked_add(reservation.client_bond)
        .ok_or(PactumEscrowError::MathOverflow)?
        .checked_add(slash)
        .ok_or(PactumEscrowError::MathOverflow)?;

    let key = reservation.key();
    let vault_bump = reservation.escrow_vault_bump;

    transfer_from_vault(
        token_program,
        escrow_vault,
        client_token_account,
        usdc_mint,
        key,
        vault_bump,
        total_client,
    )?;
    transfer_from_vault(
        token_program,
        escrow_vault,
        provider_token_account,
        usdc_mint,
        key,
        vault_bump,
        provider_refund,
    )?;
    close_vault(token_program, escrow_vault, rent_payer, key, vault_bump)?;

    reservation.status = DateRangeStatus::Cancelled;
    reservation.updated_at = now;

    cpi_record_outcomes(
        core_program,
        client,
        provider,
        global_config,
        cpi_authority,
        cpi_authority_bump,
        AgreementOutcome::Cancelled,
    )?;

    emit!(ReservationCancelled {
        reservation: key,
        client_refund: total_client,
        provider_payout: provider_refund,
        protocol_fee: 0,
        integrator_fee: 0,
    });
    Ok(())
}

/// Validates an optional `IntegratorProfile` against `marketplace` and returns `(key, fee_bps)`.
pub fn resolve_integrator(
    integrator: &Option<Box<Account<'_, IntegratorProfile>>>,
    marketplace: Pubkey,
) -> Result<(Option<Pubkey>, u16)> {
    match integrator {
        Some(profile) => {
            require!(
                profile.marketplace == marketplace && profile.is_active && !profile.is_banned,
                PactumEscrowError::InvalidIntegratorAccount
            );
            Ok((Some(profile.key()), profile.default_fee_bps))
        }
        None => Ok((None, 0)),
    }
}

/// Computed fee and payout distribution for an escrow release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeeBreakdown {
    pub protocol_fee: u64,
    pub integrator_fee: u64,
    pub provider_payout: u64,
}

/// Computes `protocol_fee`, `integrator_fee`, and net `provider_payout` from `amount`.
/// `max_protocol_fee == 0` means no cap is applied.
pub fn calculate_fees(
    amount: u64,
    protocol_fee_bps: u16,
    max_protocol_fee: u64,
    has_integrator: bool,
    integrator_fee_bps: u16,
) -> Result<FeeBreakdown> {
    let amount_u128 = u128::from(amount);
    let raw_protocol_fee = amount_u128
        .checked_mul(u128::from(protocol_fee_bps))
        .ok_or(PactumEscrowError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR)
        .ok_or(PactumEscrowError::MathOverflow)?;
    let mut protocol_fee =
        u64::try_from(raw_protocol_fee).map_err(|_| PactumEscrowError::MathOverflow)?;
    if max_protocol_fee > 0 && protocol_fee > max_protocol_fee {
        protocol_fee = max_protocol_fee;
    }

    let integrator_fee = if has_integrator && integrator_fee_bps > 0 {
        let raw_int_fee = amount_u128
            .checked_mul(u128::from(integrator_fee_bps))
            .ok_or(PactumEscrowError::MathOverflow)?
            .checked_div(BPS_DENOMINATOR)
            .ok_or(PactumEscrowError::MathOverflow)?;
        u64::try_from(raw_int_fee).map_err(|_| PactumEscrowError::MathOverflow)?
    } else {
        0
    };

    let total_fees = protocol_fee
        .checked_add(integrator_fee)
        .ok_or(PactumEscrowError::MathOverflow)?;
    let provider_payout = amount
        .checked_sub(total_fees)
        .ok_or(PactumEscrowError::MathOverflow)?;

    Ok(FeeBreakdown {
        protocol_fee,
        integrator_fee,
        provider_payout,
    })
}

/// Transfers `amount` tokens from a user-owned token account into the escrow vault.
pub fn transfer_from_user<'info>(
    token_program: &Interface<'info, TokenInterface>,
    from: &InterfaceAccount<'info, TokenAccount>,
    to: &InterfaceAccount<'info, TokenAccount>,
    mint: &InterfaceAccount<'info, Mint>,
    authority: &Signer<'info>,
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    let cpi_accounts = TransferChecked {
        from: from.to_account_info(),
        mint: mint.to_account_info(),
        to: to.to_account_info(),
        authority: authority.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(token_program.key(), cpi_accounts);
    transfer_checked(cpi_ctx, amount, mint.decimals)
}

/// Transfers `amount` tokens out of an agreement/reservation `EscrowVault` PDA.
#[allow(clippy::too_many_arguments)]
pub fn transfer_from_vault<'info>(
    token_program: &Interface<'info, TokenInterface>,
    vault: &InterfaceAccount<'info, TokenAccount>,
    to: &InterfaceAccount<'info, TokenAccount>,
    mint: &InterfaceAccount<'info, Mint>,
    agreement_key: Pubkey,
    vault_bump: u8,
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    let seeds: &[&[u8]] = &[
        ESCROW_VAULT_SEED.as_bytes(),
        agreement_key.as_ref(),
        &[vault_bump],
    ];
    let signer = &[seeds];
    let cpi_accounts = TransferChecked {
        from: vault.to_account_info(),
        mint: mint.to_account_info(),
        to: to.to_account_info(),
        authority: vault.to_account_info(),
    };
    let cpi_ctx = CpiContext::new_with_signer(token_program.key(), cpi_accounts, signer);
    transfer_checked(cpi_ctx, amount, mint.decimals)
}

/// Closes an `EscrowVault` PDA token account and returns its rent lamports to `destination`.
pub fn close_vault<'info>(
    token_program: &Interface<'info, TokenInterface>,
    vault: &InterfaceAccount<'info, TokenAccount>,
    destination: &AccountInfo<'info>,
    agreement_key: Pubkey,
    vault_bump: u8,
) -> Result<()> {
    let seeds: &[&[u8]] = &[
        ESCROW_VAULT_SEED.as_bytes(),
        agreement_key.as_ref(),
        &[vault_bump],
    ];
    let signer = &[seeds];
    let cpi_accounts = CloseAccount {
        account: vault.to_account_info(),
        destination: destination.clone(),
        authority: vault.to_account_info(),
    };
    let cpi_ctx = CpiContext::new_with_signer(token_program.key(), cpi_accounts, signer);
    close_account(cpi_ctx)
}

#[derive(AnchorSerialize, AnchorDeserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateComponents {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub leap_year: bool,
}

/// Converts a Unix timestamp (seconds since epoch) to Gregorian calendar components
/// using Howard Hinnant's civil date algorithm.
pub fn derive_date(unix_timestamp: i64) -> Result<DateComponents> {
    require!(unix_timestamp >= 0, PactumEscrowError::InvalidCheckInOut);
    let days_since_epoch = u32::try_from(unix_timestamp / SECONDS_PER_DAY)
        .map_err(|_| PactumEscrowError::InvalidCheckInOut)?;

    let z = days_since_epoch + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    let leap_year =
        (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);

    Ok(DateComponents {
        year,
        month,
        day,
        leap_year,
    })
}

pub trait TimestampExt {
    fn to_date(&self) -> Result<DateComponents>;
}

impl TimestampExt for i64 {
    fn to_date(&self) -> Result<DateComponents> {
        derive_date(*self)
    }
}

/// Computes whole nights between `check_in` and `check_out` (must be strictly positive).
pub fn count_nights(check_in: i64, check_out: i64) -> Result<u64> {
    require!(check_out > check_in, PactumEscrowError::InvalidCheckInOut);
    let diff = check_out
        .checked_sub(check_in)
        .ok_or(PactumEscrowError::MathOverflow)?;
    let nights = diff / SECONDS_PER_DAY;
    require!(nights > 0, PactumEscrowError::InvalidCheckInOut);
    u64::try_from(nights).map_err(|_| PactumEscrowError::MathOverflow.into())
}

fn month_days(month: u32, leap_year: bool) -> Result<u32> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Ok(31),
        4 | 6 | 9 | 11 => Ok(30),
        2 => Ok(if leap_year { 29 } else { 28 }),
        _ => err!(PactumEscrowError::InvalidMonth),
    }
}

fn bitmap_days(start_day: u32, end_day: u32) -> u32 {
    let mut mask: u32 = 0;
    for day in start_day..=end_day {
        mask |= 1u32 << (day - 1);
    }
    mask
}

fn apply_month_mask(
    booking_days: &mut Account<'_, BookingDays>,
    month: u32,
    start_day: u32,
    end_day: u32,
    reserve: bool,
) -> Result<()> {
    let idx = usize::try_from(
        month
            .checked_sub(1)
            .ok_or(PactumEscrowError::InvalidMonth)?,
    )
    .map_err(|_| PactumEscrowError::InvalidMonth)?;
    let mask = bitmap_days(start_day, end_day);
    if reserve {
        require!(
            booking_days.occupied_days[idx] & mask == 0,
            PactumEscrowError::DatesAlreadyBooked
        );
        booking_days.occupied_days[idx] |= mask;
    } else {
        require!(
            booking_days.occupied_days[idx] & mask == mask,
            PactumEscrowError::DatesNotBooked
        );
        booking_days.occupied_days[idx] &= !mask;
    }
    Ok(())
}

fn init_booking_days_if_needed(booking_days: &mut Account<'_, BookingDays>, year: u32, bump: u8) {
    if booking_days.year == 0 {
        booking_days.set_inner(BookingDays {
            occupied_days: [0u32; 12],
            year,
            bump,
        });
    }
}

pub fn reserve_days_single_year(
    booking_days: &mut Account<'_, BookingDays>,
    check_in: &DateComponents,
    check_out: &DateComponents,
    bump: u8,
) -> Result<()> {
    init_booking_days_if_needed(booking_days, check_in.year, bump);
    update_days_single_year(booking_days, check_in, check_out, true)
}

pub fn release_days_single_year(
    booking_days: &mut Account<'_, BookingDays>,
    check_in: &DateComponents,
    check_out: &DateComponents,
) -> Result<()> {
    require!(
        booking_days.year > 0,
        PactumEscrowError::UninitializedBookingDays
    );
    update_days_single_year(booking_days, check_in, check_out, false)
}

fn update_days_single_year(
    booking_days: &mut Account<'_, BookingDays>,
    check_in: &DateComponents,
    check_out: &DateComponents,
    reserve: bool,
) -> Result<()> {
    require!(
        check_out.year == check_in.year,
        PactumEscrowError::SingleYearBookingInvalid
    );
    require!(
        booking_days.year == check_in.year,
        PactumEscrowError::InvalidBookingDaysAccount
    );

    for month in check_in.month..=check_out.month {
        let start_day = if month == check_in.month {
            check_in.day
        } else {
            1
        };
        let end_day = if month == check_out.month {
            check_out.day
        } else {
            month_days(month, check_in.leap_year)?
        };
        apply_month_mask(booking_days, month, start_day, end_day, reserve)?;
    }

    Ok(())
}

pub fn reserve_days_cross_years(
    booking_days: &mut Account<'_, BookingDays>,
    booking_days_next: &mut Account<'_, BookingDays>,
    check_in: &DateComponents,
    check_out: &DateComponents,
    bump: u8,
    bump_next: u8,
) -> Result<()> {
    init_booking_days_if_needed(booking_days, check_in.year, bump);
    init_booking_days_if_needed(booking_days_next, check_out.year, bump_next);
    update_days_cross_years(booking_days, booking_days_next, check_in, check_out, true)
}

pub fn release_days_cross_years(
    booking_days: &mut Account<'_, BookingDays>,
    booking_days_next: &mut Account<'_, BookingDays>,
    check_in: &DateComponents,
    check_out: &DateComponents,
) -> Result<()> {
    require!(
        booking_days.year > 0 && booking_days_next.year > 0,
        PactumEscrowError::UninitializedBookingDays
    );
    update_days_cross_years(booking_days, booking_days_next, check_in, check_out, false)
}

fn update_days_cross_years(
    booking_days: &mut Account<'_, BookingDays>,
    booking_days_next: &mut Account<'_, BookingDays>,
    check_in: &DateComponents,
    check_out: &DateComponents,
    reserve: bool,
) -> Result<()> {
    require!(
        check_in.year + 1 == check_out.year,
        PactumEscrowError::CrossYearBookingInvalid
    );
    require!(
        booking_days.year == check_in.year && booking_days_next.year == check_out.year,
        PactumEscrowError::InvalidBookingDaysAccount
    );

    for month in check_in.month..=12 {
        let start_day = if month == check_in.month {
            check_in.day
        } else {
            1
        };
        let end_day = month_days(month, check_in.leap_year)?;
        apply_month_mask(booking_days, month, start_day, end_day, reserve)?;
    }

    for month in 1..=check_out.month {
        let end_day = if month == check_out.month {
            check_out.day
        } else {
            month_days(month, check_out.leap_year)?
        };
        apply_month_mask(booking_days_next, month, 1, end_day, reserve)?;
    }

    Ok(())
}
