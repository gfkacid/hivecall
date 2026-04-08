// ============================================================
// HIVECALL — SOLANA PROGRAMS (Anchor/Rust Pseudocode)
// ============================================================
// Programs:
//   1. Vault Factory Program
//   2. Native Vault Program
//   3. Staking Program
//   4. HIVE Token Program (SPL + fee hook)
// ============================================================

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Mint};

// ============================================================
// PROGRAM IDs (placeholder)
// ============================================================

declare_id!("VaultFactory111111111111111111111111111111111");
declare_id!("NativeVault1111111111111111111111111111111111");
declare_id!("StakingProg1111111111111111111111111111111111");


// ============================================================
// 1. VAULT FACTORY PROGRAM
// ============================================================
// Handles permissionless vault creation, deposits,
// withdrawals, share token issuance, and whitelist management.
// ============================================================

#[program]
mod vault_factory {
    use super::*;

    // ----------------------------------------------------------
    // create_vault
    // ----------------------------------------------------------
    // Called by any user to create a new prediction vault.
    // Deploys a VaultAccount PDA and a ShareMint for this vault.
    // User pays Solana gas.
    // Emits CreateVaultEvent so the relayer can deploy the
    // corresponding Arbitrum Vault Executor contract.
    // ----------------------------------------------------------
    pub fn create_vault(
        ctx: Context<CreateVault>,
        params: VaultParams,
    ) -> Result<()> {
        require!(params.performance_fee <= 1000, ErrorCode::FeeTooHigh); // max 10% (basis points)
        require!(params.min_quorum <= 10000, ErrorCode::InvalidQuorum);  // max 100%

        let vault = &mut ctx.accounts.vault;
        vault.admin            = ctx.accounts.admin.key();
        vault.vault_id         = params.vault_id;
        vault.visibility       = params.visibility;        // Public | Private
        vault.management_type  = params.management_type;  // Managed | Crowdsourced
        vault.deposit_cap      = params.deposit_cap;      // 0 = unlimited
        vault.performance_fee  = params.performance_fee;  // basis points (0–1000)
        vault.fee_address      = params.fee_address;
        vault.min_quorum       = params.min_quorum;       // basis points of total shares
        vault.total_shares     = 0;
        vault.solana_balance   = 0;
        vault.arbitrum_balance = 0;                       // tracked off-chain, mirrored here
        vault.epoch            = 0;
        vault.is_active        = true;
        vault.share_mint       = ctx.accounts.share_mint.key();

        emit!(CreateVaultEvent {
            vault_id:        vault.vault_id,
            admin:           vault.admin,
            management_type: vault.management_type,
            performance_fee: vault.performance_fee,
        });

        Ok(())
    }

    // ----------------------------------------------------------
    // add_to_whitelist
    // ----------------------------------------------------------
    // Admin only. Add an address to a private vault's whitelist.
    // ----------------------------------------------------------
    pub fn add_to_whitelist(
        ctx: Context<AdminOnly>,
        address: Pubkey,
    ) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        require!(vault.visibility == Visibility::Private, ErrorCode::NotPrivateVault);
        require!(vault.admin == ctx.accounts.admin.key(), ErrorCode::Unauthorized);

        let whitelist = &mut ctx.accounts.whitelist;
        whitelist.addresses.push(address);

