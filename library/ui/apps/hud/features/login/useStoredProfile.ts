// A stored profile as React state, asked for again while a just-made profile syncs.
import { useEffect, useState } from 'react'
import { fetchStoredProfile, type StoredProfile } from '@explorer-core/features/login/storedProfile'

const RETRY_MS = 2_000
/** `address`: as for fetchStoredProfile. `waitForMs`: how long a missing profile is asked for again
 *  before that answer settles (a profile just made elsewhere, still syncing between catalysts). */
export function useStoredProfile(address?: string, waitForMs = 0): StoredProfile {
  // keyed by address, so a change of address never reads as the previous one's profile
  const [state, setState] = useState<{ address?: string; data: StoredProfile }>({ data: {} })
  useEffect(() => {
    if (address == null) return
    let cancelled = false
    let retry: ReturnType<typeof setTimeout> | undefined
    const started = Date.now()
    const ask = (): void => {
      void fetchStoredProfile(address).then((data) => {
        if (cancelled) return
        if (data?.exists === false && Date.now() - started < waitForMs) {
          retry = setTimeout(ask, RETRY_MS)
          return
        }
        setState({ address, data: { ...data, settled: true } })
      })
    }
    ask()
    return () => {
      cancelled = true
      clearTimeout(retry)
    }
  }, [address, waitForMs])
  return state.address === address ? state.data : EMPTY
}

const EMPTY: StoredProfile = {}
