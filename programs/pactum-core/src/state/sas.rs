use anchor_lang::prelude::*;

pub const ATTESTATION_DISCRIMINATOR: u8 = 2;
#[derive(AnchorDeserialize, AnchorSerialize, Clone, Debug)]
pub struct Attestation {
    /// A pubkey that may either be randomly generated OR associated with a User's wallet
    pub nonce: Pubkey,
    /// Credential this attestation is related to
    pub credential: Pubkey,
    /// Reference to the Schema this Attestation adheres to
    pub schema: Pubkey,
    /// Data that was verified and matches the Schema
    pub data: Vec<u8>,
    /// The pubkey of the signer. Must be one of the `authorized_signer`s at time of attestation
    pub signer: Pubkey,
    /// Designates when the credential is expired. 0 means never expired
    pub expiry: i64,
    /// The pubkey of Attestation token account if created. Otherwise set to default pubkey.
    pub token_account: Pubkey,
}

pub const CREDENTIAL_DISCRIMINATOR: u8 = 0;
#[derive(AnchorDeserialize, AnchorSerialize, Clone, Debug)]
pub struct Credential {
    /// Admin of this credential
    pub authority: Pubkey,
    /// UTF-8 encoded Name of this credential
    /// Includes 4 bytes for length of name
    pub name: Vec<u8>,
    /// List of signers that are allowed to "attest"
    pub authorized_signers: Vec<Pubkey>,
}

/// Validates a SAS `Attestation` and its `Credential` account against the expected program,
/// credential pubkey, and holder wallet (`expected_nonce`), returning `(Attestation, now)`.
pub fn verify_sas_attestation(
    sas_attestation: &AccountInfo,
    sas_credential: &AccountInfo,
    sas_program: Pubkey,
    expected_credential: Pubkey,
    expected_nonce: Pubkey,
) -> Result<(Attestation, i64)> {
    use crate::error::PactumCoreError;

    require!(
        sas_program != Pubkey::default() && sas_attestation.owner == &sas_program,
        PactumCoreError::InvalidSasProgram
    );
    require!(
        sas_credential.owner == &sas_program,
        PactumCoreError::InvalidCredential
    );
    require!(
        sas_credential.key() == expected_credential,
        PactumCoreError::WrongPactumCredential
    );

    let attestation_data = sas_attestation.try_borrow_data()?;
    require!(
        attestation_data.len() > 1 && attestation_data[0] == ATTESTATION_DISCRIMINATOR,
        PactumCoreError::InvalidAttestationAcc
    );

    let credential_data = sas_credential.try_borrow_data()?;
    require!(
        credential_data.len() > 1 && credential_data[0] == CREDENTIAL_DISCRIMINATOR,
        PactumCoreError::InvalidCredentialAcc
    );

    let attestation = Attestation::deserialize(&mut &attestation_data.as_ref()[1..])?;
    let now = Clock::get()?.unix_timestamp;
    require!(
        attestation.expiry == 0 || attestation.expiry >= now,
        PactumCoreError::AttestationExpired
    );
    require!(
        attestation.credential == sas_credential.key(),
        PactumCoreError::WrongPactumCredential
    );
    require!(
        attestation.nonce == expected_nonce,
        PactumCoreError::InvalidAttestationAddress
    );

    let credential = Credential::deserialize(&mut &credential_data.as_ref()[1..])?;
    require!(
        credential.authorized_signers.contains(&attestation.signer),
        PactumCoreError::InvalidAttestationSigner
    );

    Ok((attestation, now))
}