        Ok(())
    }

    // ----------------------------------------------------------
    // remove_from_whitelist
    // ----------------------------------------------------------
    pub fn remove_from_whitelist(
        ctx: Context<AdminOnly>,
        address: Pubkey,
    ) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        require!(vault.admin == ctx.accounts.admin.key(), ErrorCode::Unauthorized);
        let whitelist = &mut ctx.accounts.whitelist;
        whitelist.addresses.retain(|a| a != &address);
        Ok(())
    }

    // ----------------------------------------------------------
    // deposit
    // ----------------------------------------------------------
    // User deposits USDC into vault. Receives share tokens
    // proportional to current share price.
    // User pays gas.
    //
    // Share price = total_vault_assets / total_shares
    // Shares minted = deposit_amount / share_price
    //
    // For a fresh vault: share_price = 1.0 USDC (1:1 initial)
    // ----------------------------------------------------------
    pub fn deposit(
        ctx: Context<Deposit>,
        amount: u64,
    ) -> Result<()> {
        let vault = &mut ctx.accounts.vault;

        require!(vault.is_active, ErrorCode::VaultInactive);

        // Whitelist check for private vaults
        if vault.visibility == Visibility::Private {
            let whitelist = &ctx.accounts.whitelist;
            require!(
                whitelist.addresses.contains(&ctx.accounts.user.key()),
                ErrorCode::NotWhitelisted
            );
        }

        // Deposit cap check
        if vault.deposit_cap > 0 {
            require!(
                vault.solana_balance + amount <= vault.deposit_cap,
                ErrorCode::DepositCapExceeded
            );
        }

        // Calculate shares to mint
        // share_price (in micro-USDC) = total_assets / total_shares
        // If no shares exist yet, price = 1 USDC (1_000_000 micro)
        let shares_to_mint = if vault.total_shares == 0 {
            amount // 1:1 at genesis
        } else {
            let total_assets = vault.solana_balance + vault.arbitrum_balance;
            let share_price_micro = (total_assets as u128 * 1_000_000) / vault.total_shares as u128;
            ((amount as u128 * 1_000_000) / share_price_micro) as u64
        };

        // Transfer USDC from user to vault's USDC account
        token::transfer(
            ctx.accounts.into_transfer_to_vault_context(),
            amount,
        )?;

        // Mint share tokens to user
        token::mint_to(
            ctx.accounts.into_mint_shares_context(),
            shares_to_mint,
        )?;

        vault.solana_balance += amount;
        vault.total_shares   += shares_to_mint;

        // Record user's deposit for voting power tracking
        let user_position = &mut ctx.accounts.user_position;
        user_position.usdc_deposited += amount;
        user_position.shares_held    += shares_to_mint;

        emit!(DepositEvent {
            vault_id:      vault.vault_id,
            user:          ctx.accounts.user.key(),
            usdc_amount:   amount,
            shares_minted: shares_to_mint,
        });

        Ok(())
    }

    // ----------------------------------------------------------
    // withdraw
    // ----------------------------------------------------------
    // User burns share tokens and receives USDC.
    // If vault's Solana USDC balance is insufficient,
    // a bridging event is emitted for the relayer to
    // trigger CCTP (Arbitrum → Solana). Relayer pays bridge gas.
    // User pays gas for this withdraw transaction on Solana.
    //
    // Withdrawals blocked during active epoch (positions open).
    // ----------------------------------------------------------
    pub fn withdraw(
        ctx: Context<Withdraw>,
        shares_to_burn: u64,
    ) -> Result<()> {
        let vault = &mut ctx.accounts.vault;

        require!(!vault.epoch_active, ErrorCode::EpochActive);
        require!(shares_to_burn > 0, ErrorCode::ZeroShares);

        let user_position = &mut ctx.accounts.user_position;
        require!(user_position.shares_held >= shares_to_burn, ErrorCode::InsufficientShares);

        // Calculate USDC to return
        let total_assets = vault.solana_balance + vault.arbitrum_balance;
        let usdc_to_return = ((shares_to_burn as u128 * total_assets as u128)
            / vault.total_shares as u128) as u64;

        // Apply performance fee if vault has profited
        // (profit tracking via share_price delta from user's entry — simplified here)
        // Full implementation: track user's entry share price, compute profit delta
        let fee_amount = compute_performance_fee(usdc_to_return, vault.performance_fee);
        let usdc_after_fee = usdc_to_return - fee_amount;

        // Burn shares
        token::burn(
            ctx.accounts.into_burn_shares_context(),
            shares_to_burn,
        )?;

        vault.total_shares -= shares_to_burn;
        user_position.shares_held    -= shares_to_burn;
        user_position.usdc_deposited  = user_position.usdc_deposited.saturating_sub(
            (shares_to_burn * user_position.usdc_deposited) / (user_position.shares_held + shares_to_burn)
        );

        // Lazy bridging: check if Solana balance covers the withdrawal
        if vault.solana_balance >= usdc_after_fee {
            // Pay directly from Solana balance
            token::transfer(
                ctx.accounts.into_transfer_to_user_context(),
                usdc_after_fee,
            )?;
            vault.solana_balance -= usdc_after_fee;

            // Send performance fee to fee_address
            if fee_amount > 0 {
                token::transfer(
                    ctx.accounts.into_transfer_fee_context(),
                    fee_amount,
                )?;
            }
        } else {
            // Need to bridge from Arbitrum — emit event for relayer
            // Relayer will trigger CCTP bridge then re-execute payout
            let bridge_amount = usdc_after_fee - vault.solana_balance;
            emit!(BridgeRequiredEvent {
                vault_id:      vault.vault_id,
                direction:     BridgeDirection::ArbitrumToSolana,
                amount:        bridge_amount,
                trigger:       BridgeTrigger::Withdrawal,
                user:          ctx.accounts.user.key(),
                usdc_to_pay:   usdc_after_fee,
                fee_amount:    fee_amount,
                fee_address:   vault.fee_address,
            });
            // Set user's pending withdrawal so relayer can finalize
            user_position.pending_withdrawal_usdc = usdc_after_fee;
        }

        Ok(())
    }

    // ----------------------------------------------------------
    // finalize_withdrawal
    // ----------------------------------------------------------
    // Called by the relayer admin wallet after bridging is
    // complete. Completes a pending withdrawal.
    // ----------------------------------------------------------
    pub fn finalize_withdrawal(
        ctx: Context<RelayerOnly>,
        user: Pubkey,
        usdc_amount: u64,
        fee_amount: u64,
    ) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        require!(
            ctx.accounts.relayer.key() == vault.relayer_authority,
            ErrorCode::Unauthorized
        );

        let user_position = &mut ctx.accounts.user_position;
        require!(user_position.pending_withdrawal_usdc == usdc_amount, ErrorCode::AmountMismatch);

        token::transfer(ctx.accounts.into_transfer_to_user_context(), usdc_amount)?;
        vault.solana_balance -= usdc_amount;

        if fee_amount > 0 {
            token::transfer(ctx.accounts.into_transfer_fee_context(), fee_amount)?;
        }

        user_position.pending_withdrawal_usdc = 0;

        emit!(WithdrawalFinalizedEvent { vault_id: vault.vault_id, user, usdc_amount });

        Ok(())
    }

    // ----------------------------------------------------------
    // update_arbitrum_balance
    // ----------------------------------------------------------
    // Called by relayer to mirror the vault's Arbitrum USDC
    // balance on-chain (for share price accuracy).
    // ----------------------------------------------------------
    pub fn update_arbitrum_balance(
        ctx: Context<RelayerOnly>,
        new_balance: u64,
        epoch: u64,
    ) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        require!(
            ctx.accounts.relayer.key() == vault.relayer_authority,
            ErrorCode::Unauthorized
        );
        vault.arbitrum_balance = new_balance;
        vault.epoch            = epoch;

        emit!(BalanceUpdatedEvent {
            vault_id:          vault.vault_id,
            arbitrum_balance:  new_balance,
            epoch,
        });

        Ok(())
    }

    // ----------------------------------------------------------
    // trigger_prediction (Managed vaults)
    // ----------------------------------------------------------
    // Admin signs an off-chain message. The relayer verifies
    // the signature and calls this to record the pending
    // prediction on-chain, then triggers Arbitrum execution.
    // ----------------------------------------------------------
    pub fn record_managed_prediction(
        ctx: Context<RelayerOnly>,
        market_id: String,
        direction: Direction,
        usdc_amount: u64,
        admin_signature: [u8; 64],
    ) -> Result<()> {
        let vault = &ctx.accounts.vault;
        require!(vault.management_type == ManagementType::Managed, ErrorCode::WrongVaultType);

        // Verify admin signed this instruction off-chain
        let msg = build_prediction_message(&vault.vault_id, &market_id, direction, usdc_amount);
        verify_ed25519_signature(&vault.admin, &msg, &admin_signature)?;

        // Emit bridge event if Arbitrum balance is insufficient
        if vault.arbitrum_balance < usdc_amount {
            let bridge_delta = usdc_amount - vault.arbitrum_balance;
            emit!(BridgeRequiredEvent {
                vault_id:    vault.vault_id,
                direction:   BridgeDirection::SolanaToArbitrum,
                amount:      bridge_delta,
                trigger:     BridgeTrigger::Prediction,
                user:        Pubkey::default(),
                usdc_to_pay: 0,
                fee_amount:  0,
                fee_address: Pubkey::default(),
            });
        }

        emit!(PredictionQueuedEvent {
            vault_id:    vault.vault_id,
            market_id,
            direction,
            usdc_amount,
        });

        Ok(())
    }

    // ----------------------------------------------------------
    // record_crowdsourced_prediction
    // ----------------------------------------------------------
    // Called by the relayer after vote aggregation determines
    // a prediction should be placed for a crowdsourced vault.
    // Quorum and direction have been validated off-chain.
    // ----------------------------------------------------------
    pub fn record_crowdsourced_prediction(
        ctx: Context<RelayerOnly>,
        market_id: String,
        direction: Direction,
        usdc_amount: u64,
        vote_weight: u64,     // total vote weight that passed quorum
        total_shares: u64,    // for quorum verification on-chain
    ) -> Result<()> {
        let vault = &ctx.accounts.vault;
        require!(vault.management_type == ManagementType::Crowdsourced, ErrorCode::WrongVaultType);
        require!(
            ctx.accounts.relayer.key() == vault.relayer_authority,
            ErrorCode::Unauthorized
        );

        // On-chain quorum sanity check
        let quorum_bps = (vote_weight * 10_000) / total_shares;
        require!(quorum_bps >= vault.min_quorum as u64, ErrorCode::QuorumNotMet);

        // Risk cap: max 15% of total vault assets per market
        let total_assets = vault.solana_balance + vault.arbitrum_balance;
        let max_position = total_assets * 1500 / 10_000;
        require!(usdc_amount <= max_position, ErrorCode::PositionTooLarge);

        if vault.arbitrum_balance < usdc_amount {
            let bridge_delta = usdc_amount - vault.arbitrum_balance;
            emit!(BridgeRequiredEvent {
                vault_id:    vault.vault_id,
                direction:   BridgeDirection::SolanaToArbitrum,
                amount:      bridge_delta,
                trigger:     BridgeTrigger::Prediction,
                user:        Pubkey::default(),
                usdc_to_pay: 0,
                fee_amount:  0,
                fee_address: Pubkey::default(),
            });
        }

        emit!(PredictionQueuedEvent {
            vault_id:    vault.vault_id,
            market_id,
            direction,
            usdc_amount,
        });

        Ok(())
    }

    // ----------------------------------------------------------
    // update_vault_config
    // ----------------------------------------------------------
    // Admin can update mutable config parameters post-creation.
    // Cannot change vault_id, management_type, or share_mint.
    // ----------------------------------------------------------
    pub fn update_vault_config(
        ctx: Context<AdminOnly>,
        new_config: VaultConfigUpdate,
    ) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        require!(vault.admin == ctx.accounts.admin.key(), ErrorCode::Unauthorized);

        if let Some(cap) = new_config.deposit_cap       { vault.deposit_cap      = cap; }
        if let Some(fee) = new_config.performance_fee   {
            require!(fee <= 1000, ErrorCode::FeeTooHigh);
            vault.performance_fee = fee;
        }
        if let Some(addr) = new_config.fee_address      { vault.fee_address      = addr; }
        if let Some(q)    = new_config.min_quorum        { vault.min_quorum       = q; }

        Ok(())
    }
}

