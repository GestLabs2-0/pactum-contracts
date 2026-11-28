use anchor_lang::prelude::*;
use pactum_config::GLOBAL_CONFIG_SEED;
use pactum_core::{constants::MARKETPLACE_SEED, program::PactumCore, state::DisputeParty};
use pactum_escrow::{
    constants::{AGREEMENT_SEED, DATE_RANGE_AGREEMENT_SEED, ESCROW_CONFIG_SEED, ESCROW_VAULT_SEED},
    cpi::accounts::CpiSettleDisputedAgreement,
    cpi::cpi_settle_disputed_agreement,
    program::PactumEscrow,
    state::{Agreement, DateRangeAgreement},
    SettleDisputedParams,
};

use crate::{
    constants::{BPS_DENOMINATOR, CPI_AUTHORITY_SEED},
    error::PactumDisputesError,
    state::{Dispute, DisputeConfig, DisputeOutcome},
};

/// Normalized view of either an `Agreement` or a `DateRangeAgreement`.
pub struct AgreementSnapshot {
    pub key: Pubkey,
    pub marketplace: Pubkey,
    pub client: Pubkey,
    pub provider: Pubkey,
    pub arbiter: Option<Pubkey>,
    pub integrator: Option<Pubkey>,
    pub jury_fee: u64,
    pub remaining_principal: u64,
    pub client_bond: u64,
    pub provider_bond: u64,
    pub rent_payer: Pubkey,
    pub is_reservation: bool,
}

/// Validates that exactly one of `agreement` or `reservation` is provided and extracts its fields.
pub fn extract_agreement_snapshot<'info>(
    agreement: &Option<Box<Account<'info, Agreement>>>,
    reservation: &Option<Box<Account<'info, DateRangeAgreement>>>,
    marketplace_key: Pubkey,
) -> Result<AgreementSnapshot> {
    match (agreement, reservation) {
        (Some(ag), None) => {
            let remaining_principal = ag
                .total_amount
                .checked_sub(ag.released_amount)
                .ok_or(PactumDisputesError::MathOverflow)?;
            Ok(AgreementSnapshot {
                key: ag.key(),
                marketplace: marketplace_key,
                client: ag.client,
                provider: ag.provider,
                arbiter: ag.arbiter,
                integrator: ag.integrator,
                jury_fee: ag.jury_fee,
                remaining_principal,
                client_bond: ag.client_bond,
                provider_bond: ag.provider_bond,
                rent_payer: ag.rent_payer,
                is_reservation: false,
            })
        }
        (None, Some(res)) => Ok(AgreementSnapshot {
            key: res.key(),
            marketplace: res.marketplace,
            client: res.client,
            provider: res.provider,
            arbiter: res.arbiter,
            integrator: res.integrator,
            jury_fee: res.jury_fee,
            remaining_principal: res.total_amount,
            client_bond: res.client_bond,
            provider_bond: res.provider_bond,
            rent_payer: res.rent_payer,
            is_reservation: true,
        }),
        _ => Err(PactumDisputesError::InvalidAgreementTarget.into()),
    }
}

/// Computes `(client_payout, provider_payout, fee_payout, mark_completed)` from a dispute outcome.
pub fn compute_verdict_payouts(
    snapshot: &AgreementSnapshot,
    dispute: &Dispute,
    config: &DisputeConfig,
    outcome: DisputeOutcome,
    base_fee: u64,
) -> Result<(u64, u64, u64, bool)> {
    let fee_cap = snapshot.jury_fee;
    let principal = snapshot.remaining_principal;
    let c_bond = snapshot.client_bond;
    let p_bond = snapshot.provider_bond;
    let j_fee = snapshot.jury_fee;

    match outcome {
        DisputeOutcome::ClientWins => {
            let fee_payout = base_fee.min(fee_cap);
            let loser_refund = j_fee
                .checked_sub(fee_payout)
                .ok_or(PactumDisputesError::MathOverflow)?;
            let client_payout = principal
                .checked_add(c_bond)
                .and_then(|v| v.checked_add(j_fee))
                .ok_or(PactumDisputesError::MathOverflow)?;
            let provider_payout = p_bond
                .checked_add(loser_refund)
                .ok_or(PactumDisputesError::MathOverflow)?;
            Ok((client_payout, provider_payout, fee_payout, false))
        }
        DisputeOutcome::ProviderWins => {
            let fee_payout = base_fee.min(fee_cap);
            let loser_refund = j_fee
                .checked_sub(fee_payout)
                .ok_or(PactumDisputesError::MathOverflow)?;
            let provider_payout = principal
                .checked_add(p_bond)
                .and_then(|v| v.checked_add(j_fee))
                .ok_or(PactumDisputesError::MathOverflow)?;
            let client_payout = c_bond
                .checked_add(loser_refund)
                .ok_or(PactumDisputesError::MathOverflow)?;
            Ok((client_payout, provider_payout, fee_payout, true))
        }
        DisputeOutcome::NoFault => {
            let discounted_fee = u64::try_from(
                (base_fee as u128)
                    .checked_mul(config.no_fault_fee_bps as u128)
                    .ok_or(PactumDisputesError::MathOverflow)?
                    / BPS_DENOMINATOR,
            )
            .map_err(|_| PactumDisputesError::MathOverflow)?;
            let fee_payout = discounted_fee.min(fee_cap);
            let opener_refund = j_fee
                .checked_sub(fee_payout)
                .ok_or(PactumDisputesError::MathOverflow)?;

            let client_principal = principal / 2;
            let provider_principal = principal
                .checked_sub(client_principal)
                .ok_or(PactumDisputesError::MathOverflow)?;

            let (c_fee_refund, p_fee_refund) = match dispute.opened_by {
                DisputeParty::Client => (opener_refund, j_fee),
                DisputeParty::Provider => (j_fee, opener_refund),
            };

            let client_payout = client_principal
                .checked_add(c_bond)
                .and_then(|v| v.checked_add(c_fee_refund))
                .ok_or(PactumDisputesError::MathOverflow)?;
            let provider_payout = provider_principal
                .checked_add(p_bond)
                .and_then(|v| v.checked_add(p_fee_refund))
                .ok_or(PactumDisputesError::MathOverflow)?;

            Ok((client_payout, provider_payout, fee_payout, false))
        }
        DisputeOutcome::P2PAgreement => Err(PactumDisputesError::InvalidDisputeStatus.into()),
    }
}

