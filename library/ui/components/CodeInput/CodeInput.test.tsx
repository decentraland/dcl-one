import '@testing-library/jest-dom/vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { CodeInput } from './CodeInput'

const field = () => screen.getByRole('textbox', { name: 'Verification code' })

describe('CodeInput', () => {
  describe('when it is empty', () => {
    it('is one numeric one-time-code field over six slots, the first marked current', () => {
      const { container } = render(<CodeInput value="" onChange={() => {}} />)
      expect(field()).toHaveAttribute('inputmode', 'numeric')
      expect(field()).toHaveAttribute('autocomplete', 'one-time-code')
      expect(field()).not.toHaveAttribute('aria-invalid')
      const slots = container.querySelectorAll('[aria-hidden="true"]')
      expect(slots).toHaveLength(6)
      expect(slots[0]).toHaveAttribute('data-current')
      expect(slots[1]).not.toHaveAttribute('data-current')
    })

    it('takes the name and the length it is given', () => {
      const { container } = render(
        <CodeInput value="" onChange={() => {}} length={4} aria-label="Code" />
      )
      expect(screen.getByRole('textbox', { name: 'Code' })).toBeInTheDocument()
      expect(container.querySelectorAll('[aria-hidden="true"]')).toHaveLength(4)
    })

    it('focuses the field when asked to', () => {
      render(<CodeInput value="" onChange={() => {}} autoFocus />)
      expect(field()).toHaveFocus()
    })
  })

  describe('when the player types', () => {
    it('keeps the digits only', () => {
      const onChange = vi.fn()
      render(<CodeInput value="" onChange={onChange} />)
      fireEvent.change(field(), { target: { value: '1a2-3' } })
      expect(onChange).toHaveBeenCalledWith('123')
    })

    it('cuts a pasted code at its length and completes it', () => {
      const onChange = vi.fn()
      const onComplete = vi.fn()
      render(<CodeInput value="" onChange={onChange} onComplete={onComplete} />)
      fireEvent.change(field(), { target: { value: '12345678' } })
      expect(onChange).toHaveBeenCalledWith('123456')
      expect(onComplete).toHaveBeenCalledWith('123456')
    })

    it('does not complete a code that is short', () => {
      const onComplete = vi.fn()
      render(<CodeInput value="" onChange={() => {}} onComplete={onComplete} />)
      fireEvent.change(field(), { target: { value: '12345' } })
      expect(onComplete).not.toHaveBeenCalled()
    })

    it('draws each digit in its slot and marks the next one current', () => {
      const { container } = render(<CodeInput value="12" onChange={() => {}} />)
      const slots = container.querySelectorAll('[aria-hidden="true"]')
      expect(slots[0]).toHaveTextContent('1')
      expect(slots[1]).toHaveTextContent('2')
      expect(slots[2]).toHaveAttribute('data-current')
    })

    it('keeps the last slot current once the code is full', () => {
      const { container } = render(<CodeInput value="123456" onChange={() => {}} />)
      const slots = container.querySelectorAll('[aria-hidden="true"]')
      expect(slots[5]).toHaveAttribute('data-current')
    })
  })

  describe('when the code was refused or accepted', () => {
    it('reports a refused code as invalid', () => {
      const { container } = render(<CodeInput value="123456" onChange={() => {}} state="error" />)
      expect(field()).toHaveAttribute('aria-invalid', 'true')
      expect(container.firstElementChild).toHaveAttribute('data-state', 'error')
    })

    it('does not report an accepted code as invalid', () => {
      const { container } = render(
        <CodeInput value="123456" onChange={() => {}} state="success" />
      )
      expect(field()).not.toHaveAttribute('aria-invalid')
      expect(container.firstElementChild).toHaveAttribute('data-state', 'success')
    })
  })

  describe('when it is disabled', () => {
    it('disables the field, marks no slot current and does not take the focus', () => {
      const { container } = render(<CodeInput value="12" onChange={() => {}} disabled autoFocus />)
      expect(field()).toBeDisabled()
      expect(field()).not.toHaveFocus()
      expect(container.querySelector('[data-current]')).toBeNull()
    })
  })
})
