mod common;

use {
    anchor_lang::{
        prelude::{AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::{
        constants::ARBITER_SEED,
        state::{ArbiterProfile, DisputeMode},
    },
    solana_account::Account,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn revoke_arbiter_deactivates_profile() {
    let admin = Keypair::new();
    let committee = Keypair::new();
    let arbiter = Keypair::new();
    let (mut svm, _) = common::setup_svm(Pubkey::new_unique(), admin.pubkey());
    svm.airdrop(&committee.pubkey(), 10_000_000_000).unwrap();

    let market_pda = common::create_marketplace_account(
        &mut svm,
        1,
        committee.pubkey(),
        DisputeMode::ArbiterOnly,
        false,
        &committee,
    );
    let (profile_pda, profile_bump) = Pubkey::find_program_address(
        &[
            ARBITER_SEED.as_bytes(),
            market_pda.as_ref(),
            arbiter.pubkey().as_ref(),
        ],
        &pactum_core::id(),
    );
    let profile = ArbiterProfile {
        marketplace: market_pda,
        authority: arbiter.pubkey(),
        sas_kyb_attestation: Pubkey::new_unique(),
        arbiter_fee: 5_000_000,
        staked_amount: 0,
        sla_secs: 172_800,
        active_disputes: 0,
        cases_resolved: 0,
        is_active: true,
        bump: profile_bump,
    };
    svm.set_account(
        profile_pda,
        Account {
            lamports: 1_000_000_000,
            data: common::serialize_account(&profile),
            owner: pactum_core::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let ix = Instruction::new_with_bytes(
        pactum_core::id(),
        &pactum_core::instruction::RevokeArbiter {}.data(),
        pactum_core::accounts::RevokeArbiter {
            arbiter_profile: profile_pda,
            marketplace: market_pda,
            committee: committee.pubkey(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &committee, &[&committee]).unwrap();

    let after = ArbiterProfile::try_deserialize(
        &mut svm.get_account(&profile_pda).unwrap().data.as_slice(),
    )
    .unwrap();
    assert!(!after.is_active);
}
