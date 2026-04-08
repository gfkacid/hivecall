import { useBackendHealth } from '../hooks/useBackendHealth'
import { getApiBaseUrl } from '../api/client'

export function HomePage() {
  const { health, error } = useBackendHealth()

  return (
    <section>
      <h1>Vault discovery</h1>
      <p>Public vault grid and filters will live here.</p>
      <p className="meta">
        API: <code>{getApiBaseUrl()}</code>
        {health && (
          <>
            {' '}
            — backend <code>{health.status}</code>
          </>
        )}
        {error && (
          <>
            {' '}
            — <span className="error">offline ({error})</span>
          </>
        )}
      </p>
    </section>
  )
}
