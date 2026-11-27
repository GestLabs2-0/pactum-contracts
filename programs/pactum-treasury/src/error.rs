use anchor_lang::prelude::error_code;

#[error_code]
pub enum PactumTreasuryError {
    #[msg("Unauthorized: caller does not have permission to perform this action")]
    Unauthorized,

    #[msg("Treasury is already initialized")]
    AlreadyInitialized,

    #[msg("Invalid treasury config state")]
    InvalidTreasuryConfig,

    #[msg("The stake vault account does not match the configured vault")]
    InvalidStakeVault,

    #[msg("The token mint does not match the configured USDC mint")]
    InvalidTokenMint,

    #[msg("Amount must be greater than zero")]
    ZeroAmount,

    #[msg("User is globally banned")]
    UserGloballyBanned,

    #[msg("Judge is locally banned in this marketplace")]
    JudgeBanned,

    #[msg("Arbiter profile is not active")]
    ArbiterInactive,

    #[msg("Resulting stake is below the marketplace minimum")]
    StakeBelowMinimum,

    #[msg("Resulting stake exceeds the marketplace maximum")]
    StakeAboveMaximum,

    #[msg("Insufficient staked balance")]
    InsufficientStake,

    #[msg("Cannot unstake while active disputes are assigned")]
    ActiveDisputesExist,

    #[msg("Invalid target role for slashing")]
    InvalidTargetRole,

    #[msg("Missing target role account")]
    MissingTargetAccount,

    #[msg("Target account does not belong to the provided marketplace")]
    InvalidMarketplace,

    #[msg("Arithmetic overflow or underflow")]
    MathOverflow,
}
