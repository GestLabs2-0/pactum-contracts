mod common;

use {
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::state::PricingType,
    pactum_escrow::{
        constants::{AGREEMENT_SEED, ESCROW_VAULT_SEED},
        instructions::ProposeAgreementParams,
        state::{Agreement, AgreementStatus},
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn propose_agreement_sets_custom_milestones_and_deposits_provider_bond() {
    let authority = Keypair::new();
    let client_auth = Keypair::new();
    let provider_auth = Keypair::new();

    let (mut svm, global_config_pda, escrow_config_pda, usdc_mint, _) =
        common::setup_svm(&authority);
    svm.airdrop(&provider_auth.pubkey(), 10_000_000_000)
        .unwrap();

    let market_pda = common::seed_marketplace(&mut svm, 1);
    let (client_pda, _client_user_main_pda) =
        common::seed_client(&mut svm, market_pda, &client_auth.pubkey());
    let (provider_pda, _provider_user_main_pda) =
        common::seed_provider(&mut svm, market_pda, &provider_auth.pubkey());
    let offer_pda = common::seed_offer(
        &mut svm,
        provider_pda,
        0,
        common::SeedOfferOpts {
            pricing_type: PricingType::Custom,
            base_price: None,
            provider_bond: 15_000_000,
            required_client_bond: 10_000_000,
            cancellation_policy: None,
        },
    );
    let provider_token_account =
        common::create_token_account(&mut svm, &usdc_mint, &provider_auth.pubkey(), 50_000_000);

    let nonce = 1u64;
    let (agreement_pda, _) = Pubkey::find_program_address(
        &[
            AGREEMENT_SEED.as_bytes(),
            offer_pda.as_ref(),
            client_pda.as_ref(),
            nonce.to_le_bytes().as_ref(),
        ],
        &pactum_escrow::id(),
    );
    let (escrow_vault_pda, _) = Pubkey::find_program_address(
        &[ESCROW_VAULT_SEED.as_bytes(), agreement_pda.as_ref()],
        &pactum_escrow::id(),
    );

    let ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::ProposeAgreement {
            params: ProposeAgreementParams {
                nonce,
                milestone_amounts: vec![30_000_000, 70_000_000],
                step_duration_secs: 172_800,
                deal_contract_hash: Some([5u8; 32]),
                arbiter: None,
            },
        }
        .data(),
        pactum_escrow::accounts::ProposeAgreement {
            provider_authority: provider_auth.pubkey(),
            payer: provider_auth.pubkey(),
            escrow_config: escrow_config_pda,
            global_config: global_config_pda,
            marketplace: market_pda,
            provider: provider_pda,
            client: client_pda,
            offer: offer_pda,
            integrator: None,
            agreement: agreement_pda,
            escrow_vault: escrow_vault_pda,
            provider_token_account,
            usdc_mint,
            token_program: common::TOKEN_PROGRAM_ID,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &provider_auth, &[&provider_auth]).unwrap();

    let agreement =
        Agreement::try_deserialize(&mut svm.get_account(&agreement_pda).unwrap().data.as_slice())
            .unwrap();
    assert_eq!(agreement.status, AgreementStatus::Proposed);
    assert_eq!(agreement.total_amount, 100_000_000);
    assert_eq!(agreement.milestone_amounts, vec![30_000_000, 70_000_000]);
    assert_eq!(
        common::token_account_amount(&svm, &escrow_vault_pda),
        15_000_000
    );
    assert_eq!(
        common::token_account_amount(&svm, &provider_token_account),
        35_000_000
    );
}
