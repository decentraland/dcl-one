import { useState } from 'react'
import { renderToString } from 'react-dom/server'
import { fireEvent, render, screen, type RenderResult } from '@testing-library/react'
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  vi,
  type Mock,
  type MockInstance
} from 'vitest'
import type { NavBarLinkProps } from '../../../components/NavBar/NavBar'
import defaultThumbnail from '../assets/dcl-logo.svg'
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

describe('SitesNotificationsDialog', () => {
  let show: Mock<(this: HTMLDialogElement) => void>
  let close: Mock<(this: HTMLDialogElement) => void>
  let view: RenderResult

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

  afterEach(() => {
    vi.restoreAllMocks()
    vi.resetAllMocks()
  })

  describe('when rendered on the server', () => {
    let markup: string

    beforeEach(() => {
      markup = renderToString(<SitesNotificationsDialog {...props} />)
    })

    it('should render the dialog markup', () => {
      expect(markup).toContain('Notifications')
    })

    it('should not open the browser modal', () => {
      expect(show).not.toHaveBeenCalled()
    })

    it('should not close the browser modal', () => {
      expect(close).not.toHaveBeenCalled()
    })
  })

  describe('when opened from a focused opener', () => {
    let opener: HTMLButtonElement
    let focus: MockInstance<HTMLButtonElement['focus']>

    beforeEach(() => {
      opener = document.createElement('button')
      document.body.append(opener)
      opener.focus()
    })

    afterEach(() => {
      opener.remove()
    })

    describe('and the opener stays connected', () => {
      beforeEach(() => {
        focus = vi.spyOn(opener, 'focus')
        view = render(<SitesNotificationsDialog {...props} />)
        view.rerender(<SitesNotificationsDialog {...props} state="ready" />)
      })

      it('should open the modal once across rerenders', () => {
        expect(show).toHaveBeenCalledOnce()
      })

      it('should render a single dialog', () => {
        expect(screen.getAllByRole('dialog', { name: 'Notifications' })).toHaveLength(1)
      })

      describe('and the dialog unmounts', () => {
        beforeEach(() => {
          view.unmount()
        })

        it('should close the modal once', () => {
          expect(close).toHaveBeenCalledOnce()
        })

        it('should restore focus to the opener', () => {
          expect(focus).toHaveBeenCalledOnce()
        })
      })
    })

    describe('and the opener is disconnected before the dialog unmounts', () => {
      beforeEach(() => {
        view = render(<SitesNotificationsDialog {...props} />)
        opener.remove()
        focus = vi.spyOn(opener, 'focus')
        view.unmount()
      })

      it('should not restore focus to the opener', () => {
        expect(focus).not.toHaveBeenCalled()
      })
    })
  })

  describe('when the onClose callback changes after mounting', () => {
    let previous: Mock<() => void>
    let current: Mock<() => void>
    let dialog: HTMLElement

    beforeEach(() => {
      previous = vi.fn()
      current = vi.fn()
      view = render(<SitesNotificationsDialog {...props} onClose={previous} />)
      view.rerender(<SitesNotificationsDialog {...props} onClose={current} />)
      dialog = screen.getByRole('dialog')
      vi.spyOn(dialog, 'getBoundingClientRect').mockReturnValue({
        left: 100,
        right: 300,
        top: 100,
        bottom: 300
      } as DOMRect)
    })

    describe('and the panel padding is clicked', () => {
      beforeEach(() => {
        fireEvent.click(dialog, { clientX: 110, clientY: 110 })
      })

      it('should not dismiss the dialog', () => {
        expect(current).not.toHaveBeenCalled()
      })
    })

    describe('and the dialog is dismissed by an outside-box click, cancel and the Close button', () => {
      beforeEach(() => {
        fireEvent.click(dialog, { clientX: 110, clientY: 110 })
        fireEvent.click(dialog, { clientX: 10, clientY: 10 })
        fireEvent(dialog, new Event('cancel', { cancelable: true }))
        fireEvent.click(screen.getByRole('button', { name: 'Close' }))
      })

      it('should call the current callback for each dismissal', () => {
        expect(current).toHaveBeenCalledTimes(3)
      })

      it('should not call the previous callback', () => {
        expect(previous).not.toHaveBeenCalled()
      })
    })
  })

  describe('when Escape is pressed on a focused control', () => {
    let dismiss: Mock<() => void>
    let outer: Mock<(event: KeyboardEvent) => void>

    beforeEach(() => {
      dismiss = vi.fn()
      outer = vi.fn()
      document.addEventListener('keydown', outer)
      render(<SitesNotificationsDialog {...props} onClose={dismiss} />)
      const button = screen.getByRole('button', { name: 'Close' })
      button.focus()
      fireEvent.keyDown(button, { key: 'Escape' })
    })

    afterEach(() => {
      document.removeEventListener('keydown', outer)
    })

    it('should dismiss the dialog once', () => {
      expect(dismiss).toHaveBeenCalledOnce()
    })

    it('should own the key before outer document handlers', () => {
      expect(outer).not.toHaveBeenCalled()
    })
  })

  describe('when a child claims Escape', () => {
    let dismiss: Mock<() => void>

    beforeEach(() => {
      dismiss = vi.fn()
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
    })

    it('should not dismiss the dialog', () => {
      expect(dismiss).not.toHaveBeenCalled()
    })

    it('should keep the dialog visible', () => {
      expect(screen.getByRole('dialog', { name: 'Notifications' })).toBeVisible()
    })
  })

  describe('when the notifications are loading', () => {
    beforeEach(() => {
      view = render(<SitesNotificationsDialog {...props} />)
    })

    it('should announce the loading status', () => {
      expect(screen.getByRole('status')).toHaveTextContent('Loading notifications')
    })

    describe('and the load fails', () => {
      beforeEach(() => {
        view.rerender(<SitesNotificationsDialog {...props} state="error" />)
      })

      it('should alert the load error', () => {
        expect(screen.getByRole('alert')).toHaveTextContent('Couldn’t load notifications.')
      })

      it('should drop the loading status', () => {
        expect(screen.queryByRole('status')).toBeNull()
      })

      describe('and a retry succeeds with no notifications', () => {
        beforeEach(() => {
          view.rerender(<SitesNotificationsDialog {...props} state="ready" />)
        })

        it('should show the empty state', () => {
          expect(screen.getByRole('heading', { name: 'No Notifications Yet' })).toBeVisible()
        })

        it('should link to the Decentraland Foundation Community', () => {
          expect(
            screen.getByRole('link', { name: 'Decentraland Foundation Community' })
          ).toHaveAttribute('href', '/social/communities/e99471aa-31c4-4952-abf6-99905445f43b')
        })

        describe('and read and unread notifications arrive', () => {
          beforeEach(() => {
            view.rerender(
              <SitesNotificationsDialog
                {...props}
                state="ready"
                rows={[
                  row,
                  { ...row, id: 'two', title: 'Read item', href: undefined, unread: false }
                ]}
              />
            )
          })

          it('should link a notification that has a destination', () => {
            expect(screen.getByRole('link', { name: /Notification title/ })).toHaveAttribute(
              'href',
              '/shop'
            )
          })

          it('should not mark a read notification as unread', () => {
            expect(screen.getByText('Read item').closest('li')).not.toHaveAttribute('data-unread')
          })

          it('should mark an unread notification as unread', () => {
            expect(screen.getByText(row.title).closest('li')).toHaveAttribute('data-unread', 'true')
          })

          it('should keep a single dialog', () => {
            expect(screen.getAllByRole('dialog')).toHaveLength(1)
          })

          it('should drop the empty state', () => {
            expect(screen.queryByText('No Notifications Yet')).toBeNull()
          })
        })
      })
    })
  })

  describe('when a notification thumbnail fails to load', () => {
    let image: HTMLImageElement

    beforeEach(() => {
      render(
        <SitesNotificationsDialog
          {...props}
          state="ready"
          rows={[{ ...row, image: '/missing.png' }]}
        />
      )
      image = screen.getByRole('listitem').querySelector('img')!
    })

    it('should first request the notification thumbnail', () => {
      expect(image).toHaveAttribute('src', '/missing.png')
    })

    describe('and the image errors', () => {
      beforeEach(() => {
        fireEvent.error(image)
      })

      it('should replace it with the default image', () => {
        expect(image).toHaveAttribute('src', defaultThumbnail)
      })
    })
  })

  describe('when the owner closes the dialog on cancel', () => {
    beforeEach(() => {
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
    })

    it('should not leave a dialog mounted', () => {
      expect(screen.queryByRole('dialog')).toBeNull()
    })

    it("should render the owner's closed state", () => {
      expect(screen.getByText('Closed')).toBeVisible()
    })

    it('should close the modal once', () => {
      expect(close).toHaveBeenCalledOnce()
    })
  })

  describe('when a refresh fails with cached notifications', () => {
    beforeEach(() => {
      render(<SitesNotificationsDialog {...props} state="error" rows={[row]} />)
    })

    it('should alert an inline refresh error', () => {
      expect(screen.getByRole('alert')).toHaveTextContent(
        'Couldn’t refresh notifications. Showing previous notifications.'
      )
    })

    it('should keep the cached notifications visible', () => {
      expect(screen.getByRole('link', { name: /Notification title/ })).toHaveAttribute(
        'href',
        '/shop'
      )
    })
  })

  describe('when the notification popover has a different first button', () => {
    beforeEach(() => {
      function Host() {
        const [open, setOpen] = useState(false)
        return (
          <div>
            <button type="button">Unrelated action</button>
            <button type="button" onClick={() => setOpen(true)}>
              Open notifications
            </button>
            {open ? (
              <SitesNotificationsDialog
                {...props}
                presentation="popover"
                onClose={() => setOpen(false)}
              />
            ) : null}
          </div>
        )
      }
      render(<Host />)
    })

    it('should restore focus to the actual opener after closing', () => {
      const opener = screen.getByRole('button', { name: 'Open notifications' })
      opener.focus()
      fireEvent.click(opener)
      const dismiss = screen.getByRole('button', { name: 'Close' })
      dismiss.focus()
      fireEvent.click(dismiss)
      expect(opener).toHaveFocus()
    })
  })
})
