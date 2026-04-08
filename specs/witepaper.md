# HiveCall: Crowdsourced TCG Prediction Markets Across Chains

**Version 0.1 — Draft**
**April 2026**

---

## Abstract

HiveCall is a decentralized protocol built on Solana that enables users to create and participate in crowdsourced prediction vaults targeting Trading Card Game (TCG) price markets on CardChase (Arbitrum). Through a combination of on-chain vault mechanics, off-chain vote aggregation, and native USDC bridging via Circle's Cross-Chain Transfer Protocol (CCTP), HiveCall allows communities to pool capital and collective intelligence to take positions on TCG card price movements — without requiring participants to manage cross-chain complexity themselves.

The protocol introduces a native token (HIVE) whose staking mechanics govern enhanced voting power and share ownership within HiveCall's own native vault. A 1% creator fee on all HIVE token transactions continuously drips back into the native vault, creating a self-reinforcing flywheel between token activity, vault bankroll growth, and participant returns.

---

## 1. Introduction

### 1.1 The TCG Prediction Market Opportunity

Trading Card Games — Magic: The Gathering, Pokémon, One Piece, and others — represent a multi-billion dollar collectibles economy with highly volatile, event-driven price dynamics. Card prices shift dramatically around set releases, ban list announcements, tournament results, and reprint news. This volatility creates a natural prediction market opportunity for participants with deep TCG domain knowledge.

CardChase, a prediction market protocol deployed on Arbitrum, enables participants to take directional positions on TCG card prices using USDC. However, individual participation requires cross-chain capital management, significant market research, and carries the full risk of individual decision-making.

### 1.2 The Crowdsourcing Advantage

HiveCall's core insight is that TCG communities are already highly opinionated and well-informed. Discord servers, subreddits, and trading communities constantly discuss price trajectories of cards before they move. This collective intelligence is currently informal and uncaptured. HiveCall formalizes it: vault shareholders vote on market direction, and the aggregated signal drives capital deployment.

### 1.3 Design Goals

- **Permissionless vault creation** — anyone can launch a prediction vault with flexible configuration
- **Abstracted cross-chain UX** — users interact only with Solana; CCTP bridging is invisible
- **Flexible governance** — vaults can be managed (admin-controlled) or crowdsourced (shareholder-voted)
- **Aligned incentives** — the native token creates a flywheel between trading activity and vault performance
- **Capital efficiency** — lazy bridging ensures capital works on Arbitrum until withdrawals demand otherwise

---

## 2. Protocol Architecture

### 2.1 High-Level Overview

HiveCall operates across two chains:

- **Solana** — user-facing layer. Vault creation, deposits, withdrawals, share token issuance, staking, and off-chain vote signing all occur here.
- **Arbitrum** — execution layer. Each vault has a corresponding smart contract on Arbitrum that interfaces directly with CardChase to open and close prediction positions.

**CCTP (Circle Cross-Chain Transfer Protocol)** handles all USDC movement between chains. Because CardChase settles in native USDC, CCTP's burn-and-mint mechanism ensures HiveCall always operates with native — not wrapped — USDC on both chains.

**Off-Chain Relayer Backend** aggregates votes, checks quorum conditions, and relays prediction trigger transactions. The relayer's admin wallet pays gas for all relayed operations on both Solana and Arbitrum. Users only pay gas for their own deposit and withdrawal transactions on Solana.

### 2.2 Component Map