// ============================================================
// VAULT FACTORY — ACCOUNT STRUCTS
// ============================================================

#[account]
pub struct VaultAccount {
    pub admin:             Pubkey,
    pub vault_id:          [u8; 16],     // UUID
    pub visibility:        Visibility,
    pub management_type:   ManagementType,
    pub deposit_cap:       u64,          // 0 = unlimited, in micro-USDC
    pub performance_fee:   u16,          // basis points (0–1000)
    pub fee_address:       Pubkey,
    pub min_quorum:        u16,          // basis points of total shares
    pub total_shares:      u64,
    pub solana_balance:    u64,          // USDC held on Solana (micro-USDC)
    pub arbitrum_balance:  u64,          // mirrored from Arbitrum by relayer
    pub epoch:             u64,
    pub epoch_active:      bool,
    pub is_active:         bool,
    pub share_mint:        Pubkey,
    pub relayer_authority: Pubkey,
    pub bump:              u8,
}

#[account]
pub struct UserPosition {
    pub vault_id:                [u8; 16],
    pub user:                    Pubkey,
    pub usdc_deposited:          u64,
    pub shares_held:             u64,
    pub entry_share_price:       u64,    // snapshot for perf fee calculation
    pub pending_withdrawal_usdc: u64,
    pub bump:                    u8,
}

