// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import { HelpChatBanner } from './HelpChatBanner'

const EMAIL = { address: 'support@example.com', label: 'Email support' }
const COPY = { title: 'Need help?', subtitle: 'Contact the support team.', actionLabel: 'Chat Now' }

afterEach(cleanup)

describe('HelpChatBanner', () => {
  describe('when the email-only banner first appears', () => {
    beforeEach(() => {
      const view = render(<button type="button">Keep focus here</button>)
      screen.getByRole('button', { name: 'Keep focus here' }).focus()
      view.rerender(
        <>
          <button type="button">Keep focus here</button>
          <HelpChatBanner {...COPY} emailAction={EMAIL} />
        </>
      )
    })

    it('should offer only the support email action', () => {
      expect(screen.getByRole('link', { name: EMAIL.label })).toHaveAttribute(
        'href',
        'mailto:support@example.com'
      )
      expect(screen.queryByRole('button', { name: COPY.actionLabel })).toBeNull()
    })

    it('should leave focus outside the banner', () => {
      expect(screen.getByRole('button', { name: 'Keep focus here' })).toHaveFocus()
    })
  })

  describe('when chat becomes unavailable after the email option is shown', () => {
    beforeEach(() => {
      const view = render(<HelpChatBanner {...COPY} emailAction={EMAIL} onAction={() => {}} />)
      screen.getByRole('button', { name: COPY.actionLabel }).focus()
      view.rerender(<HelpChatBanner {...COPY} emailAction={EMAIL} />)
    })

    it('should move focus from the removed chat button to the existing email action', () => {
      expect(screen.getByRole('link', { name: EMAIL.label })).toHaveFocus()
    })
  })
})
