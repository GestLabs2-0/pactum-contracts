use anchor_lang::prelude::*;

#[error_code]
pub enum PactumConfigError {
    #[msg("Invalid protocol fee basis points; must be less than 10000")]
    InvalidFeeBps,

    #[msg("Invalid global config state")]
    InvalidGlobalConfig,

    #[msg("The platform vault account does not match the configured vault")]
    InvalidVaultAccount,

    #[msg("The token mint does not match the configured USDC mint")]
    InvalidTokenMint,

    #[msg("Unauthorized: only the configured authority can perform this action")]
    Unauthorized,

    #[msg("Withdrawal amount must be greater than zero")]
    ZeroAmount,

    #[msg("Authority pubkey cannot be the default pubkey")]
    InvalidAuthority,
}
