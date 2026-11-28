use anchor_lang::prelude::*;

use crate::state::VoteChoice;

/// Seeds: `[VOTE_RECORD_SEED.as_bytes(), dispute.as_ref(), &[round], &leaf_index.to_le_bytes()]`
/// (`["vote_record", dispute, round, leaf_index]`).
#[account]
#[derive(InitSpace)]
pub struct VoteRecord {
    pub dispute: Pubkey,
    pub round: u8,
    pub leaf_index: u32,
    pub commit_hash: [u8; 32],
    pub revealed_choice: Option<VoteChoice>,
    pub has_revealed: bool,
    pub rent_payer: Pubkey,
    pub bump: u8,
}
