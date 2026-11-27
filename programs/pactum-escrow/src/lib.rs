#![allow(clippy::diverging_sub_expression)]
pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use anchor_lang::prelude::*;

pub use constants::*;
pub use events::*;
pub use instructions::*;
pub use state::*;

// Must match Anchor.toml [programs.devnet] and pactum_config::ESCROW_PROGRAM_ID.
declare_id!("68ipZiXiUhsaSYSqEM3619vXgKy5CqFmNE6rYzxrXu6a");

#[program]
pub mod pactum_escrow {
    use super::*;

    pub fn initialize_escrow(ctx: Context<InitializeEscrow>) -> Result<()> {
        handler_initialize_escrow(ctx)
    }
}
