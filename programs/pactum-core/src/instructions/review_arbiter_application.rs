use anchor_lang::{
    prelude::*,
    system_program::{create_account, CreateAccount},
};

use crate::{
    constants::{ARBITER_APP_SEED, ARBITER_SEED, MARKETPLACE_SEED},
    error::PactumCoreError,
    events::ArbiterReviewed,
    state::{ArbiterApplication, ArbiterProfile, Marketplace},
};

#[derive(Accounts)]
pub struct ReviewArbiterApplication<'info> {
    #[account(
        mut,
        close = rent_payer,
        seeds = [
            ARBITER_APP_SEED.as_bytes(),
            marketplace.key().as_ref(),
            arbiter_application.authority.as_ref(),
        ],
        bump = arbiter_application.bump,
        has_one = marketplace,
        has_one = rent_payer @ PactumCoreError::InvalidRentPayer,
    )]
    pub arbiter_application: Account<'info, ArbiterApplication>,

    #[account(
        seeds = [MARKETPLACE_SEED.as_bytes(), marketplace.market_id.to_le_bytes().as_ref()],
        bump = marketplace.bump,
        has_one = committee @ PactumCoreError::Unauthorized,
    )]
    pub marketplace: Account<'info, Marketplace>,

    #[account(mut)]
    pub committee: Signer<'info>,

    /// CHECK: Created via CPI only when `approve == true`; validated by PDA seeds.
    #[account(
        mut,
        seeds = [
            ARBITER_SEED.as_bytes(),
            marketplace.key().as_ref(),
            arbiter_application.authority.as_ref(),
        ],
        bump,
    )]
    pub arbiter_profile: UncheckedAccount<'info>,

    /// CHECK: Verified via `has_one = rent_payer` on `arbiter_application`.
    #[account(mut)]
    pub rent_payer: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_review_arbiter_application(
    ctx: Context<ReviewArbiterApplication>,
    approve: bool,
) -> Result<()> {
    let app = &ctx.accounts.arbiter_application;
    let marketplace_key = ctx.accounts.marketplace.key();

    if approve {
        let space = 8 + ArbiterProfile::INIT_SPACE;
        let lamports = Rent::get()?.minimum_balance(space);
        let bump = ctx.bumps.arbiter_profile;
        let signer_seeds: &[&[&[u8]]] = &[&[
            ARBITER_SEED.as_bytes(),
            marketplace_key.as_ref(),
            app.authority.as_ref(),
            &[bump],
        ]];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.system_program.key(),
            CreateAccount {
                from: ctx.accounts.committee.to_account_info(),
                to: ctx.accounts.arbiter_profile.to_account_info(),
            },
            signer_seeds,
        );
        create_account(cpi_ctx, lamports, space as u64, &crate::ID)?;

        let profile = ArbiterProfile {
            marketplace: marketplace_key,
            authority: app.authority,
            sas_kyb_attestation: app.sas_kyb_attestation,
            arbiter_fee: app.arbiter_fee,
            staked_amount: 0,
            sla_secs: app.sla_secs,
            active_disputes: 0,
            cases_resolved: 0,
            is_active: true,
            bump,
        };
        let mut data = ctx.accounts.arbiter_profile.try_borrow_mut_data()?;
        let mut writer: &mut [u8] = &mut data;
        profile.try_serialize(&mut writer)?;
    }

    emit!(ArbiterReviewed {
        marketplace: marketplace_key,
        authority: app.authority,
        approved: approve,
    });
    Ok(())
}
