import { useEffect, useState } from 'react'
import { apiGet } from '../api/client'

type Health = { status: string }

export function useBackendHealth() {
  const [health, setHealth] = useState<Health | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    apiGet<Health>('/health')
      .then((h) => {
        if (!cancelled) setHealth(h)
      })
      .catch((e: Error) => {
        if (!cancelled) setError(e.message)
      })
    return () => {
      cancelled = true
    }
  }, [])

  return { health, error }
}
