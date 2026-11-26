import '@testing-library/jest-dom/vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { HintsButton, KeyCap, type Hint } from './HintsButton'

const hints: Hint[] = [
  { icon: <KeyCap label="1" />, text: 'Press a number key to pick a slot.' },
  { icon: <KeyCap label="E" />, text: 'Press E to open emotes.' }
]

describe('HintsButton', () => {
  it('is a closed button named Controls, with a keyboard mark, until pressed', () => {
    const view = render(<HintsButton hints={hints} />)
    const button = screen.getByRole('button', { name: 'Controls' })
    expect(button).toHaveAttribute('aria-expanded', 'false')
    expect(button.querySelector('svg')).toHaveAttribute('aria-hidden', 'true')
    expect(screen.queryByRole('list')).toBeNull()
    expect(view.container.firstElementChild).not.toHaveClass('undefined')
  })

  it('opens a list of its hints, each with its mark, and closes it on a second press', () => {
    render(<HintsButton hints={hints} />)
    const button = screen.getByRole('button', { name: 'Controls' })
    fireEvent.click(button)
    expect(button).toHaveAttribute('aria-expanded', 'true')
    const rows = screen.getAllByRole('listitem')
    expect(rows.map((row) => row.textContent)).toEqual([
      '1Press a number key to pick a slot.',
      'EPress E to open emotes.'
    ])
    expect(screen.getByRole('list', { name: 'Controls' })).toBeInTheDocument()
    fireEvent.click(button)
    expect(screen.queryByRole('list')).toBeNull()
  })

  it('opens from the keyboard', async () => {
    render(<HintsButton hints={hints} />)
    await userEvent.tab()
    expect(screen.getByRole('button', { name: 'Controls' })).toHaveFocus()
    await userEvent.keyboard('{Enter}')
    expect(screen.getByRole('list')).toBeInTheDocument()
    await userEvent.keyboard(' ')
    expect(screen.queryByRole('list')).toBeNull()
  })

  it("takes the caller's name, mark and class", () => {
    const view = render(
      <HintsButton hints={hints} label="Emote hints" icon={<i data-testid="mark" />} className="mine" />
    )
    const button = screen.getByRole('button', { name: 'Emote hints' })
    expect(button).toContainElement(screen.getByTestId('mark'))
    expect(button.querySelector('svg')).toBeNull()
    expect(view.container.firstElementChild).toHaveClass('mine')
    fireEvent.click(button)
    expect(screen.getByRole('list', { name: 'Emote hints' })).toBeInTheDocument()
  })

  it('opens an empty list for no hints', () => {
    render(<HintsButton hints={[]} />)
    fireEvent.click(screen.getByRole('button', { name: 'Controls' }))
    expect(screen.getByRole('list')).toBeEmptyDOMElement()
  })

  it('closes on a press outside, and not on a press on the list', () => {
    render(
      <>
        <HintsButton hints={hints} />
        <p>outside</p>
      </>
    )
    fireEvent.click(screen.getByRole('button', { name: 'Controls' }))
    fireEvent.mouseDown(screen.getByText('Press E to open emotes.'))
    expect(screen.getByRole('list')).toBeInTheDocument()
    fireEvent.mouseDown(screen.getByText('outside'))
    expect(screen.queryByRole('list')).toBeNull()
  })

  it('does not close on Escape by itself', () => {
    render(<HintsButton hints={hints} />)
    fireEvent.click(screen.getByRole('button', { name: 'Controls' }))
    fireEvent.keyDown(document.body, { key: 'Escape' })
    expect(screen.getByRole('list')).toBeInTheDocument()
  })

  it("hands the host a way to close the list while it is open, and runs the host's cleanup as it leaves", () => {
    const off = vi.fn()
    let close: (() => void) | undefined
    const whileOpen = vi.fn((closer: () => void) => {
      close = closer
      return off
    })
    render(<HintsButton hints={hints} whileOpen={whileOpen} />)
    expect(whileOpen).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Controls' }))
    expect(whileOpen).toHaveBeenCalledTimes(1)
    act(() => close?.())
    expect(screen.queryByRole('list')).toBeNull()
    expect(off).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByRole('button', { name: 'Controls' }))
    expect(whileOpen).toHaveBeenCalledTimes(2)
  })

  it("runs the host's cleanup when it unmounts open, and none when it unmounts closed", () => {
    const off = vi.fn()
    const closed = render(<HintsButton hints={hints} whileOpen={() => off} />)
    closed.unmount()
    expect(off).not.toHaveBeenCalled()
    const opened = render(<HintsButton hints={hints} whileOpen={() => off} />)
    fireEvent.click(screen.getByRole('button', { name: 'Controls' }))
    opened.unmount()
    expect(off).toHaveBeenCalledTimes(1)
  })
})

describe('KeyCap', () => {
  it('shows its label, a key name or nothing', () => {
    const view = render(<KeyCap label="Esc" />)
    expect(screen.getByText('Esc').tagName).toBe('SPAN')
    view.rerender(<KeyCap label="" />)
    expect(view.container.firstElementChild).toBeEmptyDOMElement()
  })
})
