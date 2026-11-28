use anchor_lang::prelude::*;

#[error_code]
pub enum PactumDisputesError {
    /// Signer or account is not authorized to perform this action.
    #[msg("Unauthorized")]
    Unauthorized,

    /// DisputeConfig has already been initialized.
    #[msg("Dispute config is already initialized")]
    AlreadyInitialized,

    /// DisputeConfig has not been initialized yet.
    #[msg("Dispute config is not initialized")]
    ConfigNotInitialized,

    /// Supplied configuration parameters (windows or bps) are invalid.
    #[msg("Invalid dispute configuration parameter")]
    InvalidConfig,

    /// Arithmetic overflow or underflow occurred.
    #[msg("Math overflow")]
    MathOverflow,

    /// Agreement or DateRangeAgreement is not in a disputable or disputed state.
    #[msg("Invalid agreement status")]
    InvalidAgreementStatus,

    /// Both or neither of `agreement` and `reservation` were provided.
    #[msg("Provide exactly one of agreement or reservation")]
    InvalidAgreementTarget,

    /// Dispute is not in the required status for this instruction.
    #[msg("Invalid dispute status")]
    InvalidDisputeStatus,

    /// Dispute resolution mode does not match this instruction.
    #[msg("Invalid dispute mode")]
    InvalidDisputeMode,

    /// Marketplace dispute mode and agreement arbiter configuration are incompatible.
    #[msg("Incompatible marketplace dispute mode and agreement arbiter")]
    IncompatibleDisputeMode,

    /// Evidence has already been submitted by this party.
    #[msg("Evidence already submitted by this party")]
    EvidenceAlreadySubmitted,

    /// P2P settlement split does not match the agreement's escrowed funds.
    #[msg("Invalid P2P settlement split")]
    InvalidSettlementSplit,

    /// P2P negotiation window has not elapsed yet for unilateral escalation.
    #[msg("P2P window has not elapsed")]
    P2pWindowNotElapsed,

    /// Assigned arbiter account is missing or does not match `Dispute.assigned_arbiter`.
    #[msg("Invalid or missing assigned arbiter")]
    InvalidArbiter,

    /// Arbiter profile is inactive or revoked.
    #[msg("Arbiter is inactive")]
    ArbiterInactive,

    /// Arbiter SLA deadline has not elapsed and arbiter is still active.
    #[msg("Arbiter SLA has not been breached")]
    SlaNotBreached,

    /// No replacement arbiter has been proposed.
    #[msg("No replacement arbiter proposed")]
    NoReplacementProposed,

    /// Proposing party cannot accept their own replacement arbiter proposal.
    #[msg("Counterparty must accept the replacement proposal")]
    CounterpartyMustAccept,

    /// Integrator profile is missing, inactive, banned, or does not match the agreement.
    #[msg("Invalid integrator fallback account")]
    InvalidIntegrator,

    /// Jury reveal window has not elapsed yet.
    #[msg("Reveal window has not elapsed")]
    RevealWindowNotElapsed,

    /// Invalid jury panel size or missing vote/leaf accounts.
    #[msg("Invalid jury accounts")]
    InvalidJuryAccounts,

    /// VoteRecord or PoolLeaf does not match the expected dispute, round, or leaf index.
    #[msg("Vote record or pool leaf mismatch")]
    VoteRecordMismatch,

    /// Provided rent payer does not match the stored `rent_payer`.
    #[msg("Invalid rent payer")]
    InvalidRentPayer,

    /// Provided token account or mint does not match expected USDC mint or owner.
    #[msg("Invalid token account or mint")]
    InvalidTokenAccount,

    /// Provided `Client` or `Provider` account does not match the dispute.
    #[msg("Invalid party profile account")]
    InvalidPartyAccount,
}
