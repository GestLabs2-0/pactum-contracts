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

declare_id!("9g8fxn9YCSq4C4czHpCwn5PNpYmwEXCDN9h8qBehpQrW");

#[program]
pub mod pactum_core {
    use super::*;

    // -------------------------------------------------------------------------
    // Identity, Users & Marketplaces
    // -------------------------------------------------------------------------

    pub fn initialize_core_config(ctx: Context<InitializeCoreConfig>) -> Result<()> {
        handler_initialize_core_config(ctx)
    }

    pub fn init_user_main(ctx: Context<InitUserMain>) -> Result<()> {
        handler_init_user_main(ctx)
    }

    pub fn link_sas_identity(ctx: Context<LinkSasIdentity>, id_hash: [u8; 32]) -> Result<()> {
        handler_link_sas_identity(ctx, id_hash)
    }

    pub fn unlink_sas_identity(ctx: Context<UnlinkSasIdentity>) -> Result<()> {
        handler_unlink_sas_identity(ctx)
    }

    pub fn create_marketplace(
        ctx: Context<CreateMarketplace>,
        market_id: u64,
        params: CreateMarketplaceParams,
    ) -> Result<()> {
        handler_create_marketplace(ctx, market_id, params)
    }

    pub fn update_marketplace(
        ctx: Context<UpdateMarketplace>,
        params: UpdateMarketplaceParams,
    ) -> Result<()> {
        handler_update_marketplace(ctx, params)
    }

    pub fn init_client(ctx: Context<InitClient>) -> Result<()> {
        handler_init_client(ctx)
    }

    pub fn init_provider(ctx: Context<InitProvider>) -> Result<()> {
        handler_init_provider(ctx)
    }

    pub fn init_judge(ctx: Context<InitJudge>) -> Result<()> {
        handler_init_judge(ctx)
    }

    pub fn set_global_ban(ctx: Context<SetGlobalBan>, banned: bool) -> Result<()> {
        handler_set_global_ban(ctx, banned)
    }

    pub fn set_local_ban(
        ctx: Context<SetLocalBan>,
        role: ParticipantRole,
        banned: bool,
    ) -> Result<()> {
        handler_set_local_ban(ctx, role, banned)
    }

    // -------------------------------------------------------------------------
    // Offers
    // -------------------------------------------------------------------------

    pub fn create_offer(ctx: Context<CreateOffer>, params: CreateOfferParams) -> Result<()> {
        handler_create_offer(ctx, params)
    }

    pub fn update_offer(ctx: Context<UpdateOffer>, params: UpdateOfferParams) -> Result<()> {
        handler_update_offer(ctx, params)
    }

    pub fn close_offer(ctx: Context<CloseOffer>) -> Result<()> {
        handler_close_offer(ctx)
    }

    // -------------------------------------------------------------------------
    // B2B Arbiters (ArbiterRegistry)
    // -------------------------------------------------------------------------

    pub fn apply_as_arbiter(
        ctx: Context<ApplyAsArbiter>,
        arbiter_fee: u64,
        sla_secs: i64,
        id_hash: [u8; 32],
    ) -> Result<()> {
        handler_apply_as_arbiter(ctx, arbiter_fee, sla_secs, id_hash)
    }

    pub fn review_arbiter_application(
        ctx: Context<ReviewArbiterApplication>,
        approve: bool,
    ) -> Result<()> {
        handler_review_arbiter_application(ctx, approve)
    }

    pub fn revoke_arbiter(ctx: Context<RevokeArbiter>) -> Result<()> {
        handler_revoke_arbiter(ctx)
    }

    // -------------------------------------------------------------------------
    // Integrators
    // -------------------------------------------------------------------------

    pub fn init_integrator(ctx: Context<InitIntegrator>, default_fee_bps: u16) -> Result<()> {
        handler_init_integrator(ctx, default_fee_bps)
    }

    pub fn apply_as_integrator(
        ctx: Context<ApplyAsIntegrator>,
        default_fee_bps: u16,
        id_hash: Option<[u8; 32]>,
    ) -> Result<()> {
        handler_apply_as_integrator(ctx, default_fee_bps, id_hash)
    }

    pub fn review_integrator_application(
        ctx: Context<ReviewIntegratorApplication>,
        approve: bool,
    ) -> Result<()> {
        handler_review_integrator_application(ctx, approve)
    }

    pub fn revoke_integrator(ctx: Context<RevokeIntegrator>) -> Result<()> {
        handler_revoke_integrator(ctx)
    }

    // -------------------------------------------------------------------------
    // Privileged CPI Mutators (assert_cpi_authority)
    // -------------------------------------------------------------------------

    pub fn cpi_update_judge_stake(
        ctx: Context<CpiUpdateJudgeStake>,
        amount: u64,
        is_stake: bool,
        pool_id: u16,
        leaf_index: u32,
    ) -> Result<()> {
        handler_cpi_update_judge_stake(ctx, amount, is_stake, pool_id, leaf_index)
    }

    pub fn cpi_update_arbiter_stake(
        ctx: Context<CpiUpdateArbiterStake>,
        amount: u64,
        is_stake: bool,
    ) -> Result<()> {
        handler_cpi_update_arbiter_stake(ctx, amount, is_stake)
    }

    pub fn cpi_record_agreement_outcome(
        ctx: Context<CpiRecordAgreementOutcome>,
        role: ParticipantRole,
        outcome: AgreementOutcome,
    ) -> Result<()> {
        handler_cpi_record_agreement_outcome(ctx, role, outcome)
    }

    pub fn cpi_update_role_review(
        ctx: Context<CpiUpdateRoleReview>,
        role: ParticipantRole,
        score: u8,
    ) -> Result<()> {
        handler_cpi_update_role_review(ctx, role, score)
    }

    pub fn cpi_update_active_disputes(
        ctx: Context<CpiUpdateActiveDisputes>,
        target_role: ParticipantRole,
        delta: i8,
    ) -> Result<()> {
        handler_cpi_update_active_disputes(ctx, target_role, delta)
    }

    pub fn cpi_record_dispute_result(
        ctx: Context<CpiRecordDisputeResult>,
        winner: Option<DisputeParty>,
    ) -> Result<()> {
        handler_cpi_record_dispute_result(ctx, winner)
    }

    pub fn cpi_record_judge_vote(
        ctx: Context<CpiRecordJudgeVote>,
        in_consensus: bool,
    ) -> Result<()> {
        handler_cpi_record_judge_vote(ctx, in_consensus)
    }

    pub fn cpi_record_arbiter_resolution(ctx: Context<CpiRecordArbiterResolution>) -> Result<()> {
        handler_cpi_record_arbiter_resolution(ctx)
    }
}
