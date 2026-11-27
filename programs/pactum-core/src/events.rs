use anchor_lang::prelude::*;

use crate::state::{DisputeParty, ParticipantRole};

#[event]
pub struct MarketplaceCreated {
    pub marketplace: Pubkey,
    pub market_id: u64,
    pub committee: Pubkey,
}

#[event]
pub struct MarketplaceUpdated {
    pub marketplace: Pubkey,
    pub committee: Pubkey,
}

#[event]
pub struct GlobalBanUpdated {
    pub user_main: Pubkey,
    pub authority: Pubkey,
    pub is_banned: bool,
}

#[event]
pub struct LocalBanUpdated {
    pub marketplace: Pubkey,
    pub user_main: Pubkey,
    pub role: ParticipantRole,
    pub is_banned: bool,
}

#[event]
pub struct OfferCreated {
    pub offer: Pubkey,
    pub provider: Pubkey,
    pub marketplace: Pubkey,
    pub offer_id: u32,
}

#[event]
pub struct ArbiterReviewed {
    pub marketplace: Pubkey,
    pub authority: Pubkey,
    pub approved: bool,
}

#[event]
pub struct IntegratorInitialized {
    pub marketplace: Pubkey,
    pub authority: Pubkey,
    pub default_fee_bps: u16,
}

#[event]
pub struct IntegratorReviewed {
    pub marketplace: Pubkey,
    pub authority: Pubkey,
    pub approved: bool,
}

#[event]
pub struct RoleReviewRecorded {
    pub marketplace: Pubkey,
    pub user_main: Pubkey,
    pub role: ParticipantRole,
    pub score: u8,
    pub total_reviews: u32,
}

#[event]
pub struct DisputeResultRecorded {
    pub marketplace: Pubkey,
    pub client: Pubkey,
    pub provider: Pubkey,
    pub winner: Option<DisputeParty>,
}
