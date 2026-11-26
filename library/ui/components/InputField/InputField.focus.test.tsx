import { createRef, type RefObject } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, it, expect, vi, type Mock } from 'vitest'
import { InputExplorerNames } from '../InputExplorerNames/InputExplorerNames'

afterEach(() => {
  vi.resetAllMocks()
})

describe('when an input owns native focus and exposes its ref', () => {
  describe('and it has a stable callback ref', () => {
    let ref: Mock<(node: HTMLInputElement | null) => void>

    beforeEach(() => {
      ref = vi.fn((node: HTMLInputElement | null) => node?.focus())
    })

    it('should not detach the ref or move focus on an internal reveal update', async () => {
      const { rerender, unmount } = render(
        <InputExplorerNames label="Password" type="password" allowPasswordReveal ref={ref} />
      )
      const input = screen.getByLabelText('Password')
      expect(ref).toHaveBeenCalledTimes(1)
      await userEvent.setup().click(screen.getByRole('button', { name: 'Show password' }))
      expect(ref).toHaveBeenCalledTimes(1)
      expect(screen.getByRole('button', { name: 'Hide password' })).toHaveFocus()
      rerender(
        <InputExplorerNames
          label="Password"
          type="password"
          allowPasswordReveal
          ref={ref}
          error="Invalid"
        />
      )
      expect(ref).toHaveBeenCalledTimes(1)
      expect(screen.getByLabelText('Password')).toBe(input)
      unmount()
      expect(ref).toHaveBeenLastCalledWith(null)
    })
  })

  describe('and it allows password reveal', () => {
    beforeEach(() => {
      render(<InputExplorerNames label="Password" type="password" allowPasswordReveal />)
    })

    it('should use an action name rather than a second pressed-state interpretation', async () => {
      expect(screen.getByRole('button', { name: 'Show password' })).not.toHaveAttribute(
        'aria-pressed'
      )
      await userEvent.setup().click(screen.getByRole('button', { name: 'Show password' }))
      expect(screen.getByRole('button', { name: 'Hide password' })).not.toHaveAttribute(
        'aria-pressed'
      )
    })
  })
})

describe('when the public ref changes', () => {
  let first: RefObject<HTMLInputElement | null>
  let second: RefObject<HTMLInputElement | null>
  let input: HTMLElement

  beforeEach(() => {
    first = createRef<HTMLInputElement>()
    second = createRef<HTMLInputElement>()
    const { rerender } = render(<InputExplorerNames label="Name" ref={first} />)
    input = screen.getByRole('textbox')
    rerender(<InputExplorerNames label="Name" ref={second} />)
  })

  it('should clear the previous owner', () => {
    expect(first.current).toBeNull()
  })

  it('should update the new ref', () => {
    expect(second.current).toBe(input)
  })
})
