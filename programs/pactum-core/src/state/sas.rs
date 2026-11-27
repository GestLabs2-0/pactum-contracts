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

/// Verifies a SAS attestation against `expected_credential`, hashes its `data` on-chain to
/// derive `Identity` (`["identity", sha256(data)]`), and creates or validates ownership.
#[allow(clippy::too_many_arguments)]
pub fn verify_and_anchor_identity<'info>(
    sas_attestation: &AccountInfo<'info>,
    sas_credential: &AccountInfo<'info>,
    identity_acc: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    sas_program: Pubkey,
    expected_credential: Pubkey,
    authority: Pubkey,
    allow_existing: bool,
) -> Result<()> {
    use anchor_lang::system_program::{create_account, CreateAccount};
    use solana_sha256_hasher::hash;

    use crate::{constants::IDENTITY_SEED, error::PactumCoreError, state::Identity};

    let (attestation, now) = verify_sas_attestation(
        sas_attestation,
        sas_credential,
        sas_program,
        expected_credential,
        authority,
    )?;

    require!(
        !attestation.data.is_empty(),
        PactumCoreError::InvalidAttestationData
    );

    let id_hash = hash(&attestation.data).to_bytes();
    let (expected_identity, identity_bump) =
        Pubkey::find_program_address(&[IDENTITY_SEED.as_bytes(), id_hash.as_ref()], &crate::ID);
    require_keys_eq!(
        identity_acc.key(),
        expected_identity,
        PactumCoreError::InvalidAttestationData
    );

    if identity_acc.data_is_empty() {
        let space = 8 + Identity::INIT_SPACE;
        let lamports = Rent::get()?.minimum_balance(space);
        let signer_seeds: &[&[&[u8]]] =
            &[&[IDENTITY_SEED.as_bytes(), id_hash.as_ref(), &[identity_bump]]];
        let cpi_ctx = CpiContext::new_with_signer(
            system_program.key(),
            CreateAccount {
                from: payer.clone(),
                to: identity_acc.clone(),
            },
            signer_seeds,
        );
        create_account(cpi_ctx, lamports, space as u64, &crate::ID)?;

        let identity = Identity {
            authority,
            verified_at: now,
            linked: true,
            bump: identity_bump,
        };
        let mut data = identity_acc.try_borrow_mut_data()?;
        let mut writer: &mut [u8] = &mut data;
        identity.try_serialize(&mut writer)?;
    } else {
        require!(allow_existing, PactumCoreError::IdentityAlreadyClaimed);
        let data = identity_acc.try_borrow_data()?;
        let identity = Identity::try_deserialize(&mut &data[..])?;
        require!(
            identity.authority == authority,
            PactumCoreError::IdentityAlreadyClaimed
        );
    }

    Ok(())
}
