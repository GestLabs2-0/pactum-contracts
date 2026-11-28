use anchor_lang::prelude::*;
use pactum_config::{GlobalConfig, GLOBAL_CONFIG_SEED};
use pactum_core::{
    constants::{CLIENT_SEED, MARKETPLACE_SEED, PROVIDER_SEED},
    cpi::accounts::{CpiRecordJudgeVote, CpiUpdateActiveDisputes},
    cpi::{cpi_record_judge_vote, cpi_update_active_disputes},
    program::PactumCore,
    state::{Client, Judge, Marketplace, ParticipantRole, Provider},
};
use pactum_escrow::{
    program::PactumEscrow,
    state::{Agreement, DateRangeAgreement},
    SettleDisputedParams,
};
use pactum_treasury::{
    cpi::accounts::CpiSlashStake, cpi::cpi_slash_stake, program::PactumTreasury,
};

use crate::{
    constants::{
        BPS_DENOMINATOR, CPI_AUTHORITY_SEED, DEFAULT_JURY_SLASH_BPS, DISPUTE_CONFIG_SEED,
        DISPUTE_SEED, JURY_PANEL_SIZE, ROUND_INITIAL, ROUND_RUNOFF, VOTE_RECORD_SEED,
    },
    error::PactumDisputesError,
    events::{JuryRunoffTriggered, JuryTallied},
    state::{
        Dispute, DisputeConfig, DisputeMode, DisputeOutcome, DisputeStatus, VoteChoice, VoteRecord,
    },
    utils::{
        compute_verdict_payouts, extract_agreement_snapshot, invoke_record_dispute_result,
        invoke_settle_disputed_agreement, outcome_to_winner,
    },
};

#[derive(Accounts)]
pub struct TallyOrTriggerRunoff<'info> {
    #[account(
        mut,
        seeds = [DISPUTE_SEED.as_bytes(), dispute.agreement.as_ref()],
        bump = dispute.bump,
        constraint = dispute.status == DisputeStatus::Voting @ PactumDisputesError::InvalidDisputeStatus,
        constraint = dispute.mode == DisputeMode::DecentralizedJury @ PactumDisputesError::InvalidDisputeMode,
    )]
    pub dispute: Box<Account<'info, Dispute>>,

    #[account(
        seeds = [DISPUTE_CONFIG_SEED.as_bytes()],
        bump = dispute_config.bump,
        constraint = dispute_config.is_initialized @ PactumDisputesError::ConfigNotInitialized,
    )]
    pub dispute_config: Box<Account<'info, DisputeConfig>>,

    #[account(
        seeds = [MARKETPLACE_SEED.as_bytes(), marketplace.market_id.to_le_bytes().as_ref()],
        bump = marketplace.bump,
        seeds::program = pactum_core::ID,
        constraint = marketplace.key() == dispute.marketplace @ PactumDisputesError::InvalidPartyAccount,
    )]
    pub marketplace: Box<Account<'info, Marketplace>>,

    #[account(
        mut,
        seeds = [CLIENT_SEED.as_bytes(), dispute.marketplace.as_ref(), client.authority.as_ref()],
        bump = client.bump,
        seeds::program = pactum_core::ID,
        constraint = client.key() == dispute.client @ PactumDisputesError::InvalidPartyAccount,
    )]
    pub client: Box<Account<'info, Client>>,

    #[account(
        mut,
        seeds = [PROVIDER_SEED.as_bytes(), dispute.marketplace.as_ref(), provider.authority.as_ref()],
        bump = provider.bump,
        seeds::program = pactum_core::ID,
        constraint = provider.key() == dispute.provider @ PactumDisputesError::InvalidPartyAccount,
    )]
    pub provider: Box<Account<'info, Provider>>,

    #[account(mut)]
    pub agreement: Option<Box<Account<'info, Agreement>>>,

    #[account(mut)]
    pub reservation: Option<Box<Account<'info, DateRangeAgreement>>>,

    /// CHECK: Verified by `pactum-escrow` `cpi_settle_disputed_agreement`.
    #[account(mut)]
    pub escrow_vault: UncheckedAccount<'info>,

    /// CHECK: Verified by `pactum-escrow` `cpi_settle_disputed_agreement`.
    #[account(mut)]
    pub client_token_account: UncheckedAccount<'info>,

    /// CHECK: Verified by `pactum-escrow` `cpi_settle_disputed_agreement`.
    #[account(mut)]
    pub provider_token_account: UncheckedAccount<'info>,

    /// CHECK: Verified by `pactum-escrow` and `pactum-treasury`.
    #[account(mut)]
    pub platform_vault: UncheckedAccount<'info>,

    /// CHECK: Jury fee destination token account verified by `pactum-escrow`.
    #[account(mut)]
    pub fee_destination: UncheckedAccount<'info>,

    /// CHECK: Verified by `pactum-treasury` `cpi_slash_stake`.
    #[account(mut)]
    pub judge_stake_vault: UncheckedAccount<'info>,

    /// CHECK: Verified by `pactum-escrow` / `pactum-treasury`.
    pub usdc_mint: UncheckedAccount<'info>,

    /// CHECK: Verified in `pactum-escrow` against `agreement.rent_payer`.
    #[account(mut)]
    pub rent_payer: UncheckedAccount<'info>,

    /// CHECK: Verified by `pactum-escrow`.
    pub escrow_config: UncheckedAccount<'info>,

    /// CHECK: Verified by `pactum-treasury`.
    pub treasury_config: UncheckedAccount<'info>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump = global_config.bump,
        seeds::program = pactum_config::ID,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: Disputes CPI authority PDA.
    #[account(seeds = [CPI_AUTHORITY_SEED.as_bytes()], bump)]
    pub cpi_authority: UncheckedAccount<'info>,

    /// CHECK: Escrow CPI authority PDA forwarded to `pactum-escrow`.
    pub escrow_cpi_authority: UncheckedAccount<'info>,

    /// CHECK: Treasury CPI authority PDA forwarded to `pactum-treasury`.
    pub treasury_cpi_authority: UncheckedAccount<'info>,

    pub escrow_program: Program<'info, PactumEscrow>,
    pub treasury_program: Program<'info, PactumTreasury>,
    pub core_program: Program<'info, PactumCore>,
    /// CHECK: SPL Token program forwarded to CPIs.
    pub token_program: UncheckedAccount<'info>,
}