```
┌─────────────────────────────────────────────────────────┐
│                        SOLANA                           │
│                                                         │
│  ┌──────────────────┐    ┌───────────────────────────┐  │
│  │  Vault Factory   │    │     Staking Program       │  │
│  │  Program         │    │  (HIVE token staking,     │  │
│  │  (create, deposit│    │   multipliers, unbonding) │  │
│  │   withdraw,      │    └───────────────────────────┘  │
│  │   share tokens)  │                                   │
│  └──────────────────┘    ┌───────────────────────────┐  │
│                          │    Native Vault Program   │  │
│  ┌──────────────────┐    │  (USDC + staked HIVE      │  │
│  │   HIVE Token     │    │   blended share/vote      │  │
│  │   (SPL, 1% fee   │    │   mechanics, fee drip)    │  │
│  │    drip to vault)│    └───────────────────────────┘  │
│  └──────────────────┘                                   │
└────────────────────────────┬────────────────────────────┘
                             │ CCTP (native USDC)
                             │ + Relayer (instructions)
┌────────────────────────────▼────────────────────────────┐
│                       ARBITRUM                          │
│                                                         │
│  ┌──────────────────┐    ┌───────────────────────────┐  │
│  │  Vault Executor  │    │   CardChase Protocol      │  │
│  │  (per vault,     │◄───┤   (TCG prediction         │  │
│  │   operator-gated │    │    markets, USDC)         │  │
│  │   position mgmt) │    └───────────────────────────┘  │
│  └──────────────────┘                                   │
└─────────────────────────────────────────────────────────┘
```

---

## 3. Vault System

### 3.1 Vault Creation

Any user can create a vault by invoking the Vault Factory Program on Solana. Vault creation requires the user to pay a Solana transaction fee. Upon creation, the relayer detects the on-chain event and deploys a corresponding Vault Executor contract on Arbitrum, with the vault admin's key registered as the authorized operator.

**Vault Configuration Parameters:**

| Parameter | Description |
|---|---|
| `name` | Human-readable vault identifier |
| `visibility` | `Public` (anyone deposits) or `Private` (whitelist only) |
| `management_type` | `Managed` (admin picks positions) or `Crowdsourced` (shareholders vote) |
| `deposit_cap` | Maximum total USDC the vault accepts (0 = unlimited) |
| `performance_fee` | 0–10%, taken from profits at epoch settlement |
| `fee_address` | Wallet that receives performance fees |
| `whitelist` | Array of Solana pubkeys (Private vaults only) |
| `min_quorum` | Minimum % of vault shares that must vote on a market for a prediction to be placed (Crowdsourced only) |

### 3.2 Managed Vaults

In a managed vault, the admin signs off-chain messages specifying:
- Target CardChase market
- Direction (UP or DOWN)
- Position size in USDC

The relayer verifies the admin's signature and executes the prediction on Arbitrum. Shareholders deposit USDC and receive share tokens representing their proportional ownership. Profits and losses are distributed pro-rata to share token holders at epoch settlement.

### 3.3 Crowdsourced Vaults

Crowdsourced vaults aggregate shareholder opinion to drive capital deployment.

**Voting Mechanics:**
- Each shareholder's voting power equals their USDC deposit amount
- Votes are cast per market: each user can vote on as many open markets as they wish, using their full voting power on each
- Votes are signed off-chain messages — free for users, no gas required
- The relayer aggregates votes per market and evaluates quorum + direction

**Prediction Trigger Conditions:**
1. Total vote weight on a market ≥ vault's `min_quorum` threshold (as % of total shares)
2. One direction (UP or DOWN) holds a strict majority of votes cast on that market

If both conditions are met, the relayer triggers a prediction. Position size is proportional to the share of vault TVL that voted for the market, subject to a protocol-level risk cap (maximum 15% of vault TVL per single market).

**Quorum Failure:**
If quorum is not met by the epoch deadline, no prediction is placed for that market. Undeployed capital remains in the vault's Solana balance.

### 3.4 Share Token Model

Upon depositing USDC, users receive vault share tokens (SPL tokens unique to each vault). Share tokens represent proportional ownership of vault assets. This model:

- Automatically accounts for vault performance without touching individual balances
- Enables clean pro-rata distribution of gains and losses
- Allows share tokens to potentially be composable with other Solana DeFi protocols in future versions

Share price starts at 1.00 USDC and moves with vault performance. A user holding 5% of total shares owns 5% of all vault assets across both Solana and Arbitrum.

### 3.5 Capital Flow & Lazy Bridging

**Deposits:**
User deposits USDC on Solana → receives share tokens → gas paid by user.

