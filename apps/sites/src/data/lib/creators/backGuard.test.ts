import { describe, expect, it, vi } from 'vitest'
import { createBackGuard, type BackGuardHistory } from './backGuard'

/**
 * Deterministic history double: a real entry stack whose back() delivers popstate
 * asynchronously, like browsers do (jsdom does not implement traversal reliably).
 */
function createFakeHistory() {
  const entries: unknown[] = [null]
  let index = 0
  let handler: (() => void) | null = null
  const pending: Array<() => void> = []

  const history: BackGuardHistory = {
    getState: () => entries[index],
    pushState: state => {
      entries.splice(index + 1)
      entries.push(state)
      index++
    },
    back: () => {
      pending.push(() => {
        if (index > 0) index--
        handler?.()
      })
    },
    subscribe: popStateHandler => {
      handler = popStateHandler
      return () => {
        handler = null
      }
    }
  }

  return {
    history,
    /** Delivers queued popstates from programmatic back() calls. */
    flush: () => {
      while (pending.length > 0) pending.shift()!()
    },
    /** A user pressing back: traverses immediately and fires popstate. */
    userBack: () => {
      if (index > 0) index--
      handler?.()
    },
    getState: () => entries[index] as Record<string, unknown> | null,
    depth: () => index
  }
}

describe('createBackGuard', () => {
  it('intercepts a user back, keeps the guard armed and preserves the position', () => {
    const fake = createFakeHistory()
    const install = createBackGuard(fake.history)
    const onBack = vi.fn()

    const uninstall = install(onBack)
    expect(fake.getState()?.backGuard).toBe(true)

    fake.userBack()
    expect(onBack).toHaveBeenCalledTimes(1)
    // The sentinel was re-pushed, so a second back is intercepted too.
    fake.userBack()
    expect(onBack).toHaveBeenCalledTimes(2)

    uninstall()
    fake.flush()
    expect(fake.depth()).toBe(0)
  })

  it('consumes its sentinel on uninstall without firing the callback', () => {
    const fake = createFakeHistory()
    const install = createBackGuard(fake.history)
    const onBack = vi.fn()

    const uninstall = install(onBack)
    uninstall()
    fake.flush()

    expect(onBack).not.toHaveBeenCalled()
    expect(fake.getState()?.backGuard).toBeUndefined()
    expect(fake.depth()).toBe(0)
  })

  it('survives a StrictMode-style install → uninstall → reinstall replay', () => {
    const fake = createFakeHistory()
    const install = createBackGuard(fake.history)
    const onBack = vi.fn()

    // Effect replay: cleanup runs, then the effect re-runs before the programmatic pop lands.
    const uninstallFirst = install(onBack)
    uninstallFirst()
    const uninstallSecond = install(onBack)
    fake.flush()

    // The replayed uninstall must not surface as a user back.
    expect(onBack).not.toHaveBeenCalled()
    // The guard is still armed for a real user back.
    expect(fake.getState()?.backGuard).toBe(true)
    fake.userBack()
    expect(onBack).toHaveBeenCalledTimes(1)

    uninstallSecond()
    fake.flush()
    expect(fake.depth()).toBe(0)
  })
})
