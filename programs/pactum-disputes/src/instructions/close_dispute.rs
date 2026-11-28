use anchor_lang::prelude::*;
use anchor_lang::system_program;

use crate::{
    constants::DISPUTE_SEED,
    error::PactumDisputesError,
    events::DisputeClosed,
    state::{Dispute, DisputeStatus, VoteRecord},
};

#[derive(Accounts)]
pub struct CloseDispute<'info> {
    #[account(
        mut,
        close = rent_payer,
        seeds = [DISPUTE_SEED.as_bytes(), dispute.agreement.as_ref()],
        bump = dispute.bump,
        constraint = dispute.status == DisputeStatus::Resolved @ PactumDisputesError::InvalidDisputeStatus,
        has_one = rent_payer @ PactumDisputesError::InvalidRentPayer,
    )]
    pub dispute: Account<'info, Dispute>,

    /// CHECK: Verified via `has_one = rent_payer` on `Dispute`.
    #[account(mut)]
    pub rent_payer: UncheckedAccount<'info>,
}

pub fn handler_close_dispute(ctx: Context<CloseDispute>) -> Result<()> {
    require!(
        ctx.remaining_accounts.len().is_multiple_of(2),
        PactumDisputesError::InvalidJuryAccounts
    );

    let dispute_key = ctx.accounts.dispute.key();
    for pair in ctx.remaining_accounts.chunks_exact(2) {
        let vote_info = &pair[0];
        let vr_rent_payer = &pair[1];

        require_keys_eq!(
            *vote_info.owner,
            crate::ID,
            PactumDisputesError::VoteRecordMismatch
        );
        let expected_payer = {
            let data = vote_info.try_borrow_data()?;
            let mut slice: &[u8] = &data;
            let vr = VoteRecord::try_deserialize(&mut slice)?;
            require_keys_eq!(
                vr.dispute,
                dispute_key,
                PactumDisputesError::VoteRecordMismatch
            );
            vr.rent_payer
        };
        require_keys_eq!(
            vr_rent_payer.key(),
            expected_payer,
            PactumDisputesError::InvalidRentPayer
        );

        let lamports = vote_info.lamports();
        **vote_info.try_borrow_mut_lamports()? = 0;
        **vr_rent_payer.try_borrow_mut_lamports()? = vr_rent_payer
            .lamports()
            .checked_add(lamports)
            .ok_or(PactumDisputesError::MathOverflow)?;
        vote_info.assign(&system_program::ID);
        vote_info.resize(0)?;
    }

    emit!(DisputeClosed {
        dispute: dispute_key,
        agreement: ctx.accounts.dispute.agreement,
        rent_payer: ctx.accounts.rent_payer.key(),
    });
    Ok(())
}