#[account]
pub struct Whitelist {
    pub vault_id:  [u8; 16],
    pub addresses: Vec<Pubkey>,          // up to 100 addresses (adjustable)
    pub bump:      u8,
}

// ============================================================
// VAULT FACTORY — ENUMS
// ============================================================

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq)]
pub enum Visibility       { Public, Private }

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq)]
pub enum ManagementType   { Managed, Crowdsourced }

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq)]
pub enum Direction        { Up, Down }

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum BridgeDirection  { SolanaToArbitrum, ArbitrumToSolana }

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum BridgeTrigger    { Prediction, Withdrawal }

// ============================================================
// VAULT FACTORY — EVENTS
// ============================================================

#[event] pub struct CreateVaultEvent      { pub vault_id: [u8; 16], pub admin: Pubkey, pub management_type: ManagementType, pub performance_fee: u16 }
#[event] pub struct DepositEvent          { pub vault_id: [u8; 16], pub user: Pubkey, pub usdc_amount: u64, pub shares_minted: u64 }
#[event] pub struct BridgeRequiredEvent   { pub vault_id: [u8; 16], pub direction: BridgeDirection, pub amount: u64, pub trigger: BridgeTrigger, pub user: Pubkey, pub usdc_to_pay: u64, pub fee_amount: u64, pub fee_address: Pubkey }
#[event] pub struct PredictionQueuedEvent { pub vault_id: [u8; 16], pub market_id: String, pub direction: Direction, pub usdc_amount: u64 }
#[event] pub struct BalanceUpdatedEvent   { pub vault_id: [u8; 16], pub arbitrum_balance: u64, pub epoch: u64 }
#[event] pub struct WithdrawalFinalizedEvent { pub vault_id: [u8; 16], pub user: Pubkey, pub usdc_amount: u64 }

