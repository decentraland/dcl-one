// A Web Lock this page holds until the returned release, or until the page goes (the browser lets a closed or
// crashed page's locks go): the shell's way to make one tab the only one running something, across the tabs of one
// browser. Used by the in-world Shop (one per purchase operation) and Donate (one per account's pending tip).

/**
 * Takes `name` only if free, null when another page holds it; with `wait`, queued until that page lets go. Null
 * too without Web Locks: then nothing proves this page is the only one.
 */
export function webLock(locks: LockManager | undefined, name: string, wait = false): Promise<(() => void) | null> {
  return new Promise((resolve) => {
    if (!locks) return resolve(null)
    locks
      .request(name, wait ? {} : { ifAvailable: true }, (lock) => {
        if (!lock) {
          resolve(null)
          return undefined
        }
        return new Promise<void>((release) => resolve(() => release()))
      })
      .catch(() => resolve(null))
  })
}

/** This page's Web Locks, or undefined where the browser has none. */
export function pageLocks(): LockManager | undefined {
  return typeof navigator !== 'undefined' ? navigator.locks : undefined
}
