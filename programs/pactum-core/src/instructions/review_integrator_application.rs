use anchor_lang::{
    prelude::*,
    system_program::{create_account, CreateAccount},
};

use crate::{
    constants::{INTEGRATOR_APP_SEED, INTEGRATOR_SEED, MARKETPLACE_SEED},
    error::PactumCoreError,
    events::IntegratorReviewed,
    state::{IntegratorApplication, IntegratorProfile, Marketplace},
};

#[derive(Accounts)]
pub struct ReviewIntegratorApplication<'info> {
    #[account(
        mut,
        close = rent_payer,
        seeds = [
            INTEGRATOR_APP_SEED.as_bytes(),
            marketplace.key().as_ref(),
            integrator_application.authority.as_ref(),
        ],
        bump = integrator_application.bump,
        has_one = marketplace,
        has_one = rent_payer @ PactumCoreError::InvalidRentPayer,
    )]
    pub integrator_application: Account<'info, IntegratorApplication>,

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
            INTEGRATOR_SEED.as_bytes(),
            marketplace.key().as_ref(),
            integrator_application.authority.as_ref(),
        ],
        bump,
    )]
    pub integrator_profile: UncheckedAccount<'info>,

    /// CHECK: Verified via `has_one = rent_payer` on `integrator_application`.
    #[account(mut)]
    pub rent_payer: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_review_integrator_application(
    ctx: Context<ReviewIntegratorApplication>,
    approve: bool,
) -> Result<()> {
    let app = &ctx.accounts.integrator_application;
    let marketplace_key = ctx.accounts.marketplace.key();

    if approve {
        let space = 8 + IntegratorProfile::INIT_SPACE;
        let lamports = Rent::get()?.minimum_balance(space);
        let bump = ctx.bumps.integrator_profile;
        let signer_seeds: &[&[&[u8]]] = &[&[
            INTEGRATOR_SEED.as_bytes(),
            marketplace_key.as_ref(),
            app.authority.as_ref(),
            &[bump],
        ]];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.system_program.key(),
            CreateAccount {
                from: ctx.accounts.committee.to_account_info(),
                to: ctx.accounts.integrator_profile.to_account_info(),
            },
            signer_seeds,
        );
        create_account(cpi_ctx, lamports, space as u64, &crate::ID)?;

        let profile = IntegratorProfile {
            marketplace: marketplace_key,
            authority: app.authority,
            sas_kyb_attestation: app.sas_kyb_attestation,
            default_fee_bps: app.default_fee_bps,
            fallback_resolutions: 0,
            is_active: true,
            is_banned: false,
            bump,
        };
        let mut data = ctx.accounts.integrator_profile.try_borrow_mut_data()?;
        let mut writer: &mut [u8] = &mut data;
        profile.try_serialize(&mut writer)?;
    }

    emit!(IntegratorReviewed {
        marketplace: marketplace_key,
        authority: app.authority,
        approved: approve,
    });
    Ok(())
}
