use anchor_lang::prelude::*;

#[error_code]
pub enum PactumEscrowError {
    /// Caller does not have permission to perform this action.
    #[msg("Unauthorized: caller does not have permission to perform this action")]
    Unauthorized,

    /// EscrowConfig PDA has not been initialized.
    #[msg("EscrowConfig is not initialized")]
    EscrowNotInitialized,

    /// Referenced marketplace is inactive.
    #[msg("Marketplace is not active")]
    MarketplaceInactive,

    /// Referenced offer is inactive.
    #[msg("Offer is not active")]
    OfferInactive,

    /// Offer pricing_type does not match the requested agreement flow.
    #[msg("Invalid offer pricing type for this instruction")]
    InvalidOfferPricingType,

    /// Offer fields or parameters are invalid for this agreement.
    #[msg("Invalid offer parameters")]
    InvalidOfferParams,

    /// Client or provider is globally or locally banned.
    #[msg("Participant is banned")]
    ParticipantBanned,

    /// Participant does not satisfy marketplace identity requirements.
    #[msg("Identity attestation required by this marketplace")]
    IdentityRequired,

    /// Agreement is not in the required status for this transition.
    #[msg("Invalid agreement status for this operation")]
    InvalidAgreementStatus,

    /// DateRangeAgreement is not in the required status for this transition.
    #[msg("Invalid reservation status for this operation")]
    InvalidDateRangeStatus,

    /// Provided deal_contract_hash does not match the expected hash.
    #[msg("Expected deal contract hash does not match")]
    InvalidContractHash,

    /// Milestone submission deadline has passed.
    #[msg("Milestone deadline has already passed")]
    DeadlinePassed,

    /// Optimistic approval window has not elapsed yet.
    #[msg("Approval window has not elapsed yet")]
    ApprovalWindowNotElapsed,

    /// Acceptance window has not elapsed yet for permissionless expiry.
    #[msg("Accept window has not elapsed yet")]
    AcceptWindowNotElapsed,

    /// Provider acceptance window has expired.
    #[msg("Provider acceptance window has expired")]
    AcceptWindowExpired,

    /// Proposal expiry window has not elapsed yet.
    #[msg("Proposal expiry window has not elapsed yet")]
    ProposalNotExpired,

    /// Check-in or check-out timestamp requirement has not been reached yet.
    #[msg("Reservation timestamp threshold has not been reached yet")]
    TimestampNotReached,

    /// Post-checkout 24h release window has not elapsed yet.
    #[msg("Post-checkout release window has not elapsed yet")]
    ReleaseWindowNotElapsed,

    /// Post-completion 96h review window has not elapsed yet before closing.
    #[msg("Review window has not elapsed yet")]
    ReviewWindowNotElapsed,

    /// Party has already submitted a review for this agreement or reservation.
    #[msg("Counterparty has already been reviewed")]
    AlreadyReviewed,

    /// Review rating must be between 1 and 5 inclusive.
    #[msg("Invalid review rating: must be between 1 and 5")]
    InvalidRating,

    /// Milestone list is empty, exceeds MAX_MILESTONES, or contains zero amounts.
    #[msg("Invalid milestone amounts or step duration")]
    InvalidMilestones,

    /// Agreement cannot be cancelled once milestones have been released.
    #[msg("Cannot cancel agreement after milestones have been released")]
    MilestonesAlreadyStarted,

    /// Check-in and check-out dates are invalid or not in the future.
    #[msg("Invalid check-in or check-out dates")]
    InvalidCheckInOut,

    /// One or more requested dates are already occupied in BookingDays.
    #[msg("Requested dates are already booked")]
    DatesAlreadyBooked,

    /// Dates expected to be occupied in BookingDays were not marked occupied.
    #[msg("Expected dates are not marked as booked")]
    DatesNotBooked,

    /// BookingDays account year does not match the reservation year.
    #[msg("Invalid BookingDays account for reservation year")]
    InvalidBookingDaysAccount,

    /// BookingDays account has not been initialized yet.
    #[msg("BookingDays account is uninitialized")]
    UninitializedBookingDays,

    /// Single-year instruction used for a cross-year reservation.
    #[msg("Single-year reservation requires check-in and check-out in the same year")]
    SingleYearBookingInvalid,

    /// Cross-year instruction used for a non-consecutive or same-year reservation.
    #[msg("Cross-year reservation requires check-out year to be check-in year + 1")]
    CrossYearBookingInvalid,

    /// Month index is outside 1..=12.
    #[msg("Invalid calendar month")]
    InvalidMonth,

    /// Provided token account or mint does not match GlobalConfig.usdc_mint.
    #[msg("Invalid token mint")]
    InvalidTokenMint,

    /// Provided platform vault does not match GlobalConfig.platform_vault.
    #[msg("Invalid platform vault")]
    InvalidPlatformVault,

    /// Provided integrator account does not match the agreement's integrator.
    #[msg("Invalid integrator account")]
    InvalidIntegratorAccount,

    /// Provided rent payer does not match the agreement's rent_payer.
    #[msg("Invalid rent payer account")]
    InvalidRentPayer,

    /// Provided arbiter fee destination does not match the agreement's arbiter.
    #[msg("Invalid arbiter account")]
    InvalidArbiterAccount,

    /// Amount must be strictly positive.
    #[msg("Amount must be greater than zero")]
    ZeroAmount,

    /// Arithmetic overflow or underflow.
    #[msg("Arithmetic overflow or underflow")]
    MathOverflow,

    /// Cannot restore previous status when previous_status is None.
    #[msg("Missing previous status on disputed record")]
    MissingPreviousStatus,

    /// Dispute settlement splits exceed remaining vault balance.
    #[msg("Invalid dispute settlement splits")]
    InvalidSettlementSplit,
}
