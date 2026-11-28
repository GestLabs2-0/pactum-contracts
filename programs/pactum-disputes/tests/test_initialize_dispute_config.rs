mod common;

use {
    anchor_lang::{
        prelude::system_program, solana_program::instruction::Instruction, InstructionData,
        ToAccountMetas,
    },
    common::{get_dispute_config, send_ix, setup_svm},
    pactum_disputes::InitializeDisputeConfigParams,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn initialize_dispute_config_succeeds_for_global_authority() {
    let mut env = setup_svm(false);

    let ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::InitializeDisputeConfig {
            params: InitializeDisputeConfigParams {
                p2p_window_secs: 86_400,
                vote_window_secs: 172_800,
                reveal_window_secs: 43_200,
                arbiter_sla_slash_bps: 500,
                no_fault_fee_bps: 5_000,
            },
        }
        .data(),
        pactum_disputes::accounts::InitializeDisputeConfig {
            dispute_config: env.dispute_config,
            global_config: env.global_config,
            authority: env.authority.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    send_ix(&mut env.svm, ix, &env.authority, &[&env.authority]).unwrap();

    let cfg = get_dispute_config(&env.svm, &env.dispute_config);
    assert!(cfg.is_initialized);
    assert_eq!(cfg.authority, env.authority.pubkey());
    assert_eq!(cfg.global_config, env.global_config);
    assert_eq!(cfg.p2p_window_secs, 86_400);
    assert_eq!(cfg.vote_window_secs, 172_800);
    assert_eq!(cfg.reveal_window_secs, 43_200);
    assert_eq!(cfg.arbiter_sla_slash_bps, 500);
    assert_eq!(cfg.no_fault_fee_bps, 5_000);
}

#[test]
fn initialize_dispute_config_rejects_unauthorized_signer() {
    let mut env = setup_svm(false);
    let attacker = Keypair::new();
    env.svm.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();

    let ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::InitializeDisputeConfig {
            params: InitializeDisputeConfigParams {
                p2p_window_secs: 86_400,
                vote_window_secs: 172_800,
                reveal_window_secs: 43_200,
                arbiter_sla_slash_bps: 500,
                no_fault_fee_bps: 5_000,
            },
        }
        .data(),
        pactum_disputes::accounts::InitializeDisputeConfig {
            dispute_config: env.dispute_config,
            global_config: env.global_config,
            authority: attacker.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    assert!(send_ix(&mut env.svm, ix, &attacker, &[&attacker]).is_err());
}

#[test]
fn initialize_dispute_config_rejects_invalid_bps() {
    let mut env = setup_svm(false);

    let ix = Instruction::new_with_bytes(
        pactum_disputes::id(),
        &pactum_disputes::instruction::InitializeDisputeConfig {
            params: InitializeDisputeConfigParams {
                p2p_window_secs: 86_400,
                vote_window_secs: 172_800,
                reveal_window_secs: 43_200,
                arbiter_sla_slash_bps: 10_001,
                no_fault_fee_bps: 5_000,
            },
        }
        .data(),
        pactum_disputes::accounts::InitializeDisputeConfig {
            dispute_config: env.dispute_config,
            global_config: env.global_config,
            authority: env.authority.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    assert!(send_ix(&mut env.svm, ix, &env.authority, &[&env.authority]).is_err());
}
