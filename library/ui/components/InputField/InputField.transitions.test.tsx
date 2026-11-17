import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, it, expect } from 'vitest'
import { InputExplorerNames } from '../InputExplorerNames/InputExplorerNames'

describe('when password reveal eligibility changes', () => {
  it('should mask immediately and stay masked when reveal permission is removed and restored', async () => {
    const { rerender } = render(
      <InputExplorerNames
        label="Password"
        type="password"
        defaultValue="secret"
        allowPasswordReveal
      />
    )
    await userEvent.setup().click(screen.getByRole('button', { name: 'Show password' }))
    rerender(
      <InputExplorerNames
        label="Password"
        type="password"
        defaultValue="secret"
        allowPasswordReveal={false}
      />
    )
    expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password')
    expect(screen.queryByRole('button')).toBeNull()
    rerender(
      <InputExplorerNames
        label="Password"
        type="password"
        defaultValue="secret"
        allowPasswordReveal
      />
    )
    expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password')
    expect(screen.getByLabelText('Password')).toHaveValue('secret')
  })
  it('should discard a revealed state across native type changes', async () => {
    const { rerender } = render(
      <InputExplorerNames
        label="Password"
        type="password"
        defaultValue="secret"
        allowPasswordReveal
      />
    )
    await userEvent.setup().click(screen.getByRole('button', { name: 'Show password' }))
    rerender(
      <InputExplorerNames label="Password" type="text" defaultValue="secret" allowPasswordReveal />
    )
    rerender(
      <InputExplorerNames
        label="Password"
        type="password"
        defaultValue="secret"
        allowPasswordReveal
      />
    )
    expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password')
    expect(screen.getByRole('button', { name: 'Show password' })).not.toHaveAttribute(
      'aria-pressed'
    )
  })
  it('should mask while disabled and require a fresh explicit reveal after re-enabling', async () => {
    const { rerender } = render(
      <InputExplorerNames
        label="Password"
        type="password"
        defaultValue="secret"
        allowPasswordReveal
      />
    )
    await userEvent.setup().click(screen.getByRole('button', { name: 'Show password' }))
    rerender(
      <InputExplorerNames
        label="Password"
        type="password"
        defaultValue="secret"
        allowPasswordReveal
        disabled
      />
    )
    expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password')
    expect(screen.getByRole('button', { name: 'Show password' })).toBeDisabled()
    rerender(
      <InputExplorerNames
        label="Password"
        type="password"
        defaultValue="secret"
        allowPasswordReveal
      />
    )
    expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password')
  })
})

describe('when descriptions contain no meaningful text', () => {
  for (const value of ['', ' ', '\n\t'])
    it(`should omit blank descriptions and errors for ${JSON.stringify(value)}`, () => {
      const { container } = render(
        <>
          <p id="external">External</p>
          <InputExplorerNames
            label="Name"
            description={value}
            error={value}
            aria-describedby="external"
          />
        </>
      )
      const input = screen.getByRole('textbox')
      expect(input).toHaveAccessibleDescription('External')
      expect(input).toHaveAttribute('aria-describedby', 'external')
      expect(input).not.toHaveAttribute('aria-invalid')
      expect(container.querySelector('[data-invalid]')).toBeNull()
    })
  it('should associate a validation message without adding a second live channel', () => {
    render(<InputExplorerNames label="Name" error="Invalid name" />)
    expect(screen.getByRole('textbox')).toHaveAccessibleDescription('Invalid name')
    expect(screen.queryByRole('status')).toBeNull()
    expect(screen.queryByRole('alert')).toBeNull()
  })
})
