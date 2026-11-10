import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PopupHost, closeTopPopup, resetPopups } from '../design'
import { ConnectivityFeedback } from '../features/error/ConnectivityFeedback'
import { useConnectivity } from '../features/session/useConnectivity'
import { renderHook } from '@testing-library/react'
import { FakeDriver } from './harness'

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
    // A fast failed probe can complete between two HUD polls.
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

  it('reads engine state, retries through the shared command bridge, and clears on session exit', async () => {
    const driver = new FakeDriver()
    driver.commandReply = () => JSON.stringify({ connectionLost: true, probing: false, rooms: [] })
    const { result, rerender } = renderHook(({ active }) => useConnectivity(driver, active), { initialProps: { active: true } })
    await waitFor(() => expect(result.current.state?.connectionLost).toBe(true))
    expect(driver.commands).toContain('/connectivity_state')
    await act(() => result.current.retry())
    await waitFor(() => expect(driver.commands).toContain('/connectivity_retry'))
    rerender({ active: false })
    expect(result.current.state).toBeNull()
  })

  it('never overlaps a pending status read and ignores its result after leaving the session', async () => {
    vi.useFakeTimers()
    let finish: (reply: string) => void = () => {}
    class PendingDriver extends FakeDriver {
      async command(line: string): Promise<string> {
        this.commands.push(line)
        return new Promise(resolve => { finish = resolve })
      }
    }
    const driver = new PendingDriver()
    const { result, rerender } = renderHook(({ active }) => useConnectivity(driver, active), { initialProps: { active: true } })
    await act(async () => vi.advanceTimersByTimeAsync(4000))
    expect(driver.commands).toEqual(['/connectivity_state'])
    rerender({ active: false })
    await act(async () => finish(JSON.stringify({ connectionLost: true, probing: false, rooms: [] })))
    expect(result.current.state).toBeNull()
  })

  it('retains the snapshot identity while polling unchanged engine state', async () => {
    vi.useFakeTimers()
    const driver = new FakeDriver()
    let snapshot = { connectionLost: false, probing: false, rooms: [] as Array<{scope: string; state: string}> }
    driver.commandReply = () => JSON.stringify(snapshot)
    const { result } = renderHook(() => useConnectivity(driver, true))
    await act(async () => {})
    const initial = result.current.state
    expect(initial).not.toBeNull()
    await act(async () => vi.advanceTimersByTimeAsync(3000))
    expect(result.current.state).toBe(initial)
    snapshot = { ...snapshot, rooms: [{ scope: 'voice', state: 'denied' }] }
    await act(async () => vi.advanceTimersByTimeAsync(1000))
    expect(result.current.state?.rooms).toEqual([{ scope: 'voice', state: 'denied' }])
  })
})
