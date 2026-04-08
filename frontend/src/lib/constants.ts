/** Solana cluster for wallet + RPC (override via env in production). */
export const SOLANA_CLUSTER =
  import.meta.env.VITE_SOLANA_CLUSTER ?? 'devnet'

/** Placeholder program IDs — replace after deployment. */
export const PROGRAM_IDS = {
  vaultFactory: import.meta.env.VITE_VAULT_FACTORY_PROGRAM_ID ?? '',
  nativeVault: import.meta.env.VITE_NATIVE_VAULT_PROGRAM_ID ?? '',
  staking: import.meta.env.VITE_STAKING_PROGRAM_ID ?? '',
} as const
