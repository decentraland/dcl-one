// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { createStore } from './store'

type Counter = { count: number; label: string; increment: () => void }

const createCounter = () =>
  createStore<Counter>((set, get) => ({
    count: 0,
    label: 'a',
    increment: () => set({ count: get().count + 1 })
  }))

describe('createStore', () => {
  it('re-renders a component when the value it selects changes', () => {
    const useCounter = createCounter()
    const { result } = renderHook(() => useCounter(state => state.count))

    act(() => useCounter.getState().increment())

    expect(result.current).toBe(1)
  })

  it('accepts an updater that reads the current state', () => {
    const useCounter = createCounter()

    useCounter.setState(state => ({ count: state.count + 5 }))

    expect(useCounter.getState().count).toBe(5)
  })

  it('does not notify when nothing changed', () => {
    const useCounter = createCounter()
    let calls = 0
    useCounter.subscribe(() => calls++)

    useCounter.setState({ label: 'a' })

    expect(calls).toBe(0)
  })

  it('stops notifying a listener that unsubscribed', () => {
    const useCounter = createCounter()
    let calls = 0
    const unsubscribe = useCounter.subscribe(() => calls++)
    unsubscribe()

    useCounter.getState().increment()

    expect(calls).toBe(0)
  })
})
