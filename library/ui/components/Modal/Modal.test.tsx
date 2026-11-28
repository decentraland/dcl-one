import { useRef, useState } from 'react'
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderToString } from 'react-dom/server'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { Modal } from './Modal'

// jsdom has no modal dialogs: opening and closing only toggle `open`, so the dialog's role shows.
const showModal = vi.fn(function (this: HTMLDialogElement) {
  this.open = true
})
const close = vi.fn(function (this: HTMLDialogElement) {
  this.open = false
})
const nativeShowModal = HTMLDialogElement.prototype.showModal
const nativeClose = HTMLDialogElement.prototype.close

beforeEach(() => {
  showModal.mockClear()
  close.mockClear()
  HTMLDialogElement.prototype.showModal = showModal
  HTMLDialogElement.prototype.close = close
})

afterEach(() => {
  cleanup()
  HTMLDialogElement.prototype.showModal = nativeShowModal
  HTMLDialogElement.prototype.close = nativeClose
})

const outside = { clientX: -10, clientY: -10 }

function pressBackdrop(dialog: HTMLElement) {
  fireEvent.pointerDown(dialog, outside)
  fireEvent.click(dialog, outside)
}

describe('Modal', () => {
  describe('when it is mounted', () => {
    it('opens as a modal dialog named by its title and described by its text', () => {
      render(
        <Modal title="Leave the program?" description="You lose your credits." onClose={() => {}} />
      )
      const dialog = screen.getByRole('dialog', { name: 'Leave the program?' })
      expect(showModal).toHaveBeenCalledTimes(1)
      expect(dialog).toHaveAccessibleDescription('You lose your credits.')
      expect(
        within(dialog).getByRole('heading', { level: 2, name: 'Leave the program?' })
      ).toBeVisible()
    })

    it('has no description when it has no text', () => {
      render(<Modal title="Saved" onClose={() => {}} />)
      expect(screen.getByRole('dialog', { name: 'Saved' })).not.toHaveAttribute('aria-describedby')
    })

    it('keeps the caller’s description beside its own', () => {
      render(
        <>
          <p id="more">More detail.</p>
          <Modal title="Saved" description="Done." aria-describedby="more" onClose={() => {}} />
        </>
      )
      expect(screen.getByRole('dialog')).toHaveAccessibleDescription('Done. More detail.')
    })

    it('stops the page behind it from scrolling, and lets it scroll again when it closes', () => {
      document.documentElement.style.overflow = 'clip'
      const { unmount } = render(<Modal title="Saved" onClose={() => {}} />)
      expect(document.documentElement.style.overflow).toBe('hidden')
      unmount()
      expect(document.documentElement.style.overflow).toBe('clip')
      document.documentElement.style.overflow = ''
    })

    it('closes the native dialog when it is removed', () => {
      const { unmount } = render(<Modal title="Saved" onClose={() => {}} />)
      unmount()
      expect(close).toHaveBeenCalledTimes(1)
    })

    it('shows the dialog in a browser without modal dialogs', () => {
      // @ts-expect-error an old browser has no showModal
      HTMLDialogElement.prototype.showModal = undefined
      render(<Modal title="Saved" onClose={() => {}} />)
      expect(screen.getByRole('dialog', { name: 'Saved' })).toHaveAttribute('open')
    })
  })

  describe('when the title is hidden', () => {
    it('still names the dialog', () => {
      render(<Modal title="Collection in review" titleHidden onClose={() => {}} />)
      expect(screen.getByRole('dialog', { name: 'Collection in review' })).toBeInTheDocument()
      expect(screen.getByRole('heading', { name: 'Collection in review' })).toHaveAttribute(
        'data-hidden'
      )
    })
  })

  describe('when the reader closes it', () => {
    it.each([
      [
        'the close button',
        (dialog: HTMLElement) =>
          fireEvent.click(within(dialog).getByRole('button', { name: 'Close' }))
      ],
      ['Escape', (dialog: HTMLElement) => fireEvent.keyDown(dialog, { key: 'Escape' })],
      [
        'the browser’s cancel request',
        (dialog: HTMLElement) => fireEvent(dialog, new Event('cancel', { cancelable: true }))
      ],
      ['a press on the backdrop', pressBackdrop],
      [
        'a close the browser forces',
        (dialog: HTMLElement) => {
          ;(dialog as HTMLDialogElement).open = false
          fireEvent(dialog, new Event('close'))
        }
      ]
    ])('calls onClose once for %s', (_how, act) => {
      const onClose = vi.fn()
      render(<Modal title="Saved" onClose={onClose} />)
      act(screen.getByRole('dialog'))
      expect(onClose).toHaveBeenCalledTimes(1)
    })

    it('stops the browser from closing the dialog by itself, so the caller decides', () => {
      render(<Modal title="Saved" onClose={() => {}} />)
      const cancel = new Event('cancel', { cancelable: true })
      fireEvent(screen.getByRole('dialog'), cancel)
      expect(cancel.defaultPrevented).toBe(true)
    })

    it('stays open for another key, or an Escape a control inside already used', () => {
      const onClose = vi.fn()
      render(
        <Modal title="Saved" onClose={onClose}>
          <input aria-label="Name" onKeyDown={(event) => event.preventDefault()} />
        </Modal>
      )
      fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Enter' })
      fireEvent.keyDown(screen.getByRole('textbox', { name: 'Name' }), { key: 'Escape' })
      expect(onClose).not.toHaveBeenCalled()
    })

    it('stays open on a click on its own padding, or a drag that ends on the backdrop', () => {
      const onClose = vi.fn()
      render(<Modal title="Saved" onClose={onClose} />)
      const dialog = screen.getByRole('dialog')
      vi.spyOn(dialog, 'getBoundingClientRect').mockReturnValue({
        left: 0,
        top: 0,
        right: 100,
        bottom: 100
      } as DOMRect)
      fireEvent.pointerDown(dialog, { clientX: 50, clientY: 50 })
      fireEvent.click(dialog, { clientX: 50, clientY: 50 })
      fireEvent.pointerDown(dialog, { clientX: 50, clientY: 50 })
      fireEvent.click(dialog, { clientX: 150, clientY: 150 })
      expect(onClose).not.toHaveBeenCalled()
    })

    it('uses the caller’s name for the close button', () => {
      render(<Modal title="Guardado" closeLabel="Cerrar" onClose={() => {}} />)
      expect(screen.getByRole('button', { name: 'Cerrar' })).toBeInTheDocument()
    })
  })

  describe('when it has no close button', () => {
    it('draws none, and Escape still closes it', () => {
      const onClose = vi.fn()
      render(<Modal title="Saved" closeButton={false} onClose={onClose} />)
      expect(screen.queryByRole('button', { name: 'Close' })).toBeNull()
      fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' })
      expect(onClose).toHaveBeenCalledTimes(1)
    })
  })

  describe('when the backdrop does not close it', () => {
    it('stays open on a press on the backdrop, and Escape and the close button still close it', () => {
      const onClose = vi.fn()
      render(<Modal title="Replay" closeOnBackdrop={false} onClose={onClose} />)
      const dialog = screen.getByRole('dialog')
      pressBackdrop(dialog)
      expect(onClose).not.toHaveBeenCalled()
      fireEvent.keyDown(dialog, { key: 'Escape' })
      fireEvent.click(within(dialog).getByRole('button', { name: 'Close' }))
      expect(onClose).toHaveBeenCalledTimes(2)
    })
  })

  describe('when it must stay open', () => {
    it('ignores Escape, the cancel request and the backdrop, and disables the close button', () => {
      const onClose = vi.fn()
      render(<Modal title="Leaving" dismissible={false} onClose={onClose} />)
      const dialog = screen.getByRole('dialog')
      fireEvent.keyDown(dialog, { key: 'Escape' })
      fireEvent(dialog, new Event('cancel', { cancelable: true }))
      pressBackdrop(dialog)
      expect(onClose).not.toHaveBeenCalled()
      expect(within(dialog).getByRole('button', { name: 'Close' })).toBeDisabled()
    })

    it('shows the dialog again after a close the browser forces', () => {
      const onClose = vi.fn()
      render(<Modal title="Leaving" dismissible={false} onClose={onClose} />)
      const dialog = screen.getByRole('dialog') as HTMLDialogElement
      dialog.open = false
      fireEvent(dialog, new Event('close'))
      expect(dialog.open).toBe(true)
      expect(onClose).not.toHaveBeenCalled()
    })
  })

  describe('when it closes', () => {
    function Page({ withRef = false }: { withRef?: boolean }) {
      const [open, setOpen] = useState(false)
      const other = useRef<HTMLButtonElement>(null)
      return (
        <>
          <button type="button" onClick={() => setOpen(true)}>
            Open
          </button>
          <button type="button" ref={other}>
            Other
          </button>
          {open ? (
            <Modal
              title="Saved"
              onClose={() => setOpen(false)}
              returnFocusRef={withRef ? other : undefined}
            />
          ) : null}
        </>
      )
    }

    it('returns focus to the control that opened it', async () => {
      const user = userEvent.setup()
      render(<Page />)
      await user.click(screen.getByRole('button', { name: 'Open' }))
      await user.click(screen.getByRole('button', { name: 'Close' }))
      expect(screen.queryByRole('dialog')).toBeNull()
      expect(screen.getByRole('button', { name: 'Open' })).toHaveFocus()
    })

    it('returns focus to the element the caller names', async () => {
      const user = userEvent.setup()
      render(<Page withRef />)
      await user.click(screen.getByRole('button', { name: 'Open' }))
      await user.click(screen.getByRole('button', { name: 'Close' }))
      expect(screen.getByRole('button', { name: 'Other' })).toHaveFocus()
    })
  })

  describe('when the caller names the first control', () => {
    it('moves focus to it when the dialog opens', () => {
      function Confirm() {
        const cancel = useRef<HTMLButtonElement>(null)
        return (
          <Modal
            title="Delete?"
            onClose={() => {}}
            initialFocusRef={cancel}
            actions={
              <>
                <button type="button" ref={cancel}>
                  Cancel
                </button>
                <button type="button">Delete</button>
              </>
            }
          />
        )
      }
      render(<Confirm />)
      expect(screen.getByRole('button', { name: 'Cancel' })).toHaveFocus()
    })
  })

  describe('when it has parts', () => {
    it('shows the picture, the content, the checkbox and the actions in reading order', async () => {
      const onCheckedChange = vi.fn()
      const user = userEvent.setup()
      render(
        <Modal
          title="Follow this link?"
          description="It opens in your browser."
          image={<img src="/alert.png" alt="" data-testid="picture" />}
          checkbox={{ label: 'Always allow this domain', onCheckedChange }}
          actions={
            <>
              <button type="button">Cancel</button>
              <button type="button">Continue</button>
            </>
          }
          divider
          onClose={() => {}}
        >
          <a href="https://example.org/">https://example.org/</a>
        </Modal>
      )
      const dialog = screen.getByRole('dialog', { name: 'Follow this link?' })
      const order = [
        screen.getByTestId('picture'),
        within(dialog).getByRole('heading'),
        within(dialog).getByRole('link'),
        within(dialog).getByRole('checkbox', { name: 'Always allow this domain' }),
        within(dialog).getByRole('separator'),
        within(dialog).getByRole('button', { name: 'Cancel' }),
        within(dialog).getByRole('button', { name: 'Continue' })
      ]
      order.slice(1).forEach((node, index) => {
        expect(
          order[index]!.compareDocumentPosition(node) & Node.DOCUMENT_POSITION_FOLLOWING
        ).toBeTruthy()
      })
      await user.click(within(dialog).getByRole('checkbox'))
      expect(onCheckedChange).toHaveBeenCalledWith(true)
    })

    it('draws no line, checkbox or actions it was not given', () => {
      render(<Modal title="Saved" onClose={() => {}} />)
      const dialog = screen.getByRole('dialog')
      expect(within(dialog).queryByRole('separator')).toBeNull()
      expect(within(dialog).queryByRole('checkbox')).toBeNull()
      expect(within(dialog).getAllByRole('button')).toHaveLength(1)
    })
  })

  it('renders on the server as a closed dialog with its title', () => {
    const html = renderToString(<Modal title="Saved" description="Done." onClose={() => {}} />)
    expect(html).toContain('Saved')
    expect(html).not.toMatch(/<dialog[^>]* open/)
  })
})

