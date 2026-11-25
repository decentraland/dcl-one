import '@testing-library/jest-dom/vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { MenuDivider, MenuList, MenuPanel, MenuRow } from './MenuPanel'

describe('MenuPanel', () => {
  it('is a dialog named by its label, 274 wide, with no heading by default', () => {
    render(<MenuPanel label="System">content</MenuPanel>)
    const panel = screen.getByRole('dialog', { name: 'System' })
    expect(panel).toHaveAttribute('data-width', '274')
    expect(panel).toHaveTextContent('content')
    expect(screen.queryByRole('heading')).toBeNull()
  })

  it("draws its title as a heading, takes the wide size and keeps the caller's class and attributes", () => {
    render(
      <MenuPanel label="Shop" title="SHOP" width={400} className="mine" id="shop-panel">
        content
      </MenuPanel>
    )
    const panel = screen.getByRole('dialog', { name: 'Shop' })
    expect(screen.getByRole('heading', { level: 2, name: 'SHOP' })).toBeInTheDocument()
    expect(panel).toHaveAttribute('data-width', '400')
    expect(panel).toHaveClass('mine')
    expect(panel).toHaveAttribute('id', 'shop-panel')
  })
})

describe('MenuRow', () => {
  it('is a button named by its label and its trailing mark, not by its key', () => {
    const onClick = vi.fn()
    render(
      <MenuList>
        <MenuRow
          icon={<i data-testid="icon" />}
          label="Places"
          hint="X"
          trailing={<b>3</b>}
          onClick={onClick}
        />
      </MenuList>
    )
    const row = screen.getByRole('button', { name: /^Places/ })
    expect(row).toHaveAttribute('type', 'button')
    expect(row).not.toHaveAccessibleName(/X/)
    expect(screen.getByTestId('icon').parentElement).toHaveAttribute('aria-hidden', 'true')
    expect(screen.getByText('[X]')).toHaveAttribute('aria-hidden', 'true')
    fireEvent.click(row)
    expect(onClick).toHaveBeenCalledTimes(1)
  })

  it('draws no key for an empty hint', () => {
    render(<MenuRow label="Profile" hint="" />)
    expect(screen.getByRole('button', { name: 'Profile' }).children).toHaveLength(1)
  })

  it("keeps the caller's class, and does nothing while disabled", () => {
    const onClick = vi.fn()
    render(<MenuRow label="Exit" className="mine" disabled onClick={onClick} />)
    const row = screen.getByRole('button', { name: 'Exit' })
    expect(row).toHaveClass('mine')
    fireEvent.click(row)
    expect(onClick).not.toHaveBeenCalled()
  })
})

describe('MenuDivider', () => {
  it('is a separator', () => {
    render(<MenuDivider />)
    expect(screen.getByRole('separator')).toBeInTheDocument()
  })
})
