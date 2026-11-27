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

    pub fn create_fixed_agreement(
        ctx: Context<CreateFixedAgreement>,
        params: CreateFixedAgreementParams,
    ) -> Result<()> {
        handler_create_fixed_agreement(ctx, params)
    }

    pub fn provider_accept_agreement(
        ctx: Context<ProviderAcceptAgreement>,
        expected_contract_hash: Option<[u8; 32]>,
    ) -> Result<()> {
        handler_provider_accept_agreement(ctx, expected_contract_hash)
    }

    pub fn provider_reject_agreement(ctx: Context<ProviderRejectAgreement>) -> Result<()> {
        handler_provider_reject_agreement(ctx)
    }

    pub fn expire_agreement(ctx: Context<ExpireAgreement>) -> Result<()> {
        handler_expire_agreement(ctx)
    }

    pub fn propose_agreement(
        ctx: Context<ProposeAgreement>,
        params: ProposeAgreementParams,
    ) -> Result<()> {
        handler_propose_agreement(ctx, params)
    }

    pub fn fund_agreement(
        ctx: Context<FundAgreement>,
        expected_contract_hash: Option<[u8; 32]>,
    ) -> Result<()> {
        handler_fund_agreement(ctx, expected_contract_hash)
    }

    pub fn cancel_proposal(ctx: Context<CancelProposal>) -> Result<()> {
        handler_cancel_proposal(ctx)
    }

    pub fn expire_proposal(ctx: Context<ExpireProposal>) -> Result<()> {
        handler_expire_proposal(ctx)
    }

    pub fn submit_milestone(
        ctx: Context<SubmitMilestone>,
        deliverable_hash: [u8; 32],
    ) -> Result<()> {
        handler_submit_milestone(ctx, deliverable_hash)
    }

    pub fn approve_milestone(ctx: Context<ApproveMilestone>) -> Result<()> {
        handler_approve_milestone(ctx)
    }

    pub fn claim_milestone(ctx: Context<ClaimMilestone>) -> Result<()> {
        handler_claim_milestone(ctx)
    }
}
