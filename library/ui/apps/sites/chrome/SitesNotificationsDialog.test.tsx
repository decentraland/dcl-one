import { useState } from 'react'
import { renderToString } from 'react-dom/server'
import { fireEvent, render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { NavBarLinkProps } from '../../../components/NavBar/NavBar'
import SitesNotificationsDialog from './SitesNotificationsDialog'
const Link = (props: NavBarLinkProps) => <a {...props} />
const props = { onClose: () => {}, state: 'loading' as const, rows: [], Link }
const row = {
  id: 'one',
  title: 'Notification title',
  description: 'Description',
  time: 'now',
  unread: true,
  href: '/shop'
}
let show: ReturnType<typeof vi.fn>
let close: ReturnType<typeof vi.fn>
beforeEach(() => {
  show = vi.fn(function (this: HTMLDialogElement) {
    this.open = true
  })
  close = vi.fn(function (this: HTMLDialogElement) {
    this.open = false
  })
  HTMLDialogElement.prototype.showModal = show
  HTMLDialogElement.prototype.close = close
})
describe('Sites notifications presentation', () => {
  it('renders server markup without invoking browser modal methods', () => {
    expect(renderToString(<SitesNotificationsDialog {...props} />)).toContain('Notifications')
    expect(show).not.toHaveBeenCalled()
    expect(close).not.toHaveBeenCalled()
  })
  it('opens once, closes on unmount and restores a connected opener', () => {
    const opener = document.createElement('button')
    document.body.append(opener)
    opener.focus()
    const focus = vi.spyOn(opener, 'focus')
    const view = render(<SitesNotificationsDialog {...props} />)
    view.rerender(<SitesNotificationsDialog {...props} state="ready" />)
    expect(show).toHaveBeenCalledOnce()
    expect(screen.getAllByRole('dialog', { name: 'Notifications' })).toHaveLength(1)
    view.unmount()
    expect(close).toHaveBeenCalledOnce()
    expect(focus).toHaveBeenCalledOnce()
    opener.remove()
  })
  it('does not restore focus to a disconnected opener', () => {
    const opener = document.createElement('button')
    document.body.append(opener)
    opener.focus()
    const view = render(<SitesNotificationsDialog {...props} />)
    opener.remove()
    const focus = vi.spyOn(opener, 'focus')
    view.unmount()
    expect(focus).not.toHaveBeenCalled()
  })
  it('uses current callbacks for cancel and outside-box clicks, ignoring panel padding', () => {
    const previous = vi.fn()
    const current = vi.fn()
    const view = render(<SitesNotificationsDialog {...props} onClose={previous} />)
    view.rerender(<SitesNotificationsDialog {...props} onClose={current} />)
    const dialog = screen.getByRole('dialog')
    vi.spyOn(dialog, 'getBoundingClientRect').mockReturnValue({
      left: 100,
      right: 300,
      top: 100,
      bottom: 300
    } as DOMRect)
    fireEvent.click(dialog, { clientX: 110, clientY: 110 })
    expect(current).not.toHaveBeenCalled()
    fireEvent.click(dialog, { clientX: 10, clientY: 10 })
    fireEvent(dialog, new Event('cancel', { cancelable: true }))
    fireEvent.click(screen.getByRole('button', { name: 'Close' }))
    expect(current).toHaveBeenCalledTimes(3)
    expect(previous).not.toHaveBeenCalled()
  })
  it('owns focused Escape before outer document handlers and dismisses once', () => {
    const dismiss = vi.fn()
    const outer = vi.fn()
    document.addEventListener('keydown', outer)
    try {
      render(<SitesNotificationsDialog {...props} onClose={dismiss} />)
      const button = screen.getByRole('button', { name: 'Close' })
      button.focus()
      fireEvent.keyDown(button, { key: 'Escape' })
      expect(dismiss).toHaveBeenCalledOnce()
      expect(outer).not.toHaveBeenCalled()
    } finally {
      document.removeEventListener('keydown', outer)
    }
  })
  it('leaves Escape claimed by a child to that child', () => {
    const dismiss = vi.fn()
    const ChildLink = (input: NavBarLinkProps) => (
      <a {...input} onKeyDown={(event) => event.preventDefault()} />
    )
    render(
      <SitesNotificationsDialog
        {...props}
        onClose={dismiss}
        Link={ChildLink}
        state="ready"
        rows={[row]}
      />
    )
    const link = screen.getByRole('link')
    link.focus()
    fireEvent.keyDown(link, { key: 'Escape' })
    expect(dismiss).not.toHaveBeenCalled()
    expect(screen.getByRole('dialog', { name: 'Notifications' })).toBeVisible()
  })
  it('renders loading, error, empty and read/unread linked or plain rows', () => {
    const view = render(<SitesNotificationsDialog {...props} />)
    expect(screen.getByRole('status')).toHaveTextContent('Loading notifications')
    view.rerender(<SitesNotificationsDialog {...props} state="error" />)
    expect(screen.getByRole('alert')).toHaveTextContent('Couldn’t load notifications.')
    expect(screen.queryByRole('status')).toBeNull()
    view.rerender(<SitesNotificationsDialog {...props} state="ready" />)
    expect(screen.getByText('No notifications yet')).toBeVisible()
    view.rerender(
      <SitesNotificationsDialog
        {...props}
        state="ready"
        rows={[row, { ...row, id: 'two', title: 'Read item', href: undefined, unread: false }]}
      />
    )
    expect(screen.getByRole('link', { name: /Notification title/ })).toHaveAttribute(
      'href',
      '/shop'
    )
    expect(screen.getByText('Read item').closest('li')).not.toHaveAttribute('data-unread')
    expect(screen.getByText(row.title).closest('li')).toHaveAttribute('data-unread', 'true')
    expect(screen.getAllByRole('dialog')).toHaveLength(1)
    expect(screen.queryByText('No notifications yet')).toBeNull()
  })
  it('closes through the owner without leaving a modal mounted', () => {
    function Host() {
      const [open, setOpen] = useState(true)
      return open ? (
        <SitesNotificationsDialog {...props} onClose={() => setOpen(false)} />
      ) : (
        <span>Closed</span>
      )
    }
    render(<Host />)
    fireEvent(screen.getByRole('dialog'), new Event('cancel', { cancelable: true }))
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(screen.getByText('Closed')).toBeVisible()
    expect(close).toHaveBeenCalledOnce()
  })
})

it('keeps cached notification rows visible with an inline refresh error', () => {
  render(<SitesNotificationsDialog {...props} state="error" rows={[row]} />)
  expect(screen.getByRole('alert')).toHaveTextContent(
    'Couldn’t refresh notifications. Showing previous notifications.'
  )
  expect(screen.getByRole('link', { name: /Notification title/ })).toHaveAttribute('href', '/shop')
})
