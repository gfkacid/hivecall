# HiveCall

Monorepo for the HiveCall protocol: Solana vaults, Arbitrum executors, relayer backend, and app UI. Protocol behavior is described in [specs/protocol_spec.md](specs/protocol_spec.md) and [specs/witepaper.md](specs/witepaper.md).

## Layout

| Path | Stack | Role |
|------|--------|------|
| [backend/](backend/) | NestJS, Prisma, MySQL | Relayer API, off-chain votes, webhook ingest for chain events |
| [frontend/](frontend/) | Vite, React, TypeScript | Wallet UX, vault discovery, voting UI (scaffold) |
| [contracts/](contracts/) | Foundry, Solidity | `VaultExecutor`, factory, registry on Arbitrum |
| [programs/](programs/) | Anchor (Rust) | `vault-factory`, `native-vault`, `staking` on Solana |
| [specs/](specs/) | Docs / pseudocode | Source of truth for implementation |

## Backend

```bash
cd backend
cp .env.example .env
# Set DATABASE_URL, WEBHOOK_INGEST_SECRET, then:
npx prisma generate
npx prisma migrate deploy
npm run start:dev
```

- Health: `GET http://localhost:3000/health`
- Chain webhook ingest: `POST http://localhost:3000/webhooks/chain-events` with header `x-webhook-secret: <WEBHOOK_INGEST_SECRET>` and JSON body `{ "chain", "dedupeKey", "source", "payload" }`.

Prisma is pinned to v6 for a standard `DATABASE_URL` in `schema.prisma`.

## Frontend

```bash
cd frontend
cp .env.example .env.local
npm run dev
```

## Contracts (Arbitrum / Foundry)

```bash
cd contracts
forge build
forge test
# Deploy (set PRIVATE_KEY and RPC):
# forge script script/Deploy.s.sol --rpc-url arbitrum_sepolia --broadcast
```

OpenZeppelin v5 is vendored under `contracts/lib/openzeppelin-contracts`.

## Programs (Solana / Anchor)

```bash
cd programs
# Install [Anchor](https://www.anchor-lang.com/docs/installation), then:
anchor build
anchor test
```

The workspace matches `Anchor.toml` (`vault-factory`, `native-vault`, `staking`). Program IDs in `Anchor.toml` are dev placeholders; replace with your deploy keys before mainnet.

If the Anchor CLI is not installed, you can still run `cargo check` inside `programs/` with a normal Rust toolchain.

## Indexing (no custom indexer)

Use hosted providers and POST normalized events to the backend webhook:

- **Solana** (vault txs, program accounts): e.g. [Helius](https://helius.dev) webhooks or enhanced APIs.
- **Arbitrum** (executor events): e.g. [Alchemy](https://alchemy.com) Notify / address activity webhooks.

Use a stable `dedupeKey` per event (e.g. signature + log index) so retries stay idempotent in `ChainEvent`.
