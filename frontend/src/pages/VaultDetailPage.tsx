import { useParams } from 'react-router-dom'

export function VaultDetailPage() {
  const { vaultId } = useParams<{ vaultId: string }>()

  return (
    <section>
      <h1>Vault</h1>
      <p>
        Detail, voting, deposit/withdraw for vault{' '}
        <code>{vaultId ?? '—'}</code>.
      </p>
    </section>
  )
}