pub fn handler_tally_or_trigger_runoff<'info>(
    ctx: Context<'info, TallyOrTriggerRunoff<'info>>,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let window = ctx
        .accounts
        .dispute_config
        .vote_window_secs
        .checked_add(ctx.accounts.dispute_config.reveal_window_secs)
        .ok_or(PactumDisputesError::MathOverflow)?;
    let deadline = ctx
        .accounts
        .dispute
        .phase_started_at
        .checked_add(window)
        .ok_or(PactumDisputesError::MathOverflow)?;
    require!(now >= deadline, PactumDisputesError::RevealWindowNotElapsed);

    let panel_size = ctx.accounts.dispute.panel_size as usize;
    require!(
        panel_size > 0 && panel_size <= JURY_PANEL_SIZE,
        PactumDisputesError::InvalidJuryAccounts
    );
    require!(
        ctx.remaining_accounts.len() == panel_size * 2,
        PactumDisputesError::InvalidJuryAccounts
    );

    let mut counts = [0u8; 3]; // [ClientWins, ProviderWins, NoFault]
    let mut revealed_choices: [Option<VoteChoice>; JURY_PANEL_SIZE] = [None; JURY_PANEL_SIZE];
    let mut revealed_total: u8 = 0;

    for (i, slot) in revealed_choices.iter_mut().enumerate().take(panel_size) {
        let judge_info = &ctx.remaining_accounts[i * 2];
        let vote_info = &ctx.remaining_accounts[i * 2 + 1];
        let expected_leaf = ctx.accounts.dispute.selected_leaves[i];

        require_keys_eq!(
            *judge_info.owner,
            pactum_core::ID,
            PactumDisputesError::InvalidJuryAccounts
        );
        let judge_data = judge_info.try_borrow_data()?;
        let mut judge_slice: &[u8] = &judge_data;
        let judge = Judge::try_deserialize(&mut judge_slice)?;
        require_keys_eq!(
            judge.marketplace,
            ctx.accounts.dispute.marketplace,
            PactumDisputesError::InvalidJuryAccounts
        );
        require!(
            judge.leaf_index == expected_leaf,
            PactumDisputesError::InvalidJuryAccounts
        );

        let (expected_vote_pda, _) = Pubkey::find_program_address(
            &[
                VOTE_RECORD_SEED.as_bytes(),
                ctx.accounts.dispute.key().as_ref(),
                &[ctx.accounts.dispute.round],
                &expected_leaf.to_le_bytes(),
            ],
            &crate::ID,
        );
        require_keys_eq!(
            vote_info.key(),
            expected_vote_pda,
            PactumDisputesError::VoteRecordMismatch
        );

        if *vote_info.owner == crate::ID && vote_info.data_len() >= 8 {
            let vote_data = vote_info.try_borrow_data()?;
            let mut vote_slice: &[u8] = &vote_data;
            let vr = VoteRecord::try_deserialize(&mut vote_slice)?;
            require_keys_eq!(
                vr.dispute,
                ctx.accounts.dispute.key(),
                PactumDisputesError::VoteRecordMismatch
            );
            if vr.has_revealed {
                if let Some(choice) = vr.revealed_choice {
                    let allowed = match ctx.accounts.dispute.runoff_choices {
                        Some(pair) => choice == pair[0] || choice == pair[1],
                        None => true,
                    };
                    if allowed {
                        *slot = Some(choice);
                        revealed_total = revealed_total.saturating_add(1);
                        let idx = choice_index(choice);
                        counts[idx] = counts[idx].saturating_add(1);
                    }
                }
            }
        }
    }

    let winning_choice = if revealed_total == 0 {
        VoteChoice::NoFault
    } else {
        let majority = revealed_total / 2;
        if counts[0] > majority {
            VoteChoice::ClientWins
        } else if counts[1] > majority {
            VoteChoice::ProviderWins
        } else if counts[2] > majority {
            VoteChoice::NoFault
        } else if ctx.accounts.dispute.round == ROUND_INITIAL {
            let pair = top_two_choices(counts);
            let dispute = &mut ctx.accounts.dispute;
            dispute.round = ROUND_RUNOFF;
            dispute.runoff_choices = Some(pair);
            dispute.phase_started_at = now;
            emit!(JuryRunoffTriggered {
                dispute: dispute.key(),
                round: ROUND_RUNOFF,
                choices: pair,
            });
            return Ok(());
        } else {
            VoteChoice::NoFault
        }
    };

    let outcome = match winning_choice {
        VoteChoice::ClientWins => DisputeOutcome::ClientWins,
        VoteChoice::ProviderWins => DisputeOutcome::ProviderWins,
        VoteChoice::NoFault => DisputeOutcome::NoFault,
    };

    let snapshot = extract_agreement_snapshot(
        &ctx.accounts.agreement,
        &ctx.accounts.reservation,
        ctx.accounts.dispute.marketplace,
    )?;
    require_keys_eq!(
        snapshot.key,
        ctx.accounts.dispute.agreement,
        PactumDisputesError::InvalidAgreementTarget
    );

    let (client_payout, provider_payout, fee_payout, mark_completed) = compute_verdict_payouts(
        &snapshot,
        &ctx.accounts.dispute,
        &ctx.accounts.dispute_config,
        outcome,
        snapshot.jury_fee,
    )?;

    invoke_settle_disputed_agreement(
        &ctx.accounts.escrow_program,
        &ctx.accounts.escrow_config.to_account_info(),
        &ctx.accounts.global_config.to_account_info(),
        &ctx.accounts.cpi_authority.to_account_info(),
        ctx.bumps.cpi_authority,
        &ctx.accounts.client.to_account_info(),
        &ctx.accounts.provider.to_account_info(),
        ctx.accounts.agreement.as_ref().map(|a| a.to_account_info()),
        ctx.accounts
            .reservation
            .as_ref()
            .map(|r| r.to_account_info()),
        &ctx.accounts.escrow_vault.to_account_info(),
        &ctx.accounts.client_token_account.to_account_info(),
        &ctx.accounts.provider_token_account.to_account_info(),
        &ctx.accounts.platform_vault.to_account_info(),
        Some(ctx.accounts.fee_destination.to_account_info()),
        &ctx.accounts.usdc_mint.to_account_info(),
        &ctx.accounts.rent_payer.to_account_info(),
        &ctx.accounts.escrow_cpi_authority.to_account_info(),
        &ctx.accounts.core_program.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        SettleDisputedParams {
            restore_previous_status: false,
            client_payout,
            provider_payout,
            fee_payout,
            mark_completed,
        },
    )?;

    invoke_record_dispute_result(
        &ctx.accounts.core_program,
        &ctx.accounts.client.to_account_info(),
        &ctx.accounts.provider.to_account_info(),
        &ctx.accounts.global_config.to_account_info(),
        &ctx.accounts.cpi_authority.to_account_info(),
        ctx.bumps.cpi_authority,
        outcome_to_winner(outcome),
    )?;

    let seeds: &[&[u8]] = &[CPI_AUTHORITY_SEED.as_bytes(), &[ctx.bumps.cpi_authority]];
    let signer = &[seeds];

    for (i, revealed) in revealed_choices.iter().enumerate().take(panel_size) {
        let judge_info = &ctx.remaining_accounts[i * 2];
        let in_consensus = *revealed == Some(winning_choice);

        let vote_cpi = CpiContext::new_with_signer(
            ctx.accounts.core_program.key(),
            CpiRecordJudgeVote {
                judge: judge_info.clone(),
                global_config: ctx.accounts.global_config.to_account_info(),
                cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
            },
            signer,
        );
        cpi_record_judge_vote(vote_cpi, in_consensus)?;

        let active_cpi = CpiContext::new_with_signer(
            ctx.accounts.core_program.key(),
            CpiUpdateActiveDisputes {
                judge: Some(judge_info.clone()),
                arbiter_profile: None,
                global_config: ctx.accounts.global_config.to_account_info(),
                cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
            },
            signer,
        );
        cpi_update_active_disputes(active_cpi, ParticipantRole::Judge, -1)?;

        if !in_consensus {
            let staked = {
                let judge_data = judge_info.try_borrow_data()?;
                let mut judge_slice: &[u8] = &judge_data;
                Judge::try_deserialize(&mut judge_slice)?.staked_amount
            };
            let slash_amount = u64::try_from(
                (staked as u128)
                    .checked_mul(DEFAULT_JURY_SLASH_BPS as u128)
                    .ok_or(PactumDisputesError::MathOverflow)?
                    / BPS_DENOMINATOR,
            )
            .map_err(|_| PactumDisputesError::MathOverflow)?;
            if slash_amount > 0 {
                let slash_ctx = CpiContext::new_with_signer(
                    ctx.accounts.treasury_program.key(),
                    CpiSlashStake {
                        cpi_authority: ctx.accounts.cpi_authority.to_account_info(),
                        config: ctx.accounts.treasury_config.to_account_info(),
                        global_config: ctx.accounts.global_config.to_account_info(),
                        marketplace: ctx.accounts.marketplace.to_account_info(),
                        judge: Some(judge_info.clone()),
                        arbiter_profile: None,
                        stake_vault: ctx.accounts.judge_stake_vault.to_account_info(),
                        recipient_token_account: ctx.accounts.platform_vault.to_account_info(),
                        usdc_mint: ctx.accounts.usdc_mint.to_account_info(),
                        treasury_cpi_authority: ctx
                            .accounts
                            .treasury_cpi_authority
                            .to_account_info(),
                        core_program: ctx.accounts.core_program.to_account_info(),
                        token_program: ctx.accounts.token_program.to_account_info(),
                    },
                    signer,
                );
                cpi_slash_stake(slash_ctx, ParticipantRole::Judge, slash_amount)?;
            }
        }
    }

    let dispute = &mut ctx.accounts.dispute;
    dispute.status = DisputeStatus::Resolved;
    dispute.outcome = Some(outcome);

    emit!(JuryTallied {
        dispute: dispute.key(),
        round: dispute.round,
        outcome,
        fee_payout,
    });
    Ok(())
}

fn choice_index(choice: VoteChoice) -> usize {
    match choice {
        VoteChoice::ClientWins => 0,
        VoteChoice::ProviderWins => 1,
        VoteChoice::NoFault => 2,
    }
}

fn top_two_choices(counts: [u8; 3]) -> [VoteChoice; 2] {
    let mut items = [
        (counts[0], VoteChoice::ClientWins),
        (counts[1], VoteChoice::ProviderWins),
        (counts[2], VoteChoice::NoFault),
    ];
    items.sort_by(|a, b| b.0.cmp(&a.0));
    [items[0].1, items[1].1]
}