describe('when the modal displays a gallery', () => {
  it('should hide the visible title and retain the accessible name', () => {
    render(<Modal layout="gallery" title="Photo at the plaza" onClose={vi.fn()} />)
    const gallery = screen.getByRole('dialog', { name: 'Photo at the plaza' })
    expect(gallery).toHaveAttribute('data-layout', 'gallery')
    expect(within(gallery).getByRole('heading')).toHaveAttribute('data-hidden')
  })
})

describe('when a caller handles keys', () => {
  let onDismiss: ReturnType<typeof vi.fn<() => void>>

  beforeEach(() => {
    onDismiss = vi.fn<() => void>()
  })

  it('should call the handler before dismissing on Escape', () => {
    const onKeyDown = vi.fn()
    render(<Modal title="Photo" onClose={onDismiss} onKeyDown={onKeyDown} />)

    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' })

    expect(onKeyDown).toHaveBeenCalledOnce()
    expect(onDismiss).toHaveBeenCalledOnce()
    expect(onKeyDown.mock.invocationCallOrder[0]).toBeLessThan(
      onDismiss.mock.invocationCallOrder[0]!
    )
  })

  it('should let a caller consume Escape', () => {
    render(
      <Modal title="Photo" onClose={onDismiss} onKeyDown={(event) => event.preventDefault()} />
    )

    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' })

    expect(onDismiss).not.toHaveBeenCalled()
  })
})

