//! Vault Factory Program — scaffold. Full design: `specs/solana_programs_pseudocode.rs`.

use anchor_lang::prelude::*;

declare_id!("EmPTRDCu8v7FDBo1PTDhDJZk8D1UMrCnKc1WsihPA1zy");

#[program]
pub mod vault_factory {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let _ = ctx.accounts.payer.key();
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
}
