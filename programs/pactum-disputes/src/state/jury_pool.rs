use anchor_lang::prelude::*;

/// Minimal stub for user-owned `JuryPool` sortition tree.
/// Seeds: `[JURY_POOL_SEED.as_bytes(), marketplace.as_ref(), &pool_id.to_le_bytes()]`
/// (`["jury_pool", marketplace, pool_id]`).
#[account(zero_copy)]
pub struct JuryPool {
    pub marketplace: Pubkey,
    pub pool_id: u16,
    pub _padding: [u8; 6],
    pub next_leaf: u32,
    pub vacant_count: u32,
}

/// Minimal stub for user-owned `PoolLeaf` mapping a tree leaf index to a `Judge` authority.
/// Seeds: `[POOL_LEAF_SEED.as_bytes(), marketplace.as_ref(), &pool_id.to_le_bytes(), &leaf_index.to_le_bytes()]`
/// (`["pool_leaf", marketplace, pool_id, leaf_index]`).
#[account]
#[derive(InitSpace)]
pub struct PoolLeaf {
    pub judge: Pubkey,
    pub rent_payer: Pubkey,
    pub bump: u8,
}
