#![allow(clippy::diverging_sub_expression)]
pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use anchor_lang::prelude::*;

pub use constants::*;
pub use error::*;
pub use events::*;
pub use instructions::*;
pub use state::*;

// Must match Anchor.toml [programs.devnet] and pactum_config::DISPUTES_PROGRAM_ID.
declare_id!("8vgDvWkdqhpGBPAczpmZ3DJahVgNN36soRnyw6MbfMCJ");

#[program]
pub mod pactum_disputes {
    use super::*;

    pub fn initialize_dispute_config(
        ctx: Context<InitializeDisputeConfig>,
        params: InitializeDisputeConfigParams,
    ) -> Result<()> {
        handler_initialize_dispute_config(ctx, params)
    }

    pub fn update_dispute_config(
        ctx: Context<UpdateDisputeConfig>,
        params: UpdateDisputeConfigParams,
    ) -> Result<()> {
        handler_update_dispute_config(ctx, params)
    }
}
