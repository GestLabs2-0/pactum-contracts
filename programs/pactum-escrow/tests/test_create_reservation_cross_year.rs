mod common;

use {
    anchor_lang::{
        prelude::{system_program, AccountDeserialize, Pubkey},
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    pactum_core::state::PricingType,
    pactum_escrow::{
        constants::{
            BOOKING_DAYS_SEED, DATE_RANGE_AGREEMENT_SEED, ESCROW_VAULT_SEED, SECONDS_PER_DAY,
        },
        instructions::CreateReservationParams,
        state::{BookingDays, DateRangeAgreement, DateRangeStatus},
        utils::TimestampExt,
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn create_reservation_cross_year_locks_both_years() {
    let authority = Keypair::new();
    let client_auth = Keypair::new();
    let provider_auth = Keypair::new();

    let (mut svm, global_config_pda, escrow_config_pda, usdc_mint, _) =
        common::setup_svm(&authority);
    svm.airdrop(&client_auth.pubkey(), 10_000_000_000).unwrap();
    // 2026-12-28T00:00:00Z = 1798416000
    let now = 1_798_416_000i64;
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
            base_price: Some(20_000_000),
            provider_bond: 0,
            required_client_bond: 0,
            cancellation_policy: None,
        },
    );
    let client_token_account =
        common::create_token_account(&mut svm, &usdc_mint, &client_auth.pubkey(), 200_000_000);

    let check_in = now + 2 * SECONDS_PER_DAY; // 2026-12-30
    let check_out = check_in + 4 * SECONDS_PER_DAY; // 2027-01-03 (4 nights = 80 USDC)
    let year_in = check_in.to_date().unwrap().year;
    let year_out = check_out.to_date().unwrap().year;
    assert_eq!(year_in, 2026);
    assert_eq!(year_out, 2027);

    let (booking_days_pda, _) = Pubkey::find_program_address(
        &[
            BOOKING_DAYS_SEED.as_bytes(),
            offer_pda.as_ref(),
            year_in.to_le_bytes().as_ref(),
        ],
        &pactum_escrow::id(),
    );
    let (booking_days_next_pda, _) = Pubkey::find_program_address(
        &[
            BOOKING_DAYS_SEED.as_bytes(),
            offer_pda.as_ref(),
            year_out.to_le_bytes().as_ref(),
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

    let ix = Instruction::new_with_bytes(
        pactum_escrow::id(),
        &pactum_escrow::instruction::CreateReservationCrossYear {
            params: CreateReservationParams {
                check_in,
                check_out,
                arbiter: None,
            },
        }
        .data(),
        pactum_escrow::accounts::CreateReservationCrossYear {
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
            booking_days_next: booking_days_next_pda,
            reservation: reservation_pda,
            escrow_vault: escrow_vault_pda,
            client_token_account,
            usdc_mint,
            token_program: common::TOKEN_PROGRAM_ID,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
    );
    common::send_ix(&mut svm, ix, &client_auth, &[&client_auth]).unwrap();

    let res = DateRangeAgreement::try_deserialize(
        &mut svm.get_account(&reservation_pda).unwrap().data.as_slice(),
    )
    .unwrap();
    assert_eq!(res.status, DateRangeStatus::Pending);
    assert_eq!(res.total_amount, 80_000_000);
    assert_eq!(res.cancellation_policy.window_hours, 72);
    assert_eq!(res.cancellation_policy.client_refund_pct, 60);

    let bd_in = BookingDays::try_deserialize(
        &mut svm.get_account(&booking_days_pda).unwrap().data.as_slice(),
    )
    .unwrap();
    let bd_out = BookingDays::try_deserialize(
        &mut svm
            .get_account(&booking_days_next_pda)
            .unwrap()
            .data
            .as_slice(),
    )
    .unwrap();
    assert_ne!(bd_in.occupied_days[11], 0); // Dec 2026
    assert_ne!(bd_out.occupied_days[0], 0); // Jan 2027
}
