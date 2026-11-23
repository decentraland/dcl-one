// Intercepts the browser back button while a flow with unsaved work is open: a sentinel history
// entry absorbs the pop, the sentinel is re-pushed, and the caller gets to show its own
// confirmation instead of losing the work to a silent SPA navigation.

const SENTINEL_KEY = 'backGuard'

/** The slice of the History API the guard needs; injectable so tests can fake traversal. */
export type BackGuardHistory = {
  getState(): unknown
  pushState(state: unknown): void
  back(): void
  /** Subscribes to popstate; returns the unsubscribe. */
  subscribe(handler: () => void): () => void
}

export function createBackGuard(history: BackGuardHistory) {
  // Programmatic history.back() calls (consuming the sentinel on uninstall) fire popstate too.
  // The counter tells those apart from real user backs, and the shared listener keeps draining
  // it even when no guard is installed — this is what makes the guard safe under React
  // StrictMode's mount → unmount → remount effect replay.
  let pendingProgrammaticBacks = 0
  let activeOnBack: (() => void) | null = null
  let subscribed = false

  const isSentinelActive = () => {
    const state = history.getState() as Record<string, unknown> | null
    return !!state && state[SENTINEL_KEY] === true
  }

  const pushSentinel = () => history.pushState({ [SENTINEL_KEY]: true })

  const onPopState = () => {
    if (pendingProgrammaticBacks > 0) {
      pendingProgrammaticBacks--
      // A StrictMode replay uninstalls and immediately reinstalls: the reinstall may land before
      // this programmatic pop consumes the sentinel, so re-arm it for the still-active guard.
      if (activeOnBack && !isSentinelActive()) {
        pushSentinel()
      }
      return
    }
    if (!activeOnBack) return
    pushSentinel()
    activeOnBack()
  }

  /**
   * Pushes the sentinel entry and calls `onBack` whenever the user presses back while installed
   * (the history position is preserved). Returns the uninstall function, which consumes the
   * sentinel again. Only one guard can be active at a time — installing replaces the callback.
   */
  return function installBackGuard(onBack: () => void): () => void {
    if (!subscribed) {
      subscribed = true
      history.subscribe(onPopState)
    }

    if (!isSentinelActive()) {
      pushSentinel()
    }
    activeOnBack = onBack

    return () => {
      if (activeOnBack === onBack) {
        activeOnBack = null
      }
      if (isSentinelActive()) {
        pendingProgrammaticBacks++
        history.back()
      }
    }
  }
}

const windowHistory: BackGuardHistory = {
  getState: () => window.history.state as unknown,
  pushState: state => window.history.pushState(state, ''),
  back: () => window.history.back(),
  subscribe: handler => {
    window.addEventListener('popstate', handler)
    return () => window.removeEventListener('popstate', handler)
  }
}

export const installBackGuard = createBackGuard(windowHistory)
