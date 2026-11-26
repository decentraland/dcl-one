import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { Dropdown } from './Dropdown'

const options = ['Recently listed', 'Cheapest', 'Newest']
const field = () => screen.getByRole('button')

describe("when a Dropdown is the hud's sort list", () => {
  it('should start on the first option when uncontrolled, and on the default when given one', () => {
    const { rerender } = render(<Dropdown appearance="hud" kind="sort" options={options} />)
    expect(field()).toHaveTextContent('Recently listed')
    rerender(<Dropdown key="other" appearance="hud" kind="sort" options={options} defaultValue="Newest" />)
    expect(field()).toHaveTextContent('Newest')
  })

  it('should open on a click, mark the chosen option and keep the choice when uncontrolled', async () => {
    const onChange = vi.fn()
    render(<Dropdown appearance="hud" kind="sort" options={options} onChange={onChange} />)
    await userEvent.click(field())
    expect(field()).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('option', { name: 'Recently listed' })).toHaveAttribute('aria-selected', 'true')
    await userEvent.click(screen.getByRole('option', { name: 'Cheapest' }))
    expect(onChange).toHaveBeenCalledWith('Cheapest')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
    expect(field()).toHaveTextContent('Cheapest')
    expect(field()).toHaveFocus()
  })

  it('should follow a controlled value and not keep a choice of its own', async () => {
    const onChange = vi.fn()
    render(<Dropdown appearance="hud" kind="sort" options={options} value="Cheapest" onChange={onChange} />)
    await userEvent.click(field())
    await userEvent.click(screen.getByRole('option', { name: 'Newest' }))
    expect(onChange).toHaveBeenCalledWith('Newest')
    expect(field()).toHaveTextContent('Cheapest')
  })

  it('should move through the options with the keyboard and choose with Enter', async () => {
    const onChange = vi.fn()
    render(<Dropdown appearance="hud" kind="sort" options={options} onChange={onChange} />)
    field().focus()
    await userEvent.keyboard('{ArrowDown}')
    expect(screen.getByRole('listbox')).toBeInTheDocument()
    await userEvent.keyboard('{ArrowDown}{ArrowDown}{ArrowDown}')
    expect(field()).toHaveAttribute('aria-activedescendant', screen.getByRole('option', { name: 'Newest' }).id)
    await userEvent.keyboard('{Home}')
    expect(field()).toHaveAttribute('aria-activedescendant', screen.getByRole('option', { name: 'Recently listed' }).id)
    await userEvent.keyboard('{End}{ArrowUp}{Enter}')
    expect(onChange).toHaveBeenCalledWith('Cheapest')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

  it('should close on a press outside', async () => {
    render(
      <>
        <Dropdown appearance="hud" kind="sort" options={options} />
        <p>Outside</p>
      </>
    )
    await userEvent.click(field())
    fireEvent.mouseDown(screen.getByText('Outside'))
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

  it('should hand a closer to whileOpen as the list opens, and run its cleanup as it closes', async () => {
    const cleanup = vi.fn()
    let close: (() => void) | undefined
    const whileOpen = vi.fn((closer: () => void) => {
      close = closer
      return cleanup
    })
    render(<Dropdown appearance="hud" kind="sort" options={options} whileOpen={whileOpen} />)
    expect(whileOpen).not.toHaveBeenCalled()
    await userEvent.click(field())
    // The list registers in an effect, which can run after the click settles.
    await waitFor(() => expect(whileOpen).toHaveBeenCalledTimes(1))
    expect(cleanup).not.toHaveBeenCalled()
    fireEvent.click(document.body)
    close?.()
    await screen.findByRole('button', { expanded: false })
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
    await waitFor(() => expect(cleanup).toHaveBeenCalledTimes(1))
  })

  it('should open an empty list with nothing to choose', async () => {
    const onChange = vi.fn()
    render(<Dropdown appearance="hud" kind="sort" options={[]} onChange={onChange} />)
    field().focus()
    await userEvent.keyboard('{Enter}{Enter}')
    expect(onChange).not.toHaveBeenCalled()
  })
})
