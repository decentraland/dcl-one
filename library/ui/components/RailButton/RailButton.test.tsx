import '@testing-library/jest-dom/vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { RailButton } from './RailButton'

const icon = <i data-testid="icon" />

describe('RailButton', () => {
  it('is a toggle button named by its label, unpressed, with its icon and nothing else', () => {
    render(<RailButton icon={icon} label="Friends" />)
    const button = screen.getByRole('button', { name: 'Friends' })
    expect(button).toHaveAttribute('type', 'button')
    expect(button).toHaveAttribute('aria-pressed', 'false')
    expect(button).not.toHaveAttribute('title')
    expect(button).toContainElement(screen.getByTestId('icon'))
    expect(button.children).toHaveLength(1)
    expect(button.className).not.toMatch(/undefined|false|\s{2}/)
  })

  it('is pressed while active', () => {
    render(<RailButton icon={icon} label="Chat" active />)
    expect(screen.getByRole('button', { name: 'Chat' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('tells its tip the label and the shortcut, and a status in place of the label', () => {
    const view = render(<RailButton icon={icon} label="Friends" shortcut="L" />)
    expect(screen.getByRole('tooltip', { hidden: true })).toHaveTextContent('Friends')
    expect(screen.getByRole('tooltip', { hidden: true })).toHaveTextContent('L')
    view.rerender(<RailButton icon={icon} label="Voice" tooltip="3 people nearby" />)
    expect(screen.getByRole('tooltip', { hidden: true })).toHaveTextContent('3 people nearby')
    expect(screen.getByRole('button', { name: 'Voice' })).toBeInTheDocument()
  })

  it('draws no tip while quiet', () => {
    render(<RailButton icon={icon} label="Map" quiet />)
    expect(screen.queryByRole('tooltip', { hidden: true })).toBeNull()
    expect(screen.getByRole('button', { name: 'Map' })).toBeInTheDocument()
  })

  it('shows a count, and none at zero, below zero or without one', () => {
    const view = render(<RailButton icon={icon} label="Notifications" badge={3} />)
    const button = () => screen.getByRole('button', { name: 'Notifications' })
    expect(button()).toHaveTextContent('3')
    for (const badge of [0, -2, undefined]) {
      view.rerender(<RailButton icon={icon} label="Notifications" badge={badge} />)
      expect(button()).toHaveTextContent('')
      expect(button().children).toHaveLength(1)
    }
  })

  it('caps the count at 99+, or at the given maximum as +max', () => {
    const view = render(<RailButton icon={icon} label="Notifications" badge={99} />)
    const button = () => screen.getByRole('button', { name: 'Notifications' })
    expect(button()).toHaveTextContent('99')
    view.rerender(<RailButton icon={icon} label="Notifications" badge={100} />)
    expect(button()).toHaveTextContent('99+')
    view.rerender(<RailButton icon={icon} label="Notifications" badge={9} badgeMax={9} />)
    expect(button()).toHaveTextContent('9')
    view.rerender(<RailButton icon={icon} label="Notifications" badge={10} badgeMax={9} />)
    expect(button()).toHaveTextContent('+9')
  })

  it('gives the lavender count a class of its own', () => {
    const view = render(<RailButton icon={icon} label="Hangouts" badge={2} />)
    const ruby = screen.getByText('2').className
    view.rerender(<RailButton icon={icon} label="Hangouts" badge={2} badgeTone="lavender" />)
    expect(screen.getByText('2').className).not.toBe(ruby)
    expect(screen.getByText('2').className).toContain(ruby)
  })

  it('draws the status dot and the corner picture, the picture announced by its label', () => {
    const view = render(<RailButton icon={icon} label="Voice" indicator />)
    expect(view.container.querySelector('[data-indicator]')).not.toBeNull()
    expect(screen.queryByRole('img')).toBeNull()
    view.rerender(
      <RailButton icon={icon} label="Communities" cornerArt={{ src: 'live.png', label: 'A stream is live' }} />
    )
    expect(view.container.querySelector('[data-indicator]')).toBeNull()
    const art = screen.getByRole('img', { name: 'A stream is live' })
    expect(art).toHaveAttribute('src', 'live.png')
    expect(art).toHaveAttribute('draggable', 'false')
  })

  it('draws its children after the icon, and before the dot and the count', () => {
    render(
      <RailButton icon={icon} label="Voice" indicator badge={1}>
        <b data-testid="extra" />
      </RailButton>
    )
    const order = [...screen.getByRole('button', { name: 'Voice' }).children].map(
      (child) => child.getAttribute('data-testid') ?? (child.hasAttribute('data-indicator') ? 'dot' : 'count')
    )
    expect(order).toEqual(['icon', 'extra', 'dot', 'count'])
  })

  it("marks a portrait, and adds the caller's class", () => {
    const view = render(<RailButton icon={icon} label="Me" />)
    const plain = screen.getByRole('button', { name: 'Me' }).className
    view.rerender(<RailButton icon={icon} label="Me" portrait className="mine" />)
    const button = screen.getByRole('button', { name: 'Me' })
    expect(button).toHaveClass('mine')
    expect(button.className.split(' ')).toHaveLength(plain.split(' ').length + 2)
  })

  it('reports a press from the pointer and from the keyboard, and passes button attributes through', async () => {
    const onClick = vi.fn()
    render(<RailButton icon={icon} label="Friends" onClick={onClick} data-rail="friends" aria-expanded />)
    const button = screen.getByRole('button', { name: 'Friends' })
    expect(button).toHaveAttribute('data-rail', 'friends')
    expect(button).toHaveAttribute('aria-expanded', 'true')
    fireEvent.click(button)
    await userEvent.tab()
    expect(button).toHaveFocus()
    await userEvent.keyboard('{Enter}')
    await userEvent.keyboard(' ')
    expect(onClick).toHaveBeenCalledTimes(3)
  })

  it('reports no press while disabled', async () => {
    const onClick = vi.fn()
    render(<RailButton icon={icon} label="Shop" onClick={onClick} disabled />)
    const button = screen.getByRole('button', { name: 'Shop' })
    expect(button).toBeDisabled()
    await userEvent.click(button)
    expect(onClick).not.toHaveBeenCalled()
  })
})
