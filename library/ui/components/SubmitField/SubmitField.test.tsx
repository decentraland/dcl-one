import '@testing-library/jest-dom/vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { SubmitField, type SubmitFieldProps } from './SubmitField'

function setup(props: Partial<SubmitFieldProps> = {}) {
  const onChange = vi.fn()
  const onSubmit = vi.fn()
  render(
    <SubmitField
      aria-label="Email"
      value="ada@example.com"
      onChange={onChange}
      onSubmit={onSubmit}
      submitLabel="NEXT"
      canSubmit
      {...props}
    />
  )
  return { onChange, onSubmit, field: screen.getByRole('textbox', { name: 'Email' }) }
}

describe('SubmitField', () => {
  describe('when it can submit', () => {
    it('is a text field and a button, with no alert', () => {
      const { field } = setup()
      expect(field).toHaveAttribute('type', 'text')
      expect(field).not.toHaveAttribute('aria-invalid')
      expect(screen.getByRole('button', { name: 'NEXT' })).toBeEnabled()
      expect(screen.queryByRole('alert')).toBeNull()
      expect(field).not.toHaveAttribute('aria-describedby')
    })

    it('reports what is typed', () => {
      const { onChange, field } = setup()
      fireEvent.change(field, { target: { value: 'ada@example.org' } })
      expect(onChange).toHaveBeenCalledWith('ada@example.org')
    })

    it('submits from the button and from Enter, and from no other key', () => {
      const { onSubmit, field } = setup()
      fireEvent.click(screen.getByRole('button', { name: 'NEXT' }))
      fireEvent.keyDown(field, { key: 'Enter' })
      fireEvent.keyDown(field, { key: 'a' })
      expect(onSubmit).toHaveBeenCalledTimes(2)
    })

    it('passes the type, placeholder, autocomplete and length to the field', () => {
      const { field } = setup({
        type: 'email',
        placeholder: 'your@email.com',
        autoComplete: 'email',
        maxLength: 64
      })
      expect(field).toHaveAttribute('type', 'email')
      expect(field).toHaveAttribute('placeholder', 'your@email.com')
      expect(field).toHaveAttribute('autocomplete', 'email')
      expect(field).toHaveAttribute('maxlength', '64')
    })

    it('draws its icon and focuses the field when asked to', () => {
      const { field } = setup({ icon: <i data-testid="icon" />, autoFocus: true })
      expect(screen.getByTestId('icon')).toBeInTheDocument()
      expect(field).toHaveFocus()
    })
  })

  describe('when it cannot submit', () => {
    it('holds the button and Enter', () => {
      const { onSubmit, field } = setup({ canSubmit: false })
      expect(screen.getByRole('button', { name: 'NEXT' })).toBeDisabled()
      fireEvent.keyDown(field, { key: 'Enter' })
      expect(onSubmit).not.toHaveBeenCalled()
    })

    it('holds the field, the button and Enter while disabled', () => {
      const { onSubmit, field } = setup({ disabled: true })
      expect(field).toBeDisabled()
      expect(screen.getByRole('button', { name: 'NEXT' })).toBeDisabled()
      fireEvent.keyDown(field, { key: 'Enter' })
      expect(onSubmit).not.toHaveBeenCalled()
    })
  })

  describe('when it is busy', () => {
    it('disables the field, draws no button and ignores Enter', () => {
      const { onSubmit, field } = setup({ busy: true })
      expect(field).toBeDisabled()
      expect(screen.queryByRole('button')).toBeNull()
      fireEvent.keyDown(field, { key: 'Enter' })
      expect(onSubmit).not.toHaveBeenCalled()
    })
  })

  describe('when it has an error', () => {
    it('reports the field as invalid and announces the error after its icon', () => {
      const { field } = setup({ error: 'Invalid email', errorIcon: <i data-testid="warning" /> })
      expect(field).toHaveAttribute('aria-invalid', 'true')
      const alert = screen.getByRole('alert')
      expect(alert).toHaveTextContent('Invalid email')
      expect(alert).toContainElement(screen.getByTestId('warning'))
      expect(field).toHaveAccessibleDescription('Invalid email')
    })

    it('draws no error icon without an error', () => {
      setup({ errorIcon: <i data-testid="warning" /> })
      expect(screen.queryByTestId('warning')).toBeNull()
    })
  })
})
