mod common;

use {
    anchor_lang::{
        prelude::{system_program, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::state::PricingType,
    pactum_escrow::{
        constants::{
            BOOKING_DAYS_SEED, DATE_RANGE_AGREEMENT_SEED, ESCROW_VAULT_SEED, REVIEW_WINDOW_SECS,
            SECONDS_PER_DAY,
        },
        instructions::CreateReservationParams,
        utils::TimestampExt,
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn close_reservation_reclaims_rent_after_96h_and_rejects_early() {
    let authority = Keypair::new();
    let client_auth = Keypair::new();
    let provider_auth = Keypair::new();
    let relayer = Keypair::new();

    let (mut svm, global_config_pda, escrow_config_pda, usdc_mint, _) =
        common::setup_svm(&authority);
    svm.airdrop(&relayer.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&client_auth.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&provider_auth.pubkey(), 10_000_000_000)
        .unwrap();
    let now = 1_780_272_000i64;
    common::warp_to_ts(&mut svm, now);

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
            pricing_type: PricingType::DateRange,
            base_price: Some(25_000_000),
            bond: 0,
            cancellation_policy: None,
        },
    );
    let client_token_account =
        common::create_token_account(&mut svm, &usdc_mint, &client_auth.pubkey(), 200_000_000);

    let check_in = now + 5 * SECONDS_PER_DAY;
    let check_out = check_in + 2 * SECONDS_PER_DAY;
    let year = check_in.to_date().unwrap().year;
    let (booking_days_pda, _) = Pubkey::find_program_address(
        &[
            BOOKING_DAYS_SEED.as_bytes(),
            offer_pda.as_ref(),
            year.to_le_bytes().as_ref(),
        ],
        &pactum_escrow::id(),
    );
    let (reservation_pda, _) = Pubkey::find_program_address(
        &[
            DATE_RANGE_AGREEMENT_SEED.as_bytes(),
            offer_pda.as_ref(),
            client_pda.as_ref(),
            check_in.to_le_bytes().as_ref(),
        ],
        &pactum_escrow::id(),
    );
    let (escrow_vault_pda, _) = Pubkey::find_program_address(
        &[ESCROW_VAULT_SEED.as_bytes(), reservation_pda.as_ref()],
        &pactum_escrow::id(),
    );

    let create_ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::CreateReservation {
            params: CreateReservationParams {
                check_in,
                check_out,
                arbiter: None,
            },
        }
        .data(),
        pactum_escrow::accounts::CreateReservation {
            client_authority: client_auth.pubkey(),
            payer: relayer.pubkey(),
            escrow_config: escrow_config_pda,
            global_config: global_config_pda,
            marketplace: market_pda,
            client: client_pda,
            provider: provider_pda,
            offer: offer_pda,
            integrator: None,
            booking_days: booking_days_pda,
            reservation: reservation_pda,
            escrow_vault: escrow_vault_pda,
            client_token_account,
            usdc_mint,
            token_program: common::TOKEN_PROGRAM_ID,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, create_ix, &relayer, &[&relayer, &client_auth]).unwrap();

    // Provider rejects -> Cancelled.
    let reject_ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::ProviderRejectReservation {}.data(),
        pactum_escrow::accounts::ProviderRejectReservation {
            provider_authority: provider_auth.pubkey(),
            escrow_config: escrow_config_pda,
            global_config: global_config_pda,
            provider: provider_pda,
            client: client_pda,
            booking_days: booking_days_pda,
            reservation: reservation_pda,
            escrow_vault: escrow_vault_pda,
            client_token_account,
            usdc_mint,
            rent_payer: relayer.pubkey(),
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, reject_ix, &provider_auth, &[&provider_auth]).unwrap();

    // Early close rejected.
    let early_ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::CloseReservation {}.data(),
        pactum_escrow::accounts::CloseReservation {
            caller: client_auth.pubkey(),
            reservation: reservation_pda,
            rent_payer: relayer.pubkey(),
        }
        .to_account_metas(None),
    );
    assert!(common::send_ix(&mut svm, early_ix, &client_auth, &[&client_auth]).is_err());

    // After 96h -> closed to rent_payer.
    let relayer_lamports_before = svm.get_account(&relayer.pubkey()).unwrap().lamports;
    common::warp_to_ts(&mut svm, now + REVIEW_WINDOW_SECS);
    let close_ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::CloseReservation {}.data(),
        pactum_escrow::accounts::CloseReservation {
            caller: client_auth.pubkey(),
            reservation: reservation_pda,
            rent_payer: relayer.pubkey(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, close_ix, &client_auth, &[&client_auth]).unwrap();

    assert!(svm.get_account(&reservation_pda).is_none());
    assert!(svm.get_account(&relayer.pubkey()).unwrap().lamports > relayer_lamports_before);
}
