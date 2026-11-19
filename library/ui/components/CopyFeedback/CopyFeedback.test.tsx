import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { CopyFeedback } from './CopyFeedback'

function renderFeedback() {
  render(
    <CopyFeedback value="address" successAnnouncement="User ID copied" errorAnnouncement="Copy failed">
      {(copy) => <button onClick={copy}>Copy ID</button>}
    </CopyFeedback>
  )
}

describe('when copying with feedback', () => {
  let write: ReturnType<typeof vi.fn<(value: string) => Promise<void>>>
  let copy: () => Promise<void>

  beforeEach(() => {
    vi.useFakeTimers()
    write = vi.fn<(value: string) => Promise<void>>().mockResolvedValue(undefined)
    vi.stubGlobal('navigator', { clipboard: { writeText: write } })
    renderFeedback()
    copy = () => act(async () => void fireEvent.click(screen.getByRole('button', { name: 'Copy ID' })))
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.useRealTimers()
  })

  it('should show confirmation only after writing the value', async () => {
    expect(screen.queryByText('Copied')).toBeNull()
    await copy()
    expect(write).toHaveBeenCalledWith('address')
    expect(screen.getByText('Copied')).toHaveAttribute('aria-hidden', 'true')
    expect(screen.getByRole('status')).toHaveTextContent('User ID copied')
  })

  it('should restart the animation and dismiss after the last copied value', async () => {
    await copy()
    const first = screen.getByText('Copied')
    act(() => vi.advanceTimersByTime(1200))
    await copy()
    expect(screen.getByText('Copied')).not.toBe(first)
    act(() => vi.advanceTimersByTime(400))
    expect(screen.getByText('Copied')).toBeVisible()
    act(() => vi.advanceTimersByTime(2000))
    expect(screen.queryByText('Copied')).toBeNull()
  })

  it('should announce a rejected write without showing success', async () => {
    await copy()
    write.mockRejectedValueOnce(new Error('Denied'))
    await copy()
    expect(screen.queryByText('Copied')).toBeNull()
    expect(screen.getByRole('status')).toHaveTextContent('Copy failed')
  })

  it('should announce failure without a clipboard', async () => {
    vi.stubGlobal('navigator', {})
    await copy()
    expect(screen.getByRole('status')).toHaveTextContent('Copy failed')
  })
})
