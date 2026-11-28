use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum PricingType {
    Fixed,
    Custom,
    DateRange,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub struct CancellationPolicy {
    /// Hours before check-in inside which client cancellation splits apply.
    pub window_hours: u32,
    /// Percentage (0..=100) of total_amount refunded to client inside the window.
    pub client_refund_pct: u8,
}

impl CancellationPolicy {
    pub fn validate(&self) -> Result<()> {
        require!(
            self.client_refund_pct <= crate::constants::MAX_PERCENTAGE,
            crate::error::PactumCoreError::InvalidOfferParams
        );
        Ok(())
    }
}

/// Seeds: `[OFFER_SEED.as_bytes(), provider.key().as_ref(), offer_id.to_le_bytes().as_ref()]` (`["offer", provider, offer_id]`).
#[account]
#[derive(InitSpace)]
pub struct Offer {
    pub offer_id: u32,
    pub base_price: Option<u64>,
    pub pricing_type: PricingType,
    pub approval_window_secs: i64,
    pub provider_bond: u64,
    pub required_client_bond: u64,
    pub jury_fee: u64,
    pub default_arbiter: Option<Pubkey>,
    pub cancellation_policy: Option<CancellationPolicy>,
    pub content_ref: [u8; 32],
    pub is_active: bool,
    pub rent_payer: Pubkey,
    pub bump: u8,
}
