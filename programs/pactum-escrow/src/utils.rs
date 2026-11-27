use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    close_account, transfer_checked, CloseAccount, Mint, TokenAccount, TokenInterface,
    TransferChecked,
};

use crate::{
    constants::{BPS_DENOMINATOR, ESCROW_VAULT_SEED, SECONDS_PER_DAY},
    error::PactumEscrowError,
    events::AgreementCancelled,
    state::{Agreement, BookingDays},
};
use pactum_core::state::IntegratorProfile;

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
