use anchor_lang::prelude::*;

use crate::error::PactumCoreError;

pub fn apply_stake_delta(current: u64, amount: u64, is_stake: bool) -> Result<u64> {
    if is_stake {
        current
            .checked_add(amount)
            .ok_or_else(|| error!(PactumCoreError::MathOverflow))
    } else {
        current
            .checked_sub(amount)
            .ok_or_else(|| error!(PactumCoreError::MathOverflow))
    }
}

pub fn apply_i8_delta(current: u32, delta: i8) -> Result<u32> {
    if delta >= 0 {
        current
            .checked_add(delta as u32)
            .ok_or_else(|| error!(PactumCoreError::MathOverflow))
    } else {
        current
            .checked_sub(delta.unsigned_abs() as u32)
            .ok_or_else(|| error!(PactumCoreError::MathOverflow))
    }
}
