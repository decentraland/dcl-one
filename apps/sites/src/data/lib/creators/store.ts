import { useSyncExternalStore } from 'react'

type SetState<T> = (partial: Partial<T> | ((state: T) => Partial<T>)) => void

type Store<T> = {
  (): T
  <U>(selector: (state: T) => U): U
  getState: () => T
  setState: SetState<T>
  subscribe: (listener: () => void) => () => void
}

/** A store shared across the tree and readable outside React: `useStore(selector)` in components, `getState`/`setState` elsewhere. */
export function createStore<T extends object>(init: (set: SetState<T>, get: () => T) => T): Store<T> {
  const listeners = new Set<() => void>()
  let state: T
  const getState = () => state
  const setState: SetState<T> = partial => {
    const patch = typeof partial === 'function' ? partial(state) : partial
    if (Object.entries(patch).every(([key, value]) => Object.is(state[key as keyof T], value))) return
    state = { ...state, ...patch }
    for (const listener of listeners) listener()
  }
  const subscribe = (listener: () => void) => {
    listeners.add(listener)
    return () => listeners.delete(listener)
  }
  state = init(setState, getState)
  const initial = state
  function useStore(): T
  function useStore<U>(selector: (state: T) => U): U
  function useStore<U>(selector?: (state: T) => U): T | U {
    return useSyncExternalStore(
      subscribe,
      () => (selector ? selector(state) : state),
      () => (selector ? selector(initial) : initial)
    )
  }
  return Object.assign(useStore, { getState, setState, subscribe })
}
