import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { SitesFooter, type SitesFooterProps } from './SitesFooter'

function renderFooter(overrides: Partial<SitesFooterProps> = {}) {
  const onSubscribe = vi.fn(async () => undefined)
  const onChange = vi.fn()
  const props: SitesFooterProps = {
    wordmarkLabel: 'Decentraland',
    newsletter: {
      title: 'Get the weekly highlights in your inbox',
      emailLabel: 'Email',
      placeholder: 'Enter Your Email',
      submitLabel: 'Subscribe',
      sendingLabel: 'Sending…',
      successText: 'Subscribed!',
      invalidText: 'Enter a valid email address.',
      errorText: 'Something went wrong.',
      onSubscribe
    },
    connectLabel: 'Connect',
    menuLabel: 'Menu',
    columns: [
      {
        id: 'getting-started',
        title: 'Getting Started',
        links: [{ label: 'Contact Support', href: '/docs/player/faqs/contact-and-support/' }]
      },
      {
        id: 'resources',
        title: 'Resources',
        links: [{ label: 'Vote', href: 'https://dao.example', external: true }]
      }
    ],
    language: {
      listLabel: 'Language',
      triggerLabel: (current) => `Change language: ${current}`,
      locales: [
        { code: 'en', label: 'English' },
        { code: 'es', label: 'Español' },
        { code: 'fr', label: 'Français' }
      ],
      current: 'en',
      onChange
    },
    legal: [{ label: 'Privacy Policy', href: '/privacy' }],
    copyright: '© 2026 Decentraland',
    ...overrides
  }
  render(<SitesFooter {...props} />)
  return { onSubscribe, onChange }
}

