//! Native Vault Program — scaffold.

use anchor_lang::prelude::*;

declare_id!("2Rzwy9iugqWqWz2inGBPMXWH5zgC7sR6Rxtyr5nVeXW8");

#[program]
pub mod native_vault {
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
