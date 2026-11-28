use anchor_lang::prelude::*;

#[constant]
pub const ESCROW_CONFIG_SEED: &str = "escrow_config";

#[constant]
pub const ESCROW_VAULT_SEED: &str = "escrow_vault";

#[constant]
pub const AGREEMENT_SEED: &str = "agreement";

#[constant]
pub const DATE_RANGE_AGREEMENT_SEED: &str = "date_range_agreement";

#[constant]
pub const BOOKING_DAYS_SEED: &str = "booking_days";

#[constant]
pub const CPI_AUTHORITY_SEED: &str = "cpi_authority";

pub const MAX_MILESTONES: usize = 20;

pub const BPS_DENOMINATOR: u128 = 10_000;

pub const PERCENTAGE_DENOMINATOR: u128 = 100;

pub const SECONDS_PER_DAY: i64 = 86_400;

pub const SECONDS_PER_HOUR: i64 = 3_600;

pub const ACCEPT_WINDOW_SECS: i64 = 86_400;

pub const PROPOSAL_EXPIRY_SECS: i64 = 604_800;

pub const POST_CHECKOUT_RELEASE_WINDOW_SECS: i64 = 86_400;

pub const REVIEW_WINDOW_SECS: i64 = 345_600;

pub const DEFAULT_CANCELLATION_WINDOW_HOURS: u32 = 72;

pub const DEFAULT_CLIENT_REFUND_PCT: u8 = 60;

pub const MIN_REVIEW_RATING: u8 = 1;

pub const MAX_REVIEW_RATING: u8 = 5;

/// Minimum refundable jury fee reserve (2 USDC = 2_000_000 base units) deposited
/// by each party so disputes can always pay jurors/arbiters without touching principal or bonds.
pub const MIN_JURY_FEE: u64 = 2_000_000;
