use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::{
    constants::{
        CORE_PROGRAM_ID, DISPUTES_PROGRAM_ID, ESCROW_PROGRAM_ID, GLOBAL_CONFIG_SEED,
        MAX_PROTOCOL_FEE_BPS, PLATFORM_VAULT_CONFIG_SEED, PLATFORM_VAULT_SEED, TREASURY_PROGRAM_ID,
    },
    error::PactumConfigError,
    events::ConfigInitialized,
    state::GlobalConfig,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct InitializeConfigParams {
    pub protocol_fee_bps: u16,
    pub max_protocol_fee: u64,
    pub dispute_escalation_fee: u64,
    pub sas_program: Pubkey,
    pub sas_credential: Pubkey,
}

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + GlobalConfig::INIT_SPACE,
        seeds = [GLOBAL_CONFIG_SEED.as_bytes()],
        bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: PDA that will sign as the platform-vault authority.
    #[account(seeds = [PLATFORM_VAULT_SEED.as_bytes()], bump)]
    pub platform_vault_pda: UncheckedAccount<'info>,

    #[account(
        init,
        payer = authority,
        token::mint = usdc_mint,
        token::authority = platform_vault_pda,
        seeds = [PLATFORM_VAULT_CONFIG_SEED.as_bytes()],
        bump,
    )]
    pub platform_vault: InterfaceAccount<'info, TokenAccount>,

    pub usdc_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,

    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_config(
    ctx: Context<InitializeConfig>,
    params: InitializeConfigParams,
) -> Result<()> {
    require!(
        params.protocol_fee_bps < MAX_PROTOCOL_FEE_BPS,
        PactumConfigError::InvalidFeeBps
    );

    let global_config = &mut ctx.accounts.global_config;
    global_config.authority = ctx.accounts.authority.key();
    global_config.usdc_mint = ctx.accounts.usdc_mint.key();
    global_config.platform_vault = ctx.accounts.platform_vault.key();
    global_config.platform_vault_bump = ctx.bumps.platform_vault_pda;
    global_config.protocol_fee_bps = params.protocol_fee_bps;
    global_config.max_protocol_fee = params.max_protocol_fee;
    global_config.dispute_escalation_fee = params.dispute_escalation_fee;
    global_config.sas_program = params.sas_program;
    global_config.sas_credential = params.sas_credential;
    global_config.core_program = CORE_PROGRAM_ID;
    global_config.escrow_program = ESCROW_PROGRAM_ID;
    global_config.disputes_program = DISPUTES_PROGRAM_ID;
    global_config.treasury_program = TREASURY_PROGRAM_ID;
    global_config.is_initialized = true;
    global_config.bump = ctx.bumps.global_config;

    emit!(ConfigInitialized {
        authority: global_config.authority,
        usdc_mint: global_config.usdc_mint,
        protocol_fee_bps: global_config.protocol_fee_bps,
        max_protocol_fee: global_config.max_protocol_fee,
        dispute_escalation_fee: global_config.dispute_escalation_fee,
    });
    Ok(())
}
