import { render, screen, within } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import SitesFooter from './SitesFooter'

const docsUrl = (path = '') => `https://docs.example.org/${path}`

function newsletterForm() {
  return screen.getByRole('textbox', { name: 'Email' }).closest('form') as HTMLFormElement
}

describe('SitesFooter newsletter', () => {
  let user: UserEvent
  let onSubscribe: Mock<(email: string) => Promise<void>>

  beforeEach(() => {
    user = userEvent.setup()
    onSubscribe = vi.fn<(email: string) => Promise<void>>()
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  describe('when it renders', () => {
    beforeEach(() => {
      render(<SitesFooter docsUrl={docsUrl} onSubscribe={onSubscribe} />)
    })

    it('should label the email field', () => {
      expect(screen.getByRole('textbox', { name: 'Email' })).toHaveAttribute('type', 'email')
    })

    it('should show the placeholder', () => {
      expect(screen.getByPlaceholderText('Enter Your Email')).toBeInTheDocument()
    })

    it('should submit the form with the Subscribe button', () => {
      expect(within(newsletterForm()).getByRole('button', { name: 'Subscribe' })).toHaveAttribute(
        'type',
        'submit'
      )
    })
  })

  describe('when a valid address is submitted', () => {
    let resolve: () => void

    beforeEach(async () => {
      onSubscribe.mockImplementation(
        () =>
          new Promise<void>((done) => {
            resolve = done
          })
      )
      render(<SitesFooter docsUrl={docsUrl} onSubscribe={onSubscribe} />)
      await user.type(screen.getByRole('textbox', { name: 'Email' }), '  ada@example.org ')
      await user.click(screen.getByRole('button', { name: 'Subscribe' }))
    })

    it('should send the trimmed address', () => {
      expect(onSubscribe).toHaveBeenCalledExactlyOnceWith('ada@example.org')
    })

    it('should disable the field while it is sending', () => {
      expect(screen.getByRole('textbox', { name: 'Email' })).toBeDisabled()
    })

    it('should disable the button while it is sending', () => {
      expect(screen.getByRole('button', { name: 'Sending…' })).toBeDisabled()
    })

    describe('and the subscription succeeds', () => {
      beforeEach(async () => {
        resolve()
        await screen.findByRole('status')
      })

      it('should confirm the subscription', () => {
        expect(screen.getByRole('status')).toHaveTextContent('Subscribed!')
      })

      it('should clear the field', () => {
        expect(screen.getByRole('textbox', { name: 'Email' })).toHaveValue('')
      })

      it('should enable the button again', () => {
        expect(screen.getByRole('button', { name: 'Subscribe' })).toBeEnabled()
      })
    })
  })

  describe('when the subscription fails', () => {
    beforeEach(async () => {
      onSubscribe.mockRejectedValue(new Error('unavailable'))
      render(<SitesFooter docsUrl={docsUrl} onSubscribe={onSubscribe} />)
      await user.type(screen.getByRole('textbox', { name: 'Email' }), 'ada@example.org')
      await user.click(screen.getByRole('button', { name: 'Subscribe' }))
      await screen.findByRole('alert')
    })

    it('should report the failure', () => {
      expect(screen.getByRole('alert')).toHaveTextContent('Something went wrong. Please try again.')
    })

    it('should keep the address', () => {
      expect(screen.getByRole('textbox', { name: 'Email' })).toHaveValue('ada@example.org')
    })

    it('should enable the field again', () => {
      expect(screen.getByRole('textbox', { name: 'Email' })).toBeEnabled()
    })

    it('should enable the button again', () => {
      expect(screen.getByRole('button', { name: 'Subscribe' })).toBeEnabled()
    })
  })

  describe.each([
    ['empty', ''],
    ['without a domain', 'ada@'],
    ['with a space inside', 'ada lovelace@example.org']
  ])('when the address is %s', (_case, value) => {
    beforeEach(async () => {
      render(<SitesFooter docsUrl={docsUrl} onSubscribe={onSubscribe} />)
      if (value) await user.type(screen.getByRole('textbox', { name: 'Email' }), value)
      await user.click(screen.getByRole('button', { name: 'Subscribe' }))
    })

    it('should not subscribe', () => {
      expect(onSubscribe).not.toHaveBeenCalled()
    })

    it('should ask for a valid address', () => {
      expect(screen.getByRole('alert')).toHaveTextContent('Enter a valid email address.')
    })

    it('should keep the button enabled', () => {
      expect(screen.getByRole('button', { name: 'Subscribe' })).toBeEnabled()
    })

    describe('and the address is edited', () => {
      beforeEach(async () => {
        await user.type(screen.getByRole('textbox', { name: 'Email' }), 'x')
      })

      it('should clear the message', () => {
        expect(screen.queryByRole('alert')).toBeNull()
      })
    })
  })
})
