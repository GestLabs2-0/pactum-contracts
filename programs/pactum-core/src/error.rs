use anchor_lang::prelude::*;

#[error_code]
pub enum PactumCoreError {
    #[msg("Unauthorized: caller does not have permission to perform this action")]
    Unauthorized,

    #[msg("User is globally banned")]
    UserGloballyBanned,

    #[msg("Participant is locally banned in this marketplace")]
    ParticipantLocallyBanned,

    #[msg("Identity attestation required by this marketplace")]
    IdentityRequired,

    #[msg("Provided attestation is not owned by the configured SAS program")]
    InvalidSasProgram,

    #[msg("Marketplace is not active")]
    MarketplaceInactive,

    #[msg("Invalid marketplace configuration")]
    InvalidMarketplaceConfig,

    #[msg("Jury mode is not enabled in this marketplace")]
    JuryNotEnabled,

    #[msg("Invalid offer parameters")]
    InvalidOfferParams,

    #[msg("Offer count overflow")]
    OfferCountOverflow,

    #[msg("Invalid arbiter parameters")]
    InvalidArbiterParams,

    #[msg("Arbiter profile is not active")]
    ArbiterInactive,

    #[msg("Invalid review score: must be between 1 and 5")]
    InvalidScore,

    #[msg("Arithmetic overflow or underflow")]
    MathOverflow,

    #[msg("Invalid participant role for this instruction")]
    InvalidRole,

    #[msg("Missing required role account")]
    MissingRoleAccount,

    #[msg("Invalid rent payer account")]
    InvalidRentPayer,
}
