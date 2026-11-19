// The Donate panel's SEND TIP (features/donate/DonatePanel), as the /play shell draws it: the form opens with the
// first suggestion picked, so it is ready to send at once, and only a press after it has been up a moment sends.
import { afterEach, describe, it, expect, vi } from 'vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import { ARM_MS } from '../../design'
import { DonatePanel } from '../../features/donate/DonatePanel'
import type { DonateView } from '../../features/donate/types'

const FORM: DonateView = {
  step: 'form',
  scene: 'Genesis Plaza',
  creator: { address: '0x' + 'c'.repeat(40) },
  balance: 100,
  presets: [10, 25, 50],
  usdRate: null
}

afterEach(() => {
  vi.useRealTimers()
})

describe('the Donate panel’s SEND TIP', () => {
  it('sends nothing on a click that lands before it has been up a moment, as the second click of the press that opened it', () => {
    vi.useFakeTimers()
    const onSend = vi.fn()
    const handlers = { onClose: vi.fn(), onSend }
    const { rerender } = render(<DonatePanel view={{ step: 'loading' }} handlers={handlers} />)
    rerender(<DonatePanel view={FORM} handlers={handlers} />)
    const send = screen.getByRole('button', { name: 'SEND TIP' }) as HTMLButtonElement
    expect(send.disabled).toBe(false)
    expect(send.getAttribute('aria-disabled')).toBe('true')
    fireEvent.click(send)
    act(() => vi.advanceTimersByTime(ARM_MS - 1))
    fireEvent.click(send)
    expect(onSend).not.toHaveBeenCalled()
    act(() => vi.advanceTimersByTime(1))
    expect(send.getAttribute('aria-disabled')).toBeNull()
    fireEvent.click(send)
    expect(onSend).toHaveBeenCalledTimes(1)
    expect(onSend).toHaveBeenCalledWith('10')
  })

  it('stays disabled for an amount it would refuse, armed or not', () => {
    vi.useFakeTimers()
    const onSend = vi.fn()
    render(<DonatePanel view={{ ...FORM, balance: 5 }} handlers={{ onClose: vi.fn(), onSend }} />)
    act(() => vi.advanceTimersByTime(ARM_MS))
    const send = screen.getByRole('button', { name: 'SEND TIP' }) as HTMLButtonElement
    expect(send.disabled).toBe(true)
    fireEvent.click(send)
    expect(onSend).not.toHaveBeenCalled()
  })
})
