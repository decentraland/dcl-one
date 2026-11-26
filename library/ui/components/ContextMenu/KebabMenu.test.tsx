import '@testing-library/jest-dom/vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { KebabMenu } from './KebabMenu'

describe('KebabMenu', () => {
  it('is a button named by its label that opens its menu, and closes it on a second press', () => {
    const onClosed = vi.fn()
    render(<KebabMenu label="Options" items={() => [{ label: 'View profile' }]} onClosed={onClosed} />)
    const button = screen.getByRole('button', { name: 'Options' })
    expect(button).toHaveAttribute('aria-expanded', 'false')
    expect(screen.queryByRole('menu')).toBeNull()
    fireEvent.click(button)
    expect(button).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('menuitem', { name: 'View profile' })).toBeInTheDocument()
    fireEvent.click(button)
    expect(screen.queryByRole('menu')).toBeNull()
    expect(onClosed).toHaveBeenCalledTimes(1)
  })

  it('hands each entry a closer', () => {
    const run = vi.fn()
    const onClosed = vi.fn()
    render(
      <KebabMenu
        label="Options"
        onClosed={onClosed}
        items={(close) => [
          {
            label: 'Leave',
            onClick: () => {
              close()
              run()
            }
          }
        ]}
      />
    )
    fireEvent.click(screen.getByRole('button', { name: 'Options' }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'Leave' }))
    expect(run).toHaveBeenCalledTimes(1)
    expect(onClosed).toHaveBeenCalledTimes(1)
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('closes on a press outside and stays open on a press inside', () => {
    render(
      <>
        <KebabMenu label="Options" items={() => [{ label: 'View profile' }]} />
        <p>outside</p>
      </>
    )
    fireEvent.click(screen.getByRole('button', { name: 'Options' }))
    fireEvent.pointerDown(screen.getByRole('menuitem', { name: 'View profile' }))
    expect(screen.getByRole('menu')).toBeInTheDocument()
    fireEvent.pointerDown(screen.getByText('outside'))
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it("closes from the host's closer and runs its cleanup once", () => {
    const off = vi.fn()
    let closer: (() => void) | undefined
    const whileOpen = vi.fn((close: () => void) => {
      closer = close
      return off
    })
    const onClosed = vi.fn()
    render(<KebabMenu label="Options" items={() => [{ label: 'A' }]} whileOpen={whileOpen} onClosed={onClosed} />)
    expect(whileOpen).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Options' }))
    expect(whileOpen).toHaveBeenCalledTimes(1)
    act(() => closer?.())
    expect(screen.queryByRole('menu')).toBeNull()
    expect(off).toHaveBeenCalledTimes(1)
    expect(onClosed).toHaveBeenCalledTimes(1)
  })

  it('draws smaller dots in the small size', () => {
    const view = render(<KebabMenu label="Options" items={() => []} />)
    expect(view.container.querySelector('svg')).toHaveAttribute('width', '18')
    view.rerender(<KebabMenu label="Options" size="sm" items={() => []} />)
    expect(view.container.querySelector('svg')).toHaveAttribute('width', '16')
  })
})