describe('SitesFooter', () => {
  describe('when rendered', () => {
    it('should expose the wordmark, the named social links, the columns and the legal links', () => {
      renderFooter()

      const footer = screen.getByRole('contentinfo')
      expect(within(footer).getByRole('img', { name: 'Decentraland' })).toBeInTheDocument()
      expect(within(footer).getAllByRole('link', { name: 'Discord' })[0]).toHaveAttribute(
        'target',
        '_blank'
      )
      expect(within(footer).getAllByRole('link', { name: 'Contact Support' })[0]).toHaveAttribute(
        'href',
        '/docs/player/faqs/contact-and-support/'
      )
      expect(within(footer).getAllByRole('link', { name: 'Vote' })[0]).toHaveAttribute(
        'rel',
        'noopener noreferrer'
      )
      expect(within(footer).getByRole('link', { name: 'Privacy Policy' })).toHaveAttribute(
        'href',
        '/privacy'
      )
    })
  })

  describe('when a phone menu column is toggled', () => {
    it('should be a button that reports and controls its expanded list', async () => {
      renderFooter()

      const toggle = screen.getByRole('button', { name: 'Getting Started' })
      const list = document.getElementById(toggle.getAttribute('aria-controls')!)!
      expect(toggle).toHaveAttribute('aria-expanded', 'false')
      expect(list).not.toBeVisible()

      await userEvent.click(toggle)
      expect(toggle).toHaveAttribute('aria-expanded', 'true')
      expect(list).toBeVisible()

      await userEvent.click(screen.getByRole('button', { name: 'Resources' }))
      expect(toggle).toHaveAttribute('aria-expanded', 'false')
    })
  })

  describe('when the newsletter form is sent', () => {
    it('should reject an invalid address without calling the service', async () => {
      const { onSubscribe } = renderFooter()

      await userEvent.type(screen.getByRole('textbox', { name: 'Email' }), 'nope')
      await userEvent.click(screen.getByRole('button', { name: 'Subscribe' }))

      expect(onSubscribe).not.toHaveBeenCalled()
      expect(screen.getByRole('alert')).toHaveTextContent('Enter a valid email address.')
      expect(screen.getByRole('textbox', { name: 'Email' })).toHaveAttribute('aria-invalid', 'true')
    })

    it('should subscribe a valid address and confirm it', async () => {
      const { onSubscribe } = renderFooter()

      await userEvent.type(screen.getByRole('textbox', { name: 'Email' }), ' jane.doe@example.com ')
      await userEvent.click(screen.getByRole('button', { name: 'Subscribe' }))

      expect(onSubscribe).toHaveBeenCalledWith('jane.doe@example.com')
      expect(await screen.findByRole('status')).toHaveTextContent('Subscribed!')
      expect(screen.getByRole('textbox', { name: 'Email' })).toHaveValue('')
    })

    it('should report a failed subscription', async () => {
      const onSubscribe = vi.fn(async () => {
        throw new Error('down')
      })
      renderFooter({
        newsletter: {
          title: 't',
          emailLabel: 'Email',
          placeholder: '',
          submitLabel: 'Subscribe',
          sendingLabel: 'Sending…',
          successText: 'ok',
          invalidText: 'invalid',
          errorText: 'Something went wrong.',
          onSubscribe
        }
      })

      await userEvent.type(screen.getByRole('textbox', { name: 'Email' }), 'jane.doe@example.com')
      await userEvent.click(screen.getByRole('button', { name: 'Subscribe' }))

      expect(await screen.findByRole('alert')).toHaveTextContent('Something went wrong.')
    })
  })

  describe('when the language switcher is used', () => {
    it('should list the languages, mark the current one and hand a new choice to onChange', async () => {
      const { onChange } = renderFooter()

      await userEvent.click(screen.getByRole('button', { name: 'Change language: English' }))
      const options = within(screen.getByRole('listbox', { name: 'Language' })).getAllByRole(
        'option'
      )
      expect(options.map((option) => option.textContent)).toEqual([
        'English',
        'Español',
        'Français'
      ])
      expect(screen.getByRole('option', { name: 'English' })).toHaveAttribute(
        'aria-selected',
        'true'
      )

      await userEvent.click(screen.getByRole('option', { name: 'Français' }))
      expect(onChange).toHaveBeenCalledWith('fr')
      expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
    })

    it('should work from the keyboard and return focus to the trigger', async () => {
      const { onChange } = renderFooter()
      const user = userEvent.setup()

      const trigger = screen.getByRole('button', { name: 'Change language: English' })
      trigger.focus()
      await user.keyboard('{ArrowDown}')
      expect(screen.getByRole('option', { name: 'English' })).toHaveFocus()
      await user.keyboard('{ArrowDown}')
      expect(screen.getByRole('option', { name: 'Español' })).toHaveFocus()
      expect(screen.getAllByRole('option').filter((option) => option.tabIndex === 0)).toHaveLength(
        1
      )
      await user.keyboard('{Enter}')

      expect(onChange).toHaveBeenCalledWith('es')
      expect(trigger).toHaveFocus()
    })

    it('should not report picking the current language again, and close on Escape', async () => {
      const { onChange } = renderFooter()
      const user = userEvent.setup()

      await user.click(screen.getByRole('button', { name: 'Change language: English' }))
      await user.keyboard('{Escape}')
      expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
      await user.click(screen.getByRole('button', { name: 'Change language: English' }))
      await user.click(screen.getByRole('option', { name: 'English' }))
      expect(onChange).not.toHaveBeenCalled()
    })
  })

  describe('when a link is followed', () => {
    it('should report a column link, on desktop and in the phone menu, and a social link by platform', async () => {
      const onLinkClick = vi.fn()
      const onSocialClick = vi.fn()
      renderFooter({ onLinkClick, onSocialClick })
      const footer = screen.getByRole('contentinfo')
      footer.addEventListener('click', (event) => event.preventDefault())

      await userEvent.click(within(footer).getByRole('link', { name: 'Contact Support' }))
      await userEvent.click(within(footer).getByRole('button', { name: 'Getting Started' }))
      // The phone menu's copy of the link, now shown.
      await userEvent.click(within(footer).getAllByRole('link', { name: 'Contact Support' })[1]!)
      await userEvent.click(within(footer).getAllByRole('link', { name: 'Discord' })[0]!)

      expect(onLinkClick).toHaveBeenCalledTimes(2)
      expect(onLinkClick).toHaveBeenCalledWith(
        expect.objectContaining({ href: '/docs/player/faqs/contact-and-support/' })
      )
      expect(onSocialClick).toHaveBeenCalledWith('Discord')
    })

    it('should not report legal links, as source', async () => {
      const onLinkClick = vi.fn()
      renderFooter({ onLinkClick })
      const footer = screen.getByRole('contentinfo')
      footer.addEventListener('click', (event) => event.preventDefault())

      await userEvent.click(within(footer).getByRole('link', { name: 'Privacy Policy' }))
      expect(onLinkClick).not.toHaveBeenCalled()
    })
  })
})
