// The scene outlives logout, so per-account caches register a reset for when another account logs in.

export type IdentityHub = {
  onChange: (reset: () => void) => void
  /** A guard for a pending reply. Welcome can acquire its first player after the request starts. */
  session: (readAddress: () => string | null) => () => boolean
  logout: () => void
  /** Feed the current address; resets run when it differs from the last one seen. */
  observe: (address: string | null) => void
}

export function createIdentityHub(): IdentityHub {
  const resets = new Set<() => void>()
  let last: string | null | undefined
  let generation = 0
  let active: string | null = null
  const observeSession = (address: string | null): void => {
    if (active !== null && active !== address) generation++
    active = address
  }
  return {
    session: (readAddress) => {
      observeSession(readAddress())
      const started = generation
      return () => {
        observeSession(readAddress())
        return started === generation
      }
    },
    logout: () => {
      generation++
      active = null
    },
    onChange: (reset) => {
      resets.add(reset)
    },
    observe: (address) => {
      observeSession(address)
      if (last === undefined || address === null) {
        if (address !== null) last = address
        return
      }
      if (address === last) return
      last = address
      for (const reset of resets) {
        try {
          reset()
        } catch (e) {
          console.error('[identity] reset failed', e)
        }
      }
    }
  }
}

export const identity = createIdentityHub()
