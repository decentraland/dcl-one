import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PopupHost, closeTopPopup, resetPopups } from '../../design'
import { ConnectivityFeedback } from '../../features/error/ConnectivityFeedback'
import { useConnectivity } from '../../features/session/useConnectivity'
import { renderHook } from '@testing-library/react'
import { FakeDriver } from '../harness'

afterEach(() => { act(resetPopups); vi.useRealTimers() })

describe('connectivity feedback', () => {
  it('keeps one dialog and follows the engine through Retry and another failed probe', async () => {
    const retry = vi.fn()
    const exit = vi.fn()
    const lost = { connectionLost: true, probing: false, rooms: [] }
    const { rerender } = render(<><PopupHost /><ConnectivityFeedback state={lost} retry={retry} exit={exit} /></>)
    expect(screen.getAllByRole('alertdialog', { name: 'Connection Lost' })).toHaveLength(1)
    expect(screen.getByText('You appear to be offline. Please check your internet connection and retry.')).toBeVisible()
    expect(screen.getAllByRole('button').map(button => button.textContent)).toEqual(['Exit Application', 'Retry'])
    act(closeTopPopup)
    expect(screen.getByRole('alertdialog')).toBeVisible()
    rerender(<><PopupHost /><ConnectivityFeedback state={{ ...lost, probing: true }} retry={retry} exit={exit} /></>)
    expect(screen.getAllByRole('alertdialog')).toHaveLength(1)
    await userEvent.click(screen.getByRole('button', { name: 'Retry' }))
    expect(retry).toHaveBeenCalledTimes(1)
    expect(screen.getByRole('alertdialog')).toBeVisible()
    rerender(<><PopupHost /><ConnectivityFeedback state={{ ...lost, connectionLost: false }} retry={retry} exit={exit} /></>)
    expect(screen.queryByRole('alertdialog')).toBeNull()
    rerender(<><PopupHost /><ConnectivityFeedback state={lost} retry={retry} exit={exit} /></>)
    expect(screen.getByRole('alertdialog')).toBeVisible()
    rerender(<><PopupHost /><ConnectivityFeedback state={{ ...lost, connectionLost: false }} retry={retry} exit={exit} /></>)
    expect(screen.queryByRole('alertdialog')).toBeNull()
    expect(exit).not.toHaveBeenCalled()
  })

  it('keeps recovery controls when Retry is accepted but the next snapshot is still offline', async () => {
    const lost = { connectionLost: true, probing: false, rooms: [] }
    const { rerender } = render(<><PopupHost /><ConnectivityFeedback state={lost} retry={async () => {}} exit={vi.fn()} /></>)
    await userEvent.click(screen.getByRole('button', { name: 'Retry' }))
    // Accepting Retry does not mean the engine has recovered.
    rerender(<><PopupHost /><ConnectivityFeedback state={{ ...lost }} retry={async () => {}} exit={vi.fn()} /></>)
    expect(screen.getByRole('alertdialog', { name: 'Connection Lost' })).toBeVisible()
    expect(screen.getByRole('button', { name: 'Retry' })).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Exit Application' })).toBeEnabled()
  })

  it('does not submit Retry twice while the engine command is pending', async () => {
    let finish!: () => void
    const retry = vi.fn(() => new Promise<void>(resolve => { finish = resolve }))
    render(<><PopupHost /><ConnectivityFeedback state={{ connectionLost: true, probing: false, rooms: [] }} retry={retry} exit={vi.fn()} /></>)
    const button = screen.getByRole('button', { name: 'Retry' })
    await userEvent.click(button)
    expect(button).toBeDisabled()
    await userEvent.click(button)
    expect(retry).toHaveBeenCalledTimes(1)
    await act(async () => finish())
    expect(button).toBeEnabled()
  })

  it('exits through the session action and shows scoped denial separately', async () => {
    const exit = vi.fn()
    const { unmount } = render(<><PopupHost /><ConnectivityFeedback state={{ connectionLost: true, probing: false, rooms: [] }} retry={vi.fn()} exit={exit} /></>)
    await userEvent.click(screen.getByRole('button', { name: 'Exit Application' }))
    expect(exit).toHaveBeenCalledTimes(1)
    unmount()
    render(<><PopupHost /><ConnectivityFeedback state={{ connectionLost: false, probing: false, rooms: [{ scope: 'scene', state: 'denied' }] }} retry={vi.fn()} exit={exit} /></>)
    expect(screen.getByRole('status')).toHaveTextContent('Scene room: access denied')
    expect(screen.queryByRole('alertdialog')).toBeNull()
  })

  it('keeps the recovery controls available when the engine rejects Retry', async () => {
    render(<><PopupHost /><ConnectivityFeedback state={{ connectionLost: true, probing: false, rooms: [] }}
      retry={() => Promise.reject(new Error('engine unavailable'))} exit={vi.fn()} /></>)
    await userEvent.click(screen.getByRole('button', { name: 'Retry' }))
    expect(screen.getByRole('alertdialog', { name: 'Connection Lost' })).toBeVisible()
    expect(screen.getByRole('button', { name: 'Exit Application' })).toBeEnabled()
  })

  it.each([
    ['failed', 'connection failed'],
    ['disconnected', 'disconnected'],
  ])('shows a %s room while the network is reachable', (state, label) => {
    render(<ConnectivityFeedback state={{ connectionLost: false, probing: false, rooms: [{ scope: 'scene', state }] }} retry={vi.fn()} exit={vi.fn()} />)
    expect(screen.getByRole('status', { name: 'Multiplayer status' })).toHaveTextContent(`Scene room: ${label}`)
    expect(screen.queryByRole('alertdialog')).toBeNull()
  })

  it.each([
    ['private_chat', 'Private chat'],
    ['voice', 'Voice room'],
  ])('keeps %s access decisions and pauses scoped', (scope, label) => {
    const { rerender } = render(<ConnectivityFeedback state={null} retry={vi.fn()} exit={vi.fn()} />)
    for (const [state, reason] of [
      ['denied', 'access denied'],
      ['identity_conflict', 'this identity is connected elsewhere'],
      ['invalid', 'connection settings are invalid'],
      ['credentials_required', 'new credentials are required'],
      ['paused', 'paused'],
    ]) {
      rerender(<ConnectivityFeedback state={{ connectionLost: false, probing: false, rooms: [{ scope, state }] }} retry={vi.fn()} exit={vi.fn()} />)
      expect(screen.getByRole('status', { name: 'Multiplayer status' })).toHaveTextContent(`${label}: ${reason}`)
      expect(screen.queryByRole('alertdialog')).toBeNull()
    }
  })

  it.each(['scene', 'private_chat', 'voice'])('keeps the initial %s connection quiet', scope => {
    render(<ConnectivityFeedback state={{ connectionLost: false, probing: false, rooms: [{ scope, state: 'connecting' }] }} retry={vi.fn()} exit={vi.fn()} />)
    expect(screen.queryByRole('status')).toBeNull()
    expect(screen.queryByRole('alertdialog')).toBeNull()
  })

  it('subscribes to engine state, sends Retry through the scene, and clears on session exit', async () => {
    const driver = new FakeDriver()
    const { result, rerender } = renderHook(({ active }) => useConnectivity(driver, active), { initialProps: { active: true } })
    driver.emit({ kind: 'connectivity', state: { connectionLost: true, probing: false, everConnected: true, suppressed: false, rooms: [] } })
    expect(result.current.state?.connectionLost).toBe(true)
    let pending: void | Promise<void>
    act(() => { pending = result.current.retry() })
    const request = driver.sent.find(message => message.kind === 'retryConnectivity')!
    expect(request.requestId).toEqual(expect.any(String))
    driver.emit({ kind: 'connectivityRetry', requestId: request.requestId })
    await pending!
    expect(driver.sent).toEqual([{ kind: 'getConnectivity' }, request])
    expect(driver.commands).toEqual([])
    rerender({ active: false })
    expect(result.current.state).toBeNull()
  })

  it('unsubscribes on session exit and ignores events and Retry until re-entry', () => {
    const driver = new FakeDriver()
    const unsubscribe = vi.fn()
    const originalOn = driver.on.bind(driver)
    vi.spyOn(driver, 'on').mockImplementation(listener => {
      const off = originalOn(listener)
      return () => { unsubscribe(); off() }
    })
    const { result, rerender, unmount } = renderHook(({ active }) => useConnectivity(driver, active), { initialProps: { active: true } })
    const state = { connectionLost: true, probing: false, everConnected: true, suppressed: false, rooms: [] }
    driver.emit({ kind: 'connectivity', state })
    rerender({ active: false })
    driver.emit({ kind: 'connectivity', state })
    act(() => result.current.retry())
    expect(result.current.state).toBeNull()
    expect(driver.sent).toEqual([{ kind: 'getConnectivity' }])
    rerender({ active: true })
    expect(result.current.state).toBeNull()
    driver.emit({ kind: 'connectivity', state })
    expect(result.current.state).toEqual(state)
    expect(unsubscribe).toHaveBeenCalledTimes(1)
    unmount()
    expect(unsubscribe).toHaveBeenCalledTimes(2)
    driver.emit({ kind: 'connectivity', state: { ...state, connectionLost: false } })
    expect(result.current.state).toEqual(state)
  })

  it.each(['private_chat', 'voice'])('retains the snapshot identity on repeated events and accepts changed %s rooms', scope => {
    const driver = new FakeDriver()
    const snapshot = { connectionLost: false, probing: false, everConnected: true, suppressed: false, rooms: [] }
    const { result } = renderHook(() => useConnectivity(driver, true))
    driver.emit({ kind: 'connectivity', state: snapshot })
    const initial = result.current.state
    expect(initial).not.toBeNull()
    driver.emit({ kind: 'connectivity', state: { ...snapshot, rooms: [] } })
    expect(result.current.state).toBe(initial)
    driver.emit({ kind: 'connectivity', state: { ...snapshot, rooms: [{ scope, state: 'denied', retryAt: null }] } })
    expect(result.current.state?.rooms).toEqual([{ scope, state: 'denied', retryAt: null }])
    expect(driver.commands).toEqual([])
  })

  it('rejects a pending Retry on exit and ignores its late acknowledgement after re-entry', async () => {
    const driver = new FakeDriver()
    const { result, rerender } = renderHook(({ active }) => useConnectivity(driver, active), { initialProps: { active: true } })
    const first = result.current.retry()!
    const rejected = expect(first).rejects.toThrow('session ended')
    const firstRequest = driver.sent.find(message => message.kind === 'retryConnectivity')!
    rerender({ active: false })
    await rejected
    rerender({ active: true })
    const second = result.current.retry()!
    expect(result.current.retry()).toBe(second)
    const requests = driver.sent.filter(message => message.kind === 'retryConnectivity')
    expect(requests).toHaveLength(2)
    const settled = vi.fn()
    void second.then(settled)
    driver.emit({ kind: 'connectivityRetry', requestId: firstRequest.requestId })
    await Promise.resolve()
    expect(settled).not.toHaveBeenCalled()
    driver.emit({ kind: 'connectivityRetry', requestId: requests[1].requestId })
    await second
    expect(settled).toHaveBeenCalledOnce()
  })
})