/// Invokes `pactum_escrow::cpi::cpi_settle_disputed_agreement` using `disputes` CPI authority.
#[allow(clippy::too_many_arguments)]
pub fn invoke_settle_disputed_agreement<'info>(
    escrow_program: &Program<'info, PactumEscrow>,
    escrow_config: &AccountInfo<'info>,
    global_config: &AccountInfo<'info>,
    cpi_authority: &AccountInfo<'info>,
    cpi_authority_bump: u8,
    client: &AccountInfo<'info>,
    provider: &AccountInfo<'info>,
    agreement: Option<AccountInfo<'info>>,
    reservation: Option<AccountInfo<'info>>,
    escrow_vault: &AccountInfo<'info>,
    client_token_account: &AccountInfo<'info>,
    provider_token_account: &AccountInfo<'info>,
    platform_vault: &AccountInfo<'info>,
    fee_destination: Option<AccountInfo<'info>>,
    usdc_mint: &AccountInfo<'info>,
    rent_payer: &AccountInfo<'info>,
    escrow_cpi_authority: &AccountInfo<'info>,
    core_program: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    params: SettleDisputedParams,
) -> Result<()> {
    let seeds: &[&[u8]] = &[CPI_AUTHORITY_SEED.as_bytes(), &[cpi_authority_bump]];
    let signer = &[seeds];

    let cpi_ctx = CpiContext::new_with_signer(
        escrow_program.key(),
        CpiSettleDisputedAgreement {
            escrow_config: escrow_config.clone(),
            global_config: global_config.clone(),
            cpi_authority: cpi_authority.clone(),
            client: client.clone(),
            provider: provider.clone(),
            agreement,
            reservation,
            escrow_vault: escrow_vault.clone(),
            client_token_account: client_token_account.clone(),
            provider_token_account: provider_token_account.clone(),
            platform_vault: platform_vault.clone(),
            fee_destination,
            usdc_mint: usdc_mint.clone(),
            rent_payer: rent_payer.clone(),
            escrow_cpi_authority: escrow_cpi_authority.clone(),
            core_program: core_program.clone(),
            token_program: token_program.clone(),
        },
        signer,
    );
    cpi_settle_disputed_agreement(cpi_ctx, params)
}

/// Helper to record dispute result on `Client` and `Provider` via `pactum-core` CPI.
pub fn invoke_record_dispute_result<'info>(
    core_program: &Program<'info, PactumCore>,
    client: &AccountInfo<'info>,
    provider: &AccountInfo<'info>,
    global_config: &AccountInfo<'info>,
    cpi_authority: &AccountInfo<'info>,
    cpi_authority_bump: u8,
    winner: Option<DisputeParty>,
) -> Result<()> {
    let seeds: &[&[u8]] = &[CPI_AUTHORITY_SEED.as_bytes(), &[cpi_authority_bump]];
    let signer = &[seeds];
    let cpi_ctx = CpiContext::new_with_signer(
        core_program.key(),
        pactum_core::cpi::accounts::CpiRecordDisputeResult {
            client: client.clone(),
            provider: provider.clone(),
            global_config: global_config.clone(),
            cpi_authority: cpi_authority.clone(),
        },
        signer,
    );
    pactum_core::cpi::cpi_record_dispute_result(cpi_ctx, winner)
}

/// Converts `DisputeOutcome` into `Option<DisputeParty>` for `cpi_record_dispute_result`.
pub fn outcome_to_winner(outcome: DisputeOutcome) -> Option<DisputeParty> {
    match outcome {
        DisputeOutcome::ClientWins => Some(DisputeParty::Client),
        DisputeOutcome::ProviderWins => Some(DisputeParty::Provider),
        DisputeOutcome::NoFault | DisputeOutcome::P2PAgreement => None,
    }
}

/// Re-exports of cross-program seed constants used in account constraints.
pub fn _verify_cross_program_seeds_compile() {
    let _ = (
        GLOBAL_CONFIG_SEED,
        MARKETPLACE_SEED,
        ESCROW_CONFIG_SEED,
        ESCROW_VAULT_SEED,
        AGREEMENT_SEED,
        DATE_RANGE_AGREEMENT_SEED,
    );
}