describe('when the browser lacks showModal', () => {
  beforeEach(() => {
    // @ts-expect-error an old browser has no showModal
    HTMLDialogElement.prototype.showModal = undefined
    render(
      <Modal title="Gallery" closeButton={false} onClose={vi.fn()}>
        <button type="button">First</button>
        <button type="button">Last</button>
      </Modal>
    )
  })

  it('should wrap Tab from the last control to the first', () => {
    const last = screen.getByRole('button', { name: 'Last' })
    last.focus()

    fireEvent.keyDown(last, { key: 'Tab' })

    expect(screen.getByRole('button', { name: 'First' })).toHaveFocus()
  })

  it('should wrap Shift+Tab from the first control to the last', () => {
    const first = screen.getByRole('button', { name: 'First' })
    first.focus()

    fireEvent.keyDown(first, { key: 'Tab', shiftKey: true })

    expect(screen.getByRole('button', { name: 'Last' })).toHaveFocus()
  })
})

describe('when the modal shows an event or place detail', () => {
  it.each(['detail', 'scene'] as const)(
    'should draw the %s layout with a filled close button and the hidden title read before the cover',
    (layout) => {
      render(
        <Modal
          layout={layout}
          title="Genesis Plaza"
          image={<img src="cover.png" alt="" data-testid="cover" />}
          onClose={vi.fn()}
        >
          <p>What to expect</p>
        </Modal>
      )
      const dialog = screen.getByRole('dialog', { name: 'Genesis Plaza' })
      const heading = within(dialog).getByRole('heading', { level: 2, name: 'Genesis Plaza' })
      expect(dialog).toHaveAttribute('data-layout', layout)
      expect(dialog).not.toHaveAttribute('data-page')
      expect(heading).toHaveAttribute('data-hidden')
      expect(within(dialog).getByRole('button', { name: 'Close' })).toHaveAttribute(
        'data-variant',
        'filled'
      )
      const cover = within(dialog).getByTestId('cover')
      expect(heading.compareDocumentPosition(cover) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
      expect(within(dialog).getByText('What to expect')).toBeVisible()
    }
  )

  it('should keep a description the dialog is described by', () => {
    render(
      <Modal
        layout="detail"
        title="Genesis Plaza"
        description="By the Foundation"
        onClose={vi.fn()}
      />
    )
    expect(screen.getByRole('dialog')).toHaveAccessibleDescription('By the Foundation')
  })

  it('should leave the message layout untouched: no layout attribute and the plain close button', () => {
    render(<Modal title="Saved" onClose={vi.fn()} />)
    const dialog = screen.getByRole('dialog', { name: 'Saved' })
    expect(dialog).not.toHaveAttribute('data-layout')
    expect(within(dialog).getByRole('button', { name: 'Close' })).toHaveAttribute(
      'data-variant',
      'plain'
    )
    expect(within(dialog).getByRole('heading', { name: 'Saved' })).not.toHaveAttribute(
      'data-hidden'
    )
  })
})

describe("when the modal is a page's own content", () => {
  const show = vi.fn(function (this: HTMLDialogElement) {
    this.open = true
  })
  const nativeShow = HTMLDialogElement.prototype.show
  const nativeMatchMedia = window.matchMedia
  let listeners: Array<() => void>
  let phone: boolean

  beforeEach(() => {
    show.mockClear()
    HTMLDialogElement.prototype.show = show
    listeners = []
    phone = false
    window.matchMedia = vi.fn((query: string) => ({
      get matches() {
        return phone
      },
      media: query,
      addEventListener: (_: string, listener: () => void) => listeners.push(listener),
      removeEventListener: (_: string, listener: () => void) => {
        listeners = listeners.filter((l) => l !== listener)
      }
    })) as unknown as typeof window.matchMedia
  })

  afterEach(() => {
    HTMLDialogElement.prototype.show = nativeShow
    window.matchMedia = nativeMatchMedia
    document.documentElement.style.overflow = ''
  })

  const page = () => <Modal layout="scene" pageContent title="Genesis Plaza" onClose={vi.fn()} />

  it('should render on the server shown, its title the page heading', () => {
    const html = renderToString(page())
    expect(html).toMatch(/<dialog[^>]* open=""/)
    expect(html).toMatch(/<h1[^>]*>Genesis Plaza<\/h1>/)
  })

  it('should reopen it as a modal dialog above a phone, locking the page scroll', () => {
    render(page())
    expect(close).toHaveBeenCalledTimes(1)
    expect(showModal).toHaveBeenCalledTimes(1)
    expect(show).not.toHaveBeenCalled()
    expect(document.documentElement.style.overflow).toBe('hidden')
    expect(screen.getByRole('heading', { level: 1, name: 'Genesis Plaza' })).toBeInTheDocument()
  })

  it('should keep it in the page on a phone, without a modal or a scroll lock', () => {
    phone = true
    render(page())
    expect(showModal).not.toHaveBeenCalled()
    expect(close).not.toHaveBeenCalled()
    expect(document.documentElement.style.overflow).toBe('')
    expect(screen.getByRole('dialog', { name: 'Genesis Plaza' })).toHaveAttribute('open')
  })

  it('should move between the page and the top layer when the width crosses a phone', () => {
    render(page())
    phone = true
    act(() => listeners.forEach((listener) => listener()))
    expect(close).toHaveBeenCalledTimes(2)
    expect(show).toHaveBeenCalledTimes(1)
    expect(document.documentElement.style.overflow).toBe('')
    phone = false
    act(() => listeners.forEach((listener) => listener()))
    expect(showModal).toHaveBeenCalledTimes(2)
    expect(document.documentElement.style.overflow).toBe('hidden')
  })

  it('should stop listening to the width once it is removed', () => {
    const { unmount } = render(page())
    expect(listeners).toHaveLength(1)
    unmount()
    expect(listeners).toHaveLength(0)
  })

  it('should open as a modal dialog where the browser cannot match media', () => {
    // @ts-expect-error an old browser has no matchMedia
    window.matchMedia = undefined
    render(page())
    expect(showModal).toHaveBeenCalledTimes(1)
  })

  it('should ignore pageContent outside the scene layout', () => {
    const html = renderToString(
      <Modal layout="detail" pageContent title="Genesis Plaza" onClose={vi.fn()} />
    )
    expect(html).not.toMatch(/<dialog[^>]* open/)
    expect(html).toMatch(/<h2[^>]*>Genesis Plaza<\/h2>/)
  })
})
