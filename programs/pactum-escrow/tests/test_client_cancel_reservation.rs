mod common;

use {
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::state::{CancellationPolicy, Client, PricingType, Provider},
    pactum_escrow::{
        constants::{
            BOOKING_DAYS_SEED, CPI_AUTHORITY_SEED, DATE_RANGE_AGREEMENT_SEED, ESCROW_VAULT_SEED,
            SECONDS_PER_DAY, SECONDS_PER_HOUR,
        },
        instructions::CreateReservationParams,
        state::{BookingDays, DateRangeAgreement, DateRangeStatus},
        utils::TimestampExt,
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn client_cancel_reservation_applies_window_split_and_fees() {
    let authority = Keypair::new();
    let client_auth = Keypair::new();
    let provider_auth = Keypair::new();

    let (mut svm, global_config_pda, escrow_config_pda, usdc_mint, platform_vault) =
        common::setup_svm(&authority);
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
            base_price: Some(50_000_000),
            bond: 10_000_000,
            cancellation_policy: Some(CancellationPolicy {
                window_hours: 48,
                client_refund_pct: 60,
            }),
        },
    );
    let client_token_account =
        common::create_token_account(&mut svm, &usdc_mint, &client_auth.pubkey(), 200_000_000);
    let provider_token_account =
        common::create_token_account(&mut svm, &usdc_mint, &provider_auth.pubkey(), 50_000_000);

    let check_in = now + 5 * SECONDS_PER_DAY;
    let check_out = check_in + 2 * SECONDS_PER_DAY; // 100 USDC
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
    let (cpi_authority_pda, _) =
        Pubkey::find_program_address(&[CPI_AUTHORITY_SEED.as_bytes()], &pactum_escrow::id());

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
            payer: client_auth.pubkey(),
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
    common::send_ix(&mut svm, create_ix, &client_auth, &[&client_auth]).unwrap();

    let accept_ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::ProviderAcceptReservation {}.data(),
        pactum_escrow::accounts::ProviderAcceptReservation {
            provider_authority: provider_auth.pubkey(),
            escrow_config: escrow_config_pda,
            global_config: global_config_pda,
            provider: provider_pda,
            reservation: reservation_pda,
            escrow_vault: escrow_vault_pda,
            provider_token_account,
            usdc_mint,
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, accept_ix, &provider_auth, &[&provider_auth]).unwrap();

    // Warp inside 48h window (24h before check_in).
    common::warp_to_ts(&mut svm, check_in - 24 * SECONDS_PER_HOUR);
    let cancel_ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::ClientCancelReservation {}.data(),
        pactum_escrow::accounts::ClientCancelReservation {
            client_authority: client_auth.pubkey(),
            escrow_config: escrow_config_pda,
            global_config: global_config_pda,
            client: client_pda,
            provider: provider_pda,
            integrator: None,
            booking_days: booking_days_pda,
            reservation: reservation_pda,
            escrow_vault: escrow_vault_pda,
            platform_vault,
            client_token_account,
            provider_token_account,
            integrator_token_account: None,
            usdc_mint,
            rent_payer: client_auth.pubkey(),
            cpi_authority: cpi_authority_pda,
            core_program: pactum_core::id(),
            token_program: common::TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, cancel_ix, &client_auth, &[&client_auth]).unwrap();

    // Client gets 60% of 100M (60M) + 20M bond = 80M back; started 200M - 120M = 80M + 80M = 160M.
    assert_eq!(
        common::token_account_amount(&svm, &client_token_account),
        160_000_000
    );
    // Remainder = 40M; 1% protocol fee = 400k; provider gets 39.6M + 10M bond = 49.6M; total 89.6M.
    assert_eq!(common::token_account_amount(&svm, &platform_vault), 400_000);
    assert_eq!(
        common::token_account_amount(&svm, &provider_token_account),
        89_600_000
    );

    let res = DateRangeAgreement::try_deserialize(
        &mut svm.get_account(&reservation_pda).unwrap().data.as_slice(),
    )
    .unwrap();
    assert_eq!(res.status, DateRangeStatus::Cancelled);

    let bd = BookingDays::try_deserialize(
        &mut svm.get_account(&booking_days_pda).unwrap().data.as_slice(),
    )
    .unwrap();
    assert_eq!(bd.occupied_days[5], 0);

    let client_acc =
        Client::try_deserialize(&mut svm.get_account(&client_pda).unwrap().data.as_slice())
            .unwrap();
    let provider_acc =
        Provider::try_deserialize(&mut svm.get_account(&provider_pda).unwrap().data.as_slice())
            .unwrap();
    assert_eq!(client_acc.cancelled_agreements, 1);
    assert_eq!(provider_acc.cancelled_agreements, 1);
}
