mod common;

use {
    anchor_lang::prelude::AccountDeserialize, pactum_treasury::state::TreasuryConfig,
    solana_keypair::Keypair, solana_signer::Signer,
};

#[test]
fn initialize_treasury_persists_config_and_stake_vault() {
    let authority = Keypair::new();
    let (svm, global_config_pda, treasury_config_pda, _, stake_vault_pda, _) =
        common::setup_svm(&authority);

    let acc = svm.get_account(&treasury_config_pda).unwrap();
    let cfg = TreasuryConfig::try_deserialize(&mut acc.data.as_slice()).unwrap();

    assert_eq!(cfg.authority, authority.pubkey());
    assert_eq!(cfg.global_config, global_config_pda);
    assert_eq!(cfg.stake_vault, stake_vault_pda);
    assert!(cfg.is_initialized);
}
