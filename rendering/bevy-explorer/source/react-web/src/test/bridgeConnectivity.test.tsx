import { act, fireEvent, render, renderHook, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import type { ConnectivitySnapshot } from '@explorer-core/engine/generated'
import type { PageToScene, SceneToPage } from '@explorer-core/engine/protocol'
import { useConnectivity } from '@hud/features/session/useConnectivity'
import { FakeDriver } from '@hud/test/harness'
import { PopupHost, resetPopups } from '@hud/design'
import { ConnectivityFeedback } from '@hud/features/error/ConnectivityFeedback'

const { open, retry } = vi.hoisted(() => ({ open: vi.fn(), retry: vi.fn() }))
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: { getConnectivityStream: open, retryConnectivity: retry } }))
vi.mock('../../bridge-scene/src/system-helpers', async () => {
  const { relayStream } = await import('../../bridge-scene/src/relay')
  return { relay: (name: string, source: () => Promise<AsyncIterable<unknown>>, onItem: (item: unknown) => void) => {
    relayStream(name, source, onItem, () => new Promise(() => {}))
  } }
})

afterEach(() => { act(resetPopups); vi.resetAllMocks(); vi.useRealTimers() })

async function bridge() {
  const { registerConnectivity } = await import('../../bridge-scene/src/domains/connectivity')
  const handlers = new Map<string, (message: PageToScene) => void | Promise<void>>()
  class SceneDriver extends FakeDriver {
    send(message: PageToScene) {
      super.send(message)
      void handlers.get(message.kind)?.(message)
    }
  }
  const driver = new SceneDriver()
  open.mockReturnValue(new Promise(() => {}))
  registerConnectivity({
    send: message => { queueMicrotask(() => driver.emit(message)) },
    on: (kind, handler) => { handlers.set(kind, handler as (message: PageToScene) => void | Promise<void>) },
    push: () => { throw new Error('connectivity must consume its stream without a frame poll') }
  })
  return driver
}

function Feedback({ driver, active = true }: { driver: FakeDriver; active?: boolean }) {
  const { retry } = useConnectivity(driver, active)
  return <><PopupHost /><ConnectivityFeedback state={active ? { connectionLost: true, probing: false, rooms: [] } : null} retry={retry} exit={() => {}} /></>
}

describe('connectivity stream bridge', () => {
  it('replays the latest snapshot to a late HUD, delivers changes, and routes Retry to the engine', async () => {
    const { registerConnectivity } = await import('../../bridge-scene/src/domains/connectivity')
    let next!: (item: IteratorResult<ConnectivitySnapshot>) => void
    const stream: AsyncIterableIterator<ConnectivitySnapshot> = {
      [Symbol.asyncIterator]() { return this },
      next: () => new Promise(resolve => { next = resolve })
    }
    open.mockResolvedValue(stream)
    const handlers = new Map<string, (message: PageToScene) => void>()
    class SceneDriver extends FakeDriver {
      send(message: PageToScene) {
        super.send(message)
        handlers.get(message.kind)?.(message)
      }
    }
    const driver = new SceneDriver()
    const incoming: SceneToPage[] = []
    const ctx: Ctx = {
      send: message => { incoming.push(message); queueMicrotask(() => driver.emit(message)) },
      on: (kind, handler) => { handlers.set(kind, handler as (message: PageToScene) => void) },
      push: () => { throw new Error('connectivity must consume its stream without a frame poll') }
    }
    registerConnectivity(ctx)
    await waitFor(() => expect(next).toBeDefined())
    const initial: ConnectivitySnapshot = { connectionLost: true, probing: false, everConnected: true, suppressed: false, rooms: [] }
    await act(async () => next({ value: initial, done: false }))
    expect(incoming).toEqual([{ kind: 'connectivity', state: initial }])
    const { result, unmount } = renderHook(() => useConnectivity(driver, true))
    await waitFor(() => expect(result.current.state).toEqual(initial))

    const recovered: ConnectivitySnapshot = { ...initial, connectionLost: false, rooms: [{ scope: 'realm', state: 'connected', retryAt: null }] }
    await act(async () => next({ value: recovered, done: false }))
    expect(result.current.state).toEqual(recovered)
    await act(async () => { await result.current.retry() })
    expect(retry).toHaveBeenCalledOnce()
    expect(driver.commands).toEqual([])
    expect(open).toHaveBeenCalledOnce()

    incoming.length = 0
    await act(async () => { handlers.get('hello')?.({ kind: 'hello' }) })
    expect(incoming).toEqual([{ kind: 'connectivity', state: recovered }])
    unmount()
  })

  it('keeps Retry pending until the real bridge accepts it, and permits Retry after rejection', async () => {
    const driver = await bridge()
    let accept!: () => void
    retry.mockImplementationOnce(() => new Promise<void>(resolve => { accept = resolve }))
    render(<Feedback driver={driver} />)
    const button = screen.getByRole('button', { name: 'Retry' })
    await userEvent.click(button)
    expect(button).toBeDisabled()
    expect(button).toHaveAttribute('aria-busy', 'true')
    expect(screen.getByRole('button', { name: 'Exit Application' })).toBeEnabled()
    await userEvent.click(button)
    expect(retry).toHaveBeenCalledOnce()
    await act(async () => accept())
    expect(button).toBeEnabled()
    expect(screen.getByRole('alertdialog', { name: 'Connection Lost' })).toBeVisible()

    retry.mockRejectedValueOnce(new Error('engine unavailable'))
    await userEvent.click(button)
    await waitFor(() => expect(button).toBeEnabled())
    expect(retry).toHaveBeenCalledTimes(2)
    expect(screen.getByRole('button', { name: 'Exit Application' })).toBeEnabled()
  })

  it('releases a lost Retry reply on its deadline and clears pending work on session exit', async () => {
    const driver = await bridge()
    retry.mockReturnValue(new Promise(() => {}))
    vi.useFakeTimers()
    const { rerender, unmount } = render(<Feedback driver={driver} />)
    const button = screen.getByRole('button', { name: 'Retry' })
    fireEvent.click(button)
    expect(button).toBeDisabled()
    await act(async () => vi.advanceTimersByTimeAsync(10_000))
    expect(button).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Exit Application' })).toBeEnabled()
    fireEvent.click(button)
    expect(button).toBeDisabled()
    await act(async () => rerender(<Feedback driver={driver} active={false} />))
    expect(vi.getTimerCount()).toBe(0)
    unmount()
  })
})
