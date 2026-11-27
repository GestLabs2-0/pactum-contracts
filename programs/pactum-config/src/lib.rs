#![allow(clippy::diverging_sub_expression)]
pub mod constants;
pub mod cpi_authority;
pub mod error;
pub mod events;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use cpi_authority::*;
pub use events::*;
pub use instructions::*;
pub use state::*;

// Must match Anchor.toml [programs.devnet].
declare_id!("9ESE5Ztpr8zWbLyXCyiB5QqcjxHghotT8zqJxD2S3zaT");

#[program]
pub mod pactum_config {
    use super::*;

    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        params: InitializeConfigParams,
    ) -> Result<()> {
        handler_initialize_config(ctx, params)
    }

    pub fn update_config(ctx: Context<UpdateConfig>, params: UpdateConfigParams) -> Result<()> {
        handler_update_config(ctx, params)
    }

    pub fn withdraw_fees(ctx: Context<WithdrawFees>, amount: u64) -> Result<()> {
        handler_withdraw_fees(ctx, amount)
    }
}
