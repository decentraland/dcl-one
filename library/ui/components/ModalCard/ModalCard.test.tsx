import '@testing-library/jest-dom/vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { ModalActions, ModalCard, ModalShell, ModalTitle } from './ModalCard'

describe('ModalCard', () => {
  it('should be a named modal dialog of the given width that keeps a click from its host', () => {
    const onHostClick = vi.fn()
    render(
      <div onClick={onHostClick}>
        <ModalCard ariaLabel="Leave" width={500} className="mine">
          body
        </ModalCard>
      </div>
    )
    const card = screen.getByRole('dialog', { name: 'Leave' })
    expect(card).toHaveAttribute('aria-modal', 'true')
    expect(card.style.width).toBe('500px')
    expect(card).toHaveClass('mine')
    fireEvent.click(card)
    expect(onHostClick).not.toHaveBeenCalled()
  })

  it('should default to 420px and take another role and a non-modal state', () => {
    render(
      <ModalCard ariaLabel="Crash" role="alertdialog" modal={false}>
        body
      </ModalCard>
    )
    const card = screen.getByRole('alertdialog', { name: 'Crash' })
    expect(card.style.width).toBe('420px')
    expect(card).toHaveAttribute('aria-modal', 'false')
  })
})

describe('ModalTitle', () => {
  it('should draw the title and subtitle with no buttons when it has no handlers', () => {
    render(<ModalTitle title="Report" subtitle="Tell us more" />)
    expect(screen.getByRole('banner')).toHaveTextContent('ReportTell us more')
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
  })

  it('should run the close and back handlers from their named buttons', () => {
    const onClose = vi.fn()
    const onBack = vi.fn()
    render(<ModalTitle title="Report" onClose={onClose} onBack={onBack} closeLabel="Dismiss" backLabel="Previous" />)
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss' }))
    fireEvent.click(screen.getByRole('button', { name: 'Previous' }))
    expect(onClose).toHaveBeenCalledTimes(1)
    expect(onBack).toHaveBeenCalledTimes(1)
  })

  it('should hide its icon from assistive technology', () => {
    render(<ModalTitle title="Report" icon={<i data-testid="glyph" />} />)
    expect(screen.getByTestId('glyph').parentElement).toHaveAttribute('aria-hidden', 'true')
  })
})

describe('ModalActions', () => {
  it('should give each action a column of its own when equal, through a fragment too', () => {
    const { container } = render(
      <ModalActions equal>
        <>
          <button>Cancel</button>
          {null}
          <button>Confirm</button>
        </>
      </ModalActions>
    )
    const columns = [...container.firstElementChild!.children]
    expect(columns.map((column) => column.textContent)).toEqual(['Cancel', 'Confirm'])
    expect(columns.every((column) => column.tagName === 'DIV')).toBe(true)
  })

  it('should group the actions beside a lead note in a row, and not in a column', () => {
    const { container, rerender } = render(
      <ModalActions lead="Reviewed within a day.">
        <button>OK</button>
      </ModalActions>
    )
    const [lead, group] = [...container.firstElementChild!.children]
    expect(lead).toHaveTextContent('Reviewed within a day.')
    expect(group).toContainElement(screen.getByRole('button', { name: 'OK' }))
    rerender(
      <ModalActions lead="Reviewed within a day." direction="column">
        <button>OK</button>
      </ModalActions>
    )
    expect(screen.getByRole('button', { name: 'OK' }).parentElement).toBe(container.firstElementChild)
  })

  it('should justify its actions only for an alignment other than stretch', () => {
    const { container, rerender } = render(
      <ModalActions>
        <button>OK</button>
      </ModalActions>
    )
    expect((container.firstElementChild as HTMLElement).style.justifyContent).toBe('')
    rerender(
      <ModalActions align="between">
        <button>OK</button>
      </ModalActions>
    )
    expect((container.firstElementChild as HTMLElement).style.justifyContent).toBe('space-between')
  })
})

describe('ModalShell', () => {
  it('should lay out a header, a body and a footer in a card of its size', () => {
    const onClose = vi.fn()
    render(
      <ModalShell title="Leave?" ariaLabel="Leave?" size="md" onClose={onClose} actions={<button>Leave</button>}>
        You can join again.
      </ModalShell>
    )
    const card = screen.getByRole('dialog', { name: 'Leave?' })
    expect(card.style.width).toBe('540px')
    expect(card).toHaveTextContent('Leave?You can join again.Leave')
    fireEvent.click(screen.getByRole('button', { name: 'Close' }))
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('should prefer an explicit width, and fall back to the small size for an unknown one', () => {
    const { rerender } = render(
      <ModalShell ariaLabel="A" width={333} size="xl">
        body
      </ModalShell>
    )
    expect(screen.getByRole('dialog', { name: 'A' }).style.width).toBe('333px')
    rerender(
      <ModalShell ariaLabel="A" size={'huge' as 'sm'}>
        body
      </ModalShell>
    )
    expect(screen.getByRole('dialog', { name: 'A' }).style.width).toBe('420px')
  })

  it('should draw no header without a title, and no close button when it is turned off', () => {
    const { rerender } = render(
      <ModalShell ariaLabel="A" onClose={() => {}}>
        body
      </ModalShell>
    )
    expect(screen.queryByRole('banner')).not.toBeInTheDocument()
    rerender(
      <ModalShell ariaLabel="A" title="Title" onClose={() => {}} closeButton={false}>
        body
      </ModalShell>
    )
    expect(screen.getByRole('banner')).toHaveTextContent('Title')
    expect(screen.queryByRole('button', { name: 'Close' })).not.toBeInTheDocument()
  })

  it("should use the caller's header in place of the title, and its children with no body wrapper when bodyless", () => {
    render(
      <ModalShell ariaLabel="A" title="Ignored" header={<h2>Custom</h2>} bodyless>
        <p data-testid="body">body</p>
      </ModalShell>
    )
    const card = screen.getByRole('dialog', { name: 'A' })
    expect(screen.getByRole('heading', { name: 'Custom' })).toBeInTheDocument()
    expect(card).not.toHaveTextContent('Ignored')
    expect(screen.getByTestId('body').parentElement).toBe(card)
  })
})
