import { PROGRAM_IDS, SOLANA_CLUSTER } from '../lib/constants'

export function NativeVaultPage() {
  return (
    <section>
      <h1>Native protocol vault</h1>
      <p>Staking, blended voting power, and fee drip UI will live here.</p>
      <ul className="meta">
        <li>
          Cluster: <code>{SOLANA_CLUSTER}</code>
        </li>
        <li>
          Native vault program: <code>{PROGRAM_IDS.nativeVault || 'unset'}</code>
        </li>
      </ul>
    </section>
  )
}