// ============================================================
// VAULT FACTORY — ERROR CODES
// ============================================================

#[error_code]
pub enum ErrorCode {
    #[msg("Performance fee exceeds 10%")]          FeeTooHigh,
    #[msg("Quorum must be between 0 and 100%")]    InvalidQuorum,
    #[msg("Vault is not active")]                  VaultInactive,
    #[msg("Address not whitelisted")]              NotWhitelisted,
    #[msg("Deposit cap would be exceeded")]        DepositCapExceeded,
    #[msg("Cannot withdraw during active epoch")]  EpochActive,
    #[msg("Zero shares specified")]                ZeroShares,
    #[msg("Insufficient shares")]                  InsufficientShares,
    #[msg("Unauthorized caller")]                  Unauthorized,
    #[msg("Vault is not private")]                 NotPrivateVault,
    #[msg("Wrong vault management type")]          WrongVaultType,
    #[msg("Quorum not met")]                       QuorumNotMet,
    #[msg("Position size exceeds risk cap")]        PositionTooLarge,
    #[msg("Amount mismatch")]                      AmountMismatch,
}


// ============================================================
// 2. STAKING PROGRAM
// ============================================================
// Handles HIVE token staking, unbonding, and multiplier
// calculation. Multipliers are consumed by the Native Vault
// Program to compute blended voting power and share ownership.
// ============================================================

#[program]
mod staking_program {
    use super::*;

    const UNBONDING_PERIOD_SECONDS: i64 = 7 * 24 * 60 * 60; // 7 days
    const MULTIPLIER_CAP_TOKENS:    u64 = 10_000 * 1_000_000; // 10,000 HIVE (in micro-HIVE)
    const MAX_MULTIPLIER_BPS:       u64 = 2000;               // 2.0× = 200% in basis points

    // ----------------------------------------------------------
    // stake
    // ----------------------------------------------------------
    // Lock HIVE tokens. If StakeAccount doesn't exist, creates
    // it. Multiplier is updated immediately upon staking.
    // ----------------------------------------------------------
    pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
        require!(amount > 0, StakingError::ZeroAmount);

        token::transfer(
            ctx.accounts.into_transfer_to_stake_context(),
            amount,
        )?;

        let stake_account = &mut ctx.accounts.stake_account;
        stake_account.staker          = ctx.accounts.staker.key();
        stake_account.amount_staked  += amount;
        stake_account.stake_timestamp = Clock::get()?.unix_timestamp;
        stake_account.multiplier_bps  = compute_multiplier(stake_account.amount_staked);

        emit!(StakeEvent {
            staker:          stake_account.staker,
            amount_staked:   stake_account.amount_staked,
            multiplier_bps:  stake_account.multiplier_bps,
        });

