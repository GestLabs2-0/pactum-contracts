use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum ParticipantRole {
    Client,
    Provider,
    Judge,
    Arbiter,
    Integrator,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum AgreementOutcome {
    Completed,
    Cancelled,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum DisputeParty {
    Client,
    Provider,
}

/// Seeds: `[CLIENT_SEED.as_bytes(), marketplace.key().as_ref(), user_main.key().as_ref()]` (`["client", marketplace, user_main]`).
#[account]
#[derive(InitSpace)]
pub struct Client {
    pub marketplace: Pubkey,
    pub user_main: Pubkey,
    pub is_banned: bool,
    pub completed_agreements: u32,
    pub cancelled_agreements: u32,
    pub disputes_won: u32,
    pub disputes_lost: u32,
    pub total_reviews: u32,
    pub total_score: u64,
    pub bump: u8,
}

/// Seeds: `[PROVIDER_SEED.as_bytes(), marketplace.key().as_ref(), user_main.key().as_ref()]` (`["provider", marketplace, user_main]`).
#[account]
#[derive(InitSpace)]
pub struct Provider {
    pub marketplace: Pubkey,
    pub user_main: Pubkey,
    pub is_banned: bool,
    pub offer_count: u32,
    pub completed_agreements: u32,
    pub cancelled_agreements: u32,
    pub disputes_won: u32,
    pub disputes_lost: u32,
    pub total_reviews: u32,
    pub total_score: u64,
    pub bump: u8,
}

/// Seeds: `[JUDGE_SEED.as_bytes(), marketplace.key().as_ref(), user_main.key().as_ref()]` (`["judge", marketplace, user_main]`).
#[account]
#[derive(InitSpace)]
pub struct Judge {
    pub marketplace: Pubkey,
    pub user_main: Pubkey,
    pub staked_amount: u64,
    pub pool_id: u16,
    pub leaf_index: u32,
    pub is_active_in_pool: bool,
    pub active_disputes: u32,
    pub cases_voted: u32,
    pub cases_in_consensus: u32,
    pub is_banned: bool,
    pub bump: u8,
}
