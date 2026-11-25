#![allow(clippy::diverging_sub_expression)]
pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use events::*;
pub use instructions::{cpi::*, *};
pub use state::*;

// Must match Anchor.toml [programs.devnet].
declare_id!("HV16vUTaZ78bJP1CyH5KDWyx8NqS1MYSGdPkRsMcnSuY");

#[program]
pub mod pactum_treasury {
    use super::*;

    pub fn initialize_treasury(ctx: Context<InitializeTreasury>) -> Result<()> {
        handler_initialize_treasury(ctx)
    }

    pub fn stake_judge(
        ctx: Context<StakeJudge>,
        amount: u64,
        pool_id: u16,
        leaf_index: u32,
    ) -> Result<()> {
        handler_stake_judge(ctx, amount, pool_id, leaf_index)
    }

    pub fn unstake_judge(ctx: Context<UnstakeJudge>, amount: u64) -> Result<()> {
        handler_unstake_judge(ctx, amount)
    }

    pub fn stake_arbiter(ctx: Context<StakeArbiter>, amount: u64) -> Result<()> {
        handler_stake_arbiter(ctx, amount)
    }

    pub fn unstake_arbiter(ctx: Context<UnstakeArbiter>, amount: u64) -> Result<()> {
        handler_unstake_arbiter(ctx, amount)
    }

    pub fn cpi_slash_stake(
        ctx: Context<CpiSlashStake>,
        target_role: pactum_core::state::ParticipantRole,
        amount: u64,
    ) -> Result<()> {
        handler_cpi_slash_stake(ctx, target_role, amount)
    }
}
