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

    pub fn open_dispute(ctx: Context<OpenDispute>) -> Result<()> {
        handler_open_dispute(ctx)
    }

    pub fn link_evidence(ctx: Context<LinkEvidence>, evidence_hash: [u8; 32]) -> Result<()> {
        handler_link_evidence(ctx, evidence_hash)
    }

    pub fn solve_p2p_agreement(
        ctx: Context<SolveP2pAgreement>,
        params: SolveP2pParams,
    ) -> Result<()> {
        handler_solve_p2p_agreement(ctx, params)
    }

    pub fn escalate_dispute(ctx: Context<EscalateDispute>) -> Result<()> {
        handler_escalate_dispute(ctx)
    }

    pub fn resolve_by_arbiter(
        ctx: Context<ResolveByArbiter>,
        outcome: DisputeOutcome,
    ) -> Result<()> {
        handler_resolve_by_arbiter(ctx, outcome)
    }

    pub fn propose_arbiter_replacement(ctx: Context<ProposeArbiterReplacement>) -> Result<()> {
        handler_propose_arbiter_replacement(ctx)
    }

    pub fn accept_arbiter_replacement(ctx: Context<AcceptArbiterReplacement>) -> Result<()> {
        handler_accept_arbiter_replacement(ctx)
    }

    pub fn fallback_to_integrator(
        ctx: Context<FallbackToIntegrator>,
        outcome: Option<DisputeOutcome>,
    ) -> Result<()> {
        handler_fallback_to_integrator(ctx, outcome)
    }
}
