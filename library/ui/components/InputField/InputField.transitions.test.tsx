import { render, screen, type RenderResult } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, it, expect } from 'vitest'
import { InputExplorerNames } from '../InputExplorerNames/InputExplorerNames'

describe('when password reveal eligibility changes', () => {
  let rerender: RenderResult['rerender']

  beforeEach(async () => {
    ;({ rerender } = render(
      <InputExplorerNames
        label="Password"
        type="password"
        defaultValue="secret"
        allowPasswordReveal
      />
    ))
    await userEvent.setup().click(screen.getByRole('button', { name: 'Show password' }))
  })

  describe('and the reveal permission is removed', () => {
    beforeEach(() => {
      rerender(
        <InputExplorerNames
          label="Password"
          type="password"
          defaultValue="secret"
          allowPasswordReveal={false}
        />
      )
    })

    it('should mask immediately and drop the reveal button', () => {
      expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password')
      expect(screen.queryByRole('button')).toBeNull()
    })

    describe('and the reveal permission is restored', () => {
      beforeEach(() => {
        rerender(
          <InputExplorerNames
            label="Password"
            type="password"
            defaultValue="secret"
            allowPasswordReveal
          />
        )
      })

      it('should stay masked and keep the value', () => {
        expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password')
        expect(screen.getByLabelText('Password')).toHaveValue('secret')
      })
    })
  })

  describe('and the native type changes to text and back', () => {
    beforeEach(() => {
      rerender(
        <InputExplorerNames
          label="Password"
          type="text"
          defaultValue="secret"
          allowPasswordReveal
        />
      )
      rerender(
        <InputExplorerNames
          label="Password"
          type="password"
          defaultValue="secret"
          allowPasswordReveal
        />
      )
    })

    it('should discard the revealed state', () => {
      expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password')
      expect(screen.getByRole('button', { name: 'Show password' })).not.toHaveAttribute(
        'aria-pressed'
      )
    })
  })

  describe('and the field is disabled', () => {
    beforeEach(() => {
      rerender(
        <InputExplorerNames
          label="Password"
          type="password"
          defaultValue="secret"
          allowPasswordReveal
          disabled
        />
      )
    })

    it('should mask while disabled', () => {
      expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password')
      expect(screen.getByRole('button', { name: 'Show password' })).toBeDisabled()
    })

    describe('and the field is re-enabled', () => {
      beforeEach(() => {
        rerender(
          <InputExplorerNames
            label="Password"
            type="password"
            defaultValue="secret"
            allowPasswordReveal
          />
        )
      })

      it('should require a fresh explicit reveal', () => {
        expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password')
      })
    })
  })
})

describe('when descriptions contain no meaningful text', () => {
  for (const value of ['', ' ', '\n\t'])
    describe(`and the description and error are ${JSON.stringify(value)}`, () => {
      let container: HTMLElement

      beforeEach(() => {
        ;({ container } = render(
          <>
            <p id="external">External</p>
            <InputExplorerNames
              label="Name"
              description={value}
              error={value}
              aria-describedby="external"
            />
          </>
        ))
      })

      it('should omit the blank descriptions and errors', () => {
        const input = screen.getByRole('textbox')
        expect(input).toHaveAccessibleDescription('External')
        expect(input).toHaveAttribute('aria-describedby', 'external')
        expect(input).not.toHaveAttribute('aria-invalid')
        expect(container.querySelector('[data-invalid]')).toBeNull()
      })
    })

  describe('and a validation message is given', () => {
    beforeEach(() => {
      render(<InputExplorerNames label="Name" error="Invalid name" />)
    })

    it('should associate the validation message without adding a second live channel', () => {
      expect(screen.getByRole('textbox')).toHaveAccessibleDescription('Invalid name')
      expect(screen.queryByRole('status')).toBeNull()
      expect(screen.queryByRole('alert')).toBeNull()
    })
  })
})
