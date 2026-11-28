use anchor_lang::prelude::*;

#[constant]
pub const DISPUTE_CONFIG_SEED: &str = "dispute_config";

#[constant]
pub const DISPUTE_SEED: &str = "dispute";

#[constant]
pub const VOTE_RECORD_SEED: &str = "vote_record";

#[constant]
pub const JURY_POOL_SEED: &str = "jury_pool";

#[constant]
pub const POOL_LEAF_SEED: &str = "pool_leaf";

#[constant]
pub const CPI_AUTHORITY_SEED: &str = "cpi_authority";

pub const DEFAULT_P2P_WINDOW_SECS: i64 = 172_800;

pub const DEFAULT_VOTE_WINDOW_SECS: i64 = 172_800;

pub const DEFAULT_REVEAL_WINDOW_SECS: i64 = 86_400;

pub const DEFAULT_ARBITER_SLA_SLASH_BPS: u16 = 500;

pub const DEFAULT_NO_FAULT_FEE_BPS: u16 = 5_000;

pub const DEFAULT_JURY_SLASH_BPS: u16 = 500;

pub const BPS_DENOMINATOR: u128 = 10_000;

pub const MAX_BPS: u16 = 10_000;

pub const JURY_PANEL_SIZE: usize = 5;

pub const ROUND_INITIAL: u8 = 1;

pub const ROUND_RUNOFF: u8 = 2;

pub const MIN_JURY_FEE: u64 = 2_000_000;
