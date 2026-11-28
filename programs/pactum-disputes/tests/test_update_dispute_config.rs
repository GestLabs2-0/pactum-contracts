mod common;

use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    common::{get_dispute_config, send_ix, setup_svm},
    pactum_disputes::UpdateDisputeConfigParams,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn update_dispute_config_updates_fields_for_authority() {
    let mut env = setup_svm(true);

    let ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::UpdateDisputeConfig {
            params: UpdateDisputeConfigParams {
                p2p_window_secs: Some(3_600),
                vote_window_secs: None,
                reveal_window_secs: Some(7_200),
                arbiter_sla_slash_bps: Some(1_000),
                no_fault_fee_bps: None,
            },
        }
        .data(),
        pactum_disputes::accounts::UpdateDisputeConfig {
            dispute_config: env.dispute_config,
            global_config: env.global_config,
            authority: env.authority.pubkey(),
        }
        .to_account_metas(None),
    );
    send_ix(&mut env.svm, ix, &env.authority, &[&env.authority]).unwrap();

    let cfg = get_dispute_config(&env.svm, &env.dispute_config);
    assert_eq!(cfg.p2p_window_secs, 3_600);
    assert_eq!(cfg.vote_window_secs, 172_800);
    assert_eq!(cfg.reveal_window_secs, 7_200);
    assert_eq!(cfg.arbiter_sla_slash_bps, 1_000);
    assert_eq!(cfg.no_fault_fee_bps, 5_000);
}

#[test]
fn update_dispute_config_rejects_unauthorized_signer() {
    let mut env = setup_svm(true);
    let attacker = Keypair::new();
    env.svm.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();

    let ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::UpdateDisputeConfig {
            params: UpdateDisputeConfigParams {
                p2p_window_secs: Some(3_600),
                vote_window_secs: None,
                reveal_window_secs: None,
                arbiter_sla_slash_bps: None,
                no_fault_fee_bps: None,
            },
        }
        .data(),
        pactum_disputes::accounts::UpdateDisputeConfig {
            dispute_config: env.dispute_config,
            global_config: env.global_config,
            authority: attacker.pubkey(),
        }
        .to_account_metas(None),
    );
    assert!(send_ix(&mut env.svm, ix, &attacker, &[&attacker]).is_err());
}