        Ok(())
    }

    // ----------------------------------------------------------
    // initiate_unstake
    // ----------------------------------------------------------
    // Begins the 7-day unbonding period. Multiplier is lost
    // immediately — prevents epoch-boundary gaming.
    // ----------------------------------------------------------
    pub fn initiate_unstake(ctx: Context<InitiateUnstake>, amount: u64) -> Result<()> {
        let stake_account = &mut ctx.accounts.stake_account;
        require!(stake_account.staker == ctx.accounts.staker.key(), StakingError::Unauthorized);
        require!(amount <= stake_account.amount_staked, StakingError::InsufficientStake);
        require!(stake_account.unbonding_amount == 0, StakingError::UnbondingInProgress);

        stake_account.amount_staked   -= amount;
        stake_account.unbonding_amount = amount;
        stake_account.unbonding_since  = Clock::get()?.unix_timestamp;

        // Recompute multiplier immediately with reduced stake
        stake_account.multiplier_bps = compute_multiplier(stake_account.amount_staked);

        emit!(UnstakeInitiatedEvent {
            staker:           stake_account.staker,
            unbonding_amount: amount,
            available_at:     stake_account.unbonding_since + UNBONDING_PERIOD_SECONDS,
        });

        Ok(())
    }

    // ----------------------------------------------------------
    // complete_unstake
    // ----------------------------------------------------------
    // After unbonding period, return tokens to user wallet.
    // ----------------------------------------------------------
    pub fn complete_unstake(ctx: Context<CompleteUnstake>) -> Result<()> {
        let stake_account = &mut ctx.accounts.stake_account;
        require!(stake_account.staker == ctx.accounts.staker.key(), StakingError::Unauthorized);
        require!(stake_account.unbonding_amount > 0, StakingError::NothingToUnstake);

        let now = Clock::get()?.unix_timestamp;
        require!(
            now >= stake_account.unbonding_since + UNBONDING_PERIOD_SECONDS,
            StakingError::UnbondingNotComplete
        );

        let amount = stake_account.unbonding_amount;
        stake_account.unbonding_amount = 0;
        stake_account.unbonding_since  = 0;

        token::transfer(
            ctx.accounts.into_transfer_to_staker_context(),
            amount,
        )?;

        emit!(UnstakeCompletedEvent {
            staker: stake_account.staker,
            amount_returned: amount,
        });

        Ok(())
    }

    // ----------------------------------------------------------
    // get_multiplier (view helper — also callable on-chain)
    // ----------------------------------------------------------
    pub fn get_multiplier(ctx: Context<GetMultiplier>) -> Result<u64> {
        let stake_account = &ctx.accounts.stake_account;
        // If unbonding, multiplier is already reduced to remaining stake
        Ok(stake_account.multiplier_bps)
    }

    // ----------------------------------------------------------
    // INTERNAL: compute_multiplier
    // ----------------------------------------------------------
    // multiplier_bps = min(amount_staked / MULTIPLIER_CAP * MAX_BPS, MAX_BPS)
    // Returns basis points (e.g. 1000 bps = 1.0×, 2000 bps = 2.0×)
    // ----------------------------------------------------------
    fn compute_multiplier(amount_staked: u64) -> u16 {
        let multiplier = (amount_staked as u128 * MAX_MULTIPLIER_BPS as u128)
            / MULTIPLIER_CAP_TOKENS as u128;
        multiplier.min(MAX_MULTIPLIER_BPS as u128) as u16
    }
}

// ============================================================
// STAKING PROGRAM — ACCOUNT STRUCTS
// ============================================================

#[account]
pub struct StakeAccount {
    pub staker:            Pubkey,
    pub amount_staked:     u64,        // active stake (micro-HIVE)
    pub stake_timestamp:   i64,
    pub unbonding_amount:  u64,        // tokens in unbonding
    pub unbonding_since:   i64,        // unix timestamp unbonding started
    pub multiplier_bps:    u16,        // current effective multiplier (basis points)
    pub bump:              u8,
}

// ============================================================
// STAKING PROGRAM — EVENTS & ERRORS
// ============================================================

#[event] pub struct StakeEvent            { pub staker: Pubkey, pub amount_staked: u64, pub multiplier_bps: u16 }
#[event] pub struct UnstakeInitiatedEvent { pub staker: Pubkey, pub unbonding_amount: u64, pub available_at: i64 }
#[event] pub struct UnstakeCompletedEvent { pub staker: Pubkey, pub amount_returned: u64 }

#[error_code]
pub enum StakingError {
    #[msg("Amount must be greater than zero")]    ZeroAmount,
    #[msg("Insufficient staked balance")]         InsufficientStake,
    #[msg("Unbonding already in progress")]       UnbondingInProgress,
    #[msg("Unbonding period not yet complete")]   UnbondingNotComplete,
    #[msg("Nothing to unstake")]                  NothingToUnstake,
    #[msg("Unauthorized")]                        Unauthorized,
}


// ============================================================
// 3. NATIVE VAULT PROGRAM
// ============================================================
// Extends vault mechanics with blended USDC + staked HIVE
// voting power and share ownership. Also handles the HIVE
// fee drip into the vault bankroll.
// ============================================================

