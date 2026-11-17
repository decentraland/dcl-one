import { createRef } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, it, expect, vi } from 'vitest'
import { InputExplorerNames } from '../InputExplorerNames/InputExplorerNames'
describe('when an input owns native focus and exposes its ref', () => {
  it('should not detach a stable callback ref or move focus on an internal reveal update', async () => {
    const ref = vi.fn((node: HTMLInputElement | null) => node?.focus())
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
  it('should use an action name rather than a second pressed-state interpretation', async () => {
    render(<InputExplorerNames label="Password" type="password" allowPasswordReveal />)
    expect(screen.getByRole('button', { name: 'Show password' })).not.toHaveAttribute(
      'aria-pressed'
    )
    await userEvent.setup().click(screen.getByRole('button', { name: 'Show password' }))
    expect(screen.getByRole('button', { name: 'Hide password' })).not.toHaveAttribute(
      'aria-pressed'
    )
  })
})

it('should update a changed public ref and clear the previous owner', () => {
  const first = createRef<HTMLInputElement>(),
    second = createRef<HTMLInputElement>()
  const { rerender } = render(<InputExplorerNames label="Name" ref={first} />)
  const input = screen.getByRole('textbox')
  rerender(<InputExplorerNames label="Name" ref={second} />)
  expect(first.current).toBeNull()
  expect(second.current).toBe(input)
})