**Prediction Execution:**
When a prediction is triggered:
1. Check vault's current USDC balance on Arbitrum
2. If sufficient → execute directly from Arbitrum balance
3. If insufficient → bridge the delta from Solana to Arbitrum via CCTP (burn on Solana, mint on Arbitrum), then execute

**Winnings:**
Settled winnings accumulate on Arbitrum. They remain there until needed, acting as a reserve for future predictions without requiring round-trip bridging.

**Withdrawals:**
User requests withdrawal on Solana → burns share tokens:
1. Check vault's USDC balance on Solana
2. If sufficient → pay directly
3. If insufficient → bridge delta from Arbitrum to Solana via CCTP, then pay
User pays Solana gas for the withdrawal transaction. Bridging gas (if triggered) is paid by the protocol admin wallet.

---

## 4. Native Protocol Vault

### 4.1 Overview

HiveCall operates its own first-party vault alongside user-created vaults. The native vault shares the same prediction mechanics as crowdsourced user vaults but introduces a blended voting power and share ownership model that incorporates staked HIVE tokens alongside USDC deposits.

**Minimum deposit to participate in governance: 10 USDC.**
Deposits below 10 USDC are accepted and earn returns but carry no voting rights.

### 4.2 Blended Voting Power

```
Voting Power = USDC_deposited × (1 + Staking_Multiplier)

Staking_Multiplier = min(tokens_staked / 10,000, 2.0)
```

| Tokens Staked | Multiplier | Effective Voting Power (per 1,000 USDC) |
|---|---|---|
| 0 | 0.0× | 1,000 |
| 1,000 | 0.1× | 1,100 |
| 2,500 | 0.25× | 1,250 |
| 5,000 | 0.5× | 1,500 |
| 10,000 | 2.0× | 3,000 (cap) |

### 4.3 Blended Share Ownership

Share ownership in the native vault follows the same blended formula:

```
User Share % = User_Blended_Weight / Σ(All_Users_Blended_Weight)

Blended_Weight = USDC_deposited × (1 + Staking_Multiplier)
```

A staker with the same USDC deposit as a non-staker earns a larger share of vault profits. Share percentages are recalculated at epoch boundaries, not in real time, to prevent gaming around stake/unstake timing.

### 4.4 Fee Drip Mechanism

The HIVE token carries a 1% creator fee on all transactions, collected on-chain. These fees accumulate and are periodically (daily) swapped to USDC and deposited into the native vault's Solana balance. This:

- Grows the native vault's bankroll proportionally to HIVE trading volume
- Distributes value to all native vault participants (depositors and stakers)
- Creates a flywheel: token activity → larger bankroll → better prediction capacity → stronger returns → more depositors → greater HIVE demand

---

## 5. HIVE Token

### 5.1 Launch & Initial Distribution

HIVE launches on bags.fm (Solana). Token distribution:

| Allocation | % | Vesting |
|---|---|---|
| Community / Rewards Pool | 40% | 4-year epoch-based emission |
| Treasury | 20% | DAO-governed |
| Team | 15% | 1-year cliff, 3-year linear |
| Ecosystem / Partnerships | 10% | Milestone-based |
| Initial Liquidity | 10% | Locked LP at launch |
| Seed / Angel | 5% | 6-month cliff, 2-year linear |

### 5.2 Token Utility

- **Staking** — stake HIVE to boost voting power and share ownership in the native vault
- **Fee generation** — 1% creator fee on all HIVE transactions drips back into the native vault
- **Governance** — future protocol parameter changes governed by staked HIVE holders

### 5.3 Value Accrual

HIVE derives value from its role in the native vault: staking HIVE increases a holder's share of vault profits. The more profitable the native vault, the more valuable staking HIVE becomes. Combined with the fee drip growing the vault bankroll, there is a structural link between protocol usage and token value — without relying on inflationary emissions as the primary incentive.

---

## 6. Staking Program

### 6.1 Mechanics