#[program]
mod native_vault {
    use super::*;

    const MIN_DEPOSIT_FOR_VOTING: u64 = 10_000_000; // 10 USDC (micro-USDC)

    // ----------------------------------------------------------
    // deposit_native
    // ----------------------------------------------------------
    // Deposit USDC into the native vault. Blended weight is
    // calculated using USDC deposited × (1 + multiplier).
    // Share % is based on blended weight across all users.
    // User pays gas.
    // ----------------------------------------------------------
    pub fn deposit_native(ctx: Context<NativeDeposit>, usdc_amount: u64) -> Result<()> {
        // Fetch staking multiplier for depositor from StakeAccount PDA
        let multiplier_bps = ctx.accounts.stake_account.multiplier_bps as u64;

        // blended_weight = usdc_amount × (1 + multiplier)
        // multiplier_bps=1000 means 1.0, so: usdc × (10_000 + bps) / 10_000
        let blended_weight = (usdc_amount as u128
            * (10_000 + multiplier_bps) as u128
            / 10_000) as u64;

        // Transfer USDC
        token::transfer(
            ctx.accounts.into_transfer_to_vault_context(),
            usdc_amount,
        )?;

        let vault = &mut ctx.accounts.native_vault_state;
        let user_state = &mut ctx.accounts.native_user_state;

        // Update user state
        user_state.usdc_deposited    += usdc_amount;
        user_state.blended_weight     = blended_weight; // recalculated on each deposit/stake change
        user_state.can_vote           = usdc_amount >= MIN_DEPOSIT_FOR_VOTING;

        // Update vault totals
        vault.total_usdc_deposited   += usdc_amount;
        vault.total_blended_weight   += blended_weight;
        vault.solana_balance         += usdc_amount;

        // Mint native share tokens proportional to blended weight
        let shares_to_mint = if vault.total_native_shares == 0 {
            blended_weight
        } else {
            ((blended_weight as u128 * vault.total_native_shares as u128)
                / vault.total_blended_weight as u128) as u64
        };

        token::mint_to(ctx.accounts.into_mint_native_shares_context(), shares_to_mint)?;
        vault.total_native_shares    += shares_to_mint;
        user_state.native_shares_held += shares_to_mint;

        emit!(NativeDepositEvent {
            user:           ctx.accounts.user.key(),
            usdc_amount,
            blended_weight,
            shares_minted:  shares_to_mint,
            can_vote:       user_state.can_vote,
        });

        Ok(())
    }

    // ----------------------------------------------------------
    // refresh_blended_weight
    // ----------------------------------------------------------
    // Called by user (or relayer at epoch boundary) to update
    // a user's blended weight after staking/unstaking HIVE.
    // Share recalculation applies at next epoch boundary.
    // ----------------------------------------------------------
    pub fn refresh_blended_weight(ctx: Context<RefreshWeight>) -> Result<()> {
        let user_state    = &mut ctx.accounts.native_user_state;
        let stake_account = &ctx.accounts.stake_account;
        let vault         = &mut ctx.accounts.native_vault_state;

        let old_weight     = user_state.blended_weight;
        let multiplier_bps = stake_account.multiplier_bps as u64;
        let new_weight     = (user_state.usdc_deposited as u128
            * (10_000 + multiplier_bps) as u128
            / 10_000) as u64;

        // Update vault's total blended weight
        vault.total_blended_weight = vault.total_blended_weight
            .saturating_sub(old_weight)
            .saturating_add(new_weight);

        user_state.blended_weight  = new_weight;
        user_state.pending_refresh = false;

        emit!(WeightRefreshedEvent {
            user:       ctx.accounts.user.key(),
            old_weight,
            new_weight,
            multiplier_bps: multiplier_bps as u16,
        });

        Ok(())
    }

    // ----------------------------------------------------------
    // receive_fee_drip
    // ----------------------------------------------------------
    // Called by the protocol admin wallet (or a permissioned
    // drip CPI) to deposit HIVE fee proceeds (converted to
    // USDC) into the native vault bankroll.
    // Relayer pays gas.
    // ----------------------------------------------------------
    pub fn receive_fee_drip(
        ctx: Context<FeeDrip>,
        usdc_amount: u64,
    ) -> Result<()> {
        require!(
            ctx.accounts.admin.key() == ctx.accounts.native_vault_state.protocol_admin,
            NativeVaultError::Unauthorized
        );

        token::transfer(
            ctx.accounts.into_transfer_to_vault_context(),
            usdc_amount,
        )?;

        let vault = &mut ctx.accounts.native_vault_state;
        vault.solana_balance        += usdc_amount;
        vault.total_fee_drip_received += usdc_amount;

        emit!(FeeDripEvent {
            usdc_amount,
            new_solana_balance: vault.solana_balance,
        });

        Ok(())
    }

