const base = import.meta.env.VITE_API_BASE_URL ?? 'http://localhost:3000'

export async function apiGet<T>(path: string): Promise<T> {
  const res = await fetch(`${base}${path}`)
  if (!res.ok) {
    throw new Error(`${res.status} ${res.statusText}`)
  }
  return res.json() as Promise<T>
}

export function getApiBaseUrl(): string {
  return base
}
