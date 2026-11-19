// ArmedButton (design/ArmedButton): a Button that answers only ARM_MS after it appears, and takes every prop of
// Button's <button> form, radius included.
import { afterEach, describe, it, expect, vi } from 'vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import { ArmedButton, ARM_MS } from '../design'

afterEach(() => {
  vi.useRealTimers()
})

describe('ArmedButton', () => {
  it('is a plain button, announced as disabled until it arms, that answers only once armed', () => {
    vi.useFakeTimers()
    const onClick = vi.fn()
    render(
      <ArmedButton size="modal" radius="pill" variant="secondary" onClick={onClick}>
        SEND
      </ArmedButton>
    )
    const button = screen.getByRole('button', { name: 'SEND' }) as HTMLButtonElement
    expect(button.type).toBe('button')
    expect(button.getAttribute('aria-disabled')).toBe('true')
    fireEvent.click(button)
    expect(onClick).not.toHaveBeenCalled()
    act(() => vi.advanceTimersByTime(ARM_MS))
    expect(button.getAttribute('aria-disabled')).toBeNull()
    fireEvent.click(button)
    expect(onClick).toHaveBeenCalledTimes(1)
  })
})