Users stake HIVE tokens into the Staking Program (a dedicated Solana program). Staked tokens are locked and cannot be transferred. Unstaking initiates a 7-day unbonding period, during which the staking multiplier is immediately lost. Tokens are returned to the user's wallet after the unbonding period completes.

The 7-day unbonding period prevents epoch-boundary gaming (staking just before settlement to capture boosted shares, then immediately unstaking).

### 6.2 Stake Account Structure

Each user's staking position is tracked in a PDA-derived `StakeAccount`:
- Staker public key
- Amount currently staked
- Stake timestamp
- Unbonding amount and unbonding start timestamp (if in progress)
- Cached current multiplier (updated lazily on interaction)

---

## 7. Off-Chain Infrastructure

### 7.1 Relayer Backend

The relayer is the protocol's trusted off-chain component in V1. It is responsible for:

1. **Monitoring** on-chain vault state on Solana and CardChase market states on Arbitrum
2. **Collecting votes** — users submit signed off-chain messages; the relayer verifies signatures against on-chain share balances
3. **Aggregating votes** — per vault, per market, weighted by voting power
4. **Evaluating quorum** — checking if the quorum + directional majority conditions are met
5. **Relaying predictions** — submitting trigger transactions on Solana (state update) and Arbitrum (CardChase position entry), with gas paid by the protocol admin wallet
6. **Settlement monitoring** — detecting CardChase market resolutions and updating vault balances on-chain
7. **Fee drip execution** — periodically routing accumulated HIVE creator fees → USDC → native vault

The relayer's vote aggregation logic is open-source and publicly verifiable. Full decentralization of the relayer is a V3 milestone.

### 7.2 Trust Assumptions (V1)

- Relayer correctly aggregates votes and does not censor or alter them
- Relayer admin wallet is secured (multisig recommended)
- Large cross-chain transactions (>$50,000) require a 24-hour time-lock before execution

---

## 8. Risk Management

### 8.1 Position Limits

- Maximum 15% of vault TVL in any single CardChase market
- Maximum 10% of a CardChase market's open interest (prevents vault from moving the market it is predicting)

### 8.2 Insurance Fund

5% of all performance fees across all vaults accumulates into a protocol-level insurance fund. If a vault's epoch return falls below -5%, the insurance fund partially covers losses up to its available balance. The fund targets a reserve of 3% of total protocol TVL before overflow redirects to treasury.

### 8.3 Smart Contract Risk Mitigation

- Both Solana programs and Arbitrum contracts audited before mainnet launch
- TVL caps enforced per vault and at protocol level during initial phases
- Emergency pause functions on all programs and contracts, controlled by a multisig
- Bug bounty program active from day one

### 8.4 Bridge Risk

CCTP is used exclusively for USDC movement, eliminating wrapped asset risk. A separate lightweight messaging layer (Wormhole or a centralized relayer) passes instructions only — no value flows through it. All cross-chain instruction execution requires operator signature verification on the Arbitrum side.

---

## 9. Roadmap

| Phase | Key Deliverables |
|---|---|
| **V1 — Foundation** | HIVE token launch on bags.fm, native vault with manual admin, staking program, CCTP integration, basic frontend |
| **V2 — Vault Factory** | Permissionless vault creation (managed + crowdsourced), off-chain voting backend, relayer live, lazy bridging |
| **V3 — Full Protocol** | Complete frontend (vault discovery, voting UI, account profiles), insurance fund, fee drip automation |
| **V4 — Decentralization** | Relayer decentralization, on-chain reputation scores, DAO governance for treasury and protocol parameters |

---

## 10. Conclusion

HiveCall captures an underserved opportunity: TCG communities already possess deep, time-sensitive market intelligence that currently has no structured outlet. By combining permissionless vault creation, crowdsourced prediction mechanics, CCTP-native cross-chain USDC flows, and a token flywheel tied to the native vault, HiveCall turns collective TCG expertise into structured, on-chain capital allocation — accessible to anyone on Solana.

---

*This document is a working draft and subject to change. Nothing herein constitutes financial or investment advice.*