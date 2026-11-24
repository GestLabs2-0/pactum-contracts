use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum PricingType {
    Fixed,
    Custom,
    DateRange,
}

/// Seeds: `[OFFER_SEED.as_bytes(), provider.key().as_ref(), offer_id.to_le_bytes().as_ref()]` (`["offer", provider, offer_id]`).
#[account]
#[derive(InitSpace)]
pub struct Offer {
    pub provider: Pubkey,
    pub marketplace: Pubkey,
    pub offer_id: u32,
    pub base_price: Option<u64>,
    pub pricing_type: PricingType,
    pub approval_window_secs: i64,
    pub provider_bond: u64,
    pub required_client_bond: u64,
    pub jury_fee: u64,
    pub default_arbiter: Option<Pubkey>,
    pub content_ref: [u8; 32],
    pub state_hash: [u8; 32],
    pub is_active: bool,
    pub rent_payer: Pubkey,
    pub bump: u8,
}