    // ----------------------------------------------------------
    // get_voting_power (view)
    // ----------------------------------------------------------
    pub fn get_voting_power(ctx: Context<GetVotingPower>) -> Result<u64> {
        let user_state = &ctx.accounts.native_user_state;
        if !user_state.can_vote {
            return Ok(0);
        }
        Ok(user_state.blended_weight)
    }

    // ----------------------------------------------------------
    // get_share_pct (view — returns bps of total shares)
    // ----------------------------------------------------------
    pub fn get_share_pct(ctx: Context<GetSharePct>) -> Result<u64> {
        let vault      = &ctx.accounts.native_vault_state;
        let user_state = &ctx.accounts.native_user_state;
        if vault.total_native_shares == 0 { return Ok(0); }
        let bps = (user_state.native_shares_held as u128 * 10_000)
            / vault.total_native_shares as u128;
        Ok(bps as u64)
    }
}

// ============================================================
// NATIVE VAULT — ACCOUNT STRUCTS
// ============================================================

#[account]
pub struct NativeVaultState {
    pub protocol_admin:          Pubkey,
    pub relayer_authority:       Pubkey,
    pub share_mint:              Pubkey,
    pub usdc_vault:              Pubkey,       // USDC token account
    pub total_usdc_deposited:    u64,
    pub total_blended_weight:    u64,
    pub total_native_shares:     u64,
    pub solana_balance:          u64,
    pub arbitrum_balance:        u64,
    pub epoch:                   u64,
    pub epoch_active:            bool,
    pub total_fee_drip_received: u64,
    pub bump:                    u8,
}

#[account]
pub struct NativeUserState {
    pub user:                  Pubkey,
    pub usdc_deposited:        u64,
    pub blended_weight:        u64,
    pub native_shares_held:    u64,
    pub can_vote:              bool,
    pub pending_refresh:       bool,       // set true when stake changes mid-epoch
    pub entry_share_price:     u64,
    pub pending_withdrawal:    u64,
    pub bump:                  u8,
}

// ============================================================
// NATIVE VAULT — EVENTS & ERRORS
// ============================================================

#[event] pub struct NativeDepositEvent  { pub user: Pubkey, pub usdc_amount: u64, pub blended_weight: u64, pub shares_minted: u64, pub can_vote: bool }
#[event] pub struct FeeDripEvent        { pub usdc_amount: u64, pub new_solana_balance: u64 }
#[event] pub struct WeightRefreshedEvent { pub user: Pubkey, pub old_weight: u64, pub new_weight: u64, pub multiplier_bps: u16 }

#[error_code]
pub enum NativeVaultError {
    #[msg("Unauthorized")]        Unauthorized,
    #[msg("Minimum deposit not met for voting")] MinDepositNotMet,
}


// ============================================================
// SHARED HELPERS
// ============================================================

fn compute_performance_fee(usdc_amount: u64, fee_bps: u16) -> u64 {
    // Only charge on profit above entry price (simplified: applied to full amount here)
    // Full implementation should compare against entry share price
    (usdc_amount as u128 * fee_bps as u128 / 10_000) as u64
}

fn build_prediction_message(
    vault_id: &[u8; 16],
    market_id: &str,
    direction: Direction,
    amount: u64,
) -> Vec<u8> {
    // Deterministic serialization of prediction params for signature verification
    let mut msg = Vec::new();
    msg.extend_from_slice(vault_id);
    msg.extend_from_slice(market_id.as_bytes());
    msg.push(direction as u8);
    msg.extend_from_slice(&amount.to_le_bytes());
    msg
}

fn verify_ed25519_signature(
    pubkey: &Pubkey,
    message: &[u8],
    signature: &[u8; 64],
) -> Result<()> {
    // Use Solana's native ed25519 program for signature verification
    // In practice, use anchor's `Instructions` sysvar or ed25519 program CPI
    // Pseudocode — actual implementation uses solana_program::ed25519_program
    let is_valid = ed25519_verify(pubkey.as_ref(), message, signature);
    require!(is_valid, ErrorCode::Unauthorized);
    Ok(())
}