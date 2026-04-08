//! Staking Program — scaffold.

use anchor_lang::prelude::*;

declare_id!("HW3taPZT4E7YGQEQKvQUCNcTGPsbfPv8mxGTdwFvMSKj");

#[program]
pub mod staking {
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
