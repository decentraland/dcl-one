import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { CopyFeedback } from './CopyFeedback'

function renderFeedback() {
  render(
    <CopyFeedback
      value="address"
      successAnnouncement="User ID copied"
      errorAnnouncement="Copy failed"
    >
      {(copy) => <button onClick={copy}>Copy ID</button>}
    </CopyFeedback>
  )
}

describe('CopyFeedback', () => {
  let write: Mock<(value: string) => Promise<void>>
  let copy: () => Promise<void>

  beforeEach(() => {
    vi.useFakeTimers()
    write = vi.fn<(value: string) => Promise<void>>().mockResolvedValue(undefined)
    vi.stubGlobal('navigator', { clipboard: { writeText: write } })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.useRealTimers()
    vi.resetAllMocks()
  })

  describe('when copying with feedback', () => {
    beforeEach(() => {
      renderFeedback()
      copy = () =>
        act(async () => void fireEvent.click(screen.getByRole('button', { name: 'Copy ID' })))
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

    describe('and a later write is rejected', () => {
      beforeEach(async () => {
        await copy()
        write.mockRejectedValueOnce(new Error('Denied'))
        await copy()
      })

      it('should not show success', () => {
        expect(screen.queryByText('Copied')).toBeNull()
      })

      it('should announce the failure', () => {
        expect(screen.getByRole('status')).toHaveTextContent('Copy failed')
      })
    })

    describe('and there is no clipboard', () => {
      beforeEach(async () => {
        vi.stubGlobal('navigator', {})
        await copy()
      })

      it('should announce the failure', () => {
        expect(screen.getByRole('status')).toHaveTextContent('Copy failed')
      })
    })
  })

  describe('when copy feedback replaces an inline value', () => {
    beforeEach(() => {
      render(
        <CopyFeedback
          value="full address"
          inlineValue="0x12…34"
          copiedLabel="Address copied!"
          successAnnouncement="User ID copied"
          errorAnnouncement="Copy failed"
        >
          {(activate) => <button onClick={activate}>Copy ID</button>}
        </CopyFeedback>
      )
      copy = () =>
        act(async () => void fireEvent.click(screen.getByRole('button', { name: 'Copy ID' })))
    })

    it('should restore the short address two seconds after the last successful copy', async () => {
      expect(screen.getByText('0x12…34')).toBeVisible()
      await copy()
      expect(screen.queryByText('0x12…34')).toBeNull()
      expect(screen.getByText('Address copied!')).toBeVisible()
      act(() => vi.advanceTimersByTime(1600))
      await copy()
      act(() => vi.advanceTimersByTime(1999))
      expect(screen.getByText('Address copied!')).toBeVisible()
      act(() => vi.advanceTimersByTime(1))
      expect(screen.getByText('0x12…34')).toBeVisible()
    })

    describe('and copying fails', () => {
      beforeEach(async () => {
        write.mockRejectedValueOnce(new Error('Denied'))
        await copy()
      })

      it('should retain the short address', () => {
        expect(screen.getByText('0x12…34')).toBeVisible()
      })

      it('should not show the copied label', () => {
        expect(screen.queryByText('Address copied!')).toBeNull()
      })

      it('should announce the failure', () => {
        expect(screen.getByRole('status')).toHaveTextContent('Copy failed')
      })
    })
  })

  describe('when the host sets a custom feedback duration', () => {
    beforeEach(() => {
      render(
        <CopyFeedback value="full address" durationMs={2000} showTooltip={false}>
          {(activate) => <button onClick={activate}>Copy address</button>}
        </CopyFeedback>
      )
    })

    it('should clear feedback after the configured two seconds', async () => {
      await act(async () => fireEvent.click(screen.getByRole('button', { name: 'Copy address' })))
      act(() => vi.advanceTimersByTime(1999))
      expect(screen.getByRole('status')).toHaveTextContent('Copied')
      act(() => vi.advanceTimersByTime(1))
      expect(screen.getByRole('status')).toBeEmptyDOMElement()
    })
  })
})
