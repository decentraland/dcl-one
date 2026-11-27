import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { IconButton } from '../design'

// The HUD's IconButton is the shared RailButton (library/ui/components/RailButton, tested there) holding
// one of the HUD's icons: a glyph by name, a piece of art, or the player's picture.
describe('IconButton', () => {
  it('draws the named glyph at 24px in a pressable button named by its label', () => {
    const onClick = vi.fn()
    render(<IconButton icon="friends" label="Friends" shortcut="L" badge={3} onClick={onClick} data-rail="friends" />)
    const button = screen.getByRole('button', { name: 'Friends' })
    expect(button.getAttribute('aria-pressed')).toBe('false')
    expect(button.getAttribute('data-rail')).toBe('friends')
    expect((button.firstElementChild as HTMLElement).style.width).toBe('24px')
    expect(button.textContent).toBe('3')
    fireEvent.click(button)
    expect(onClick).toHaveBeenCalledTimes(1)
  })

  it('draws white art as a mask in place of the glyph, and colour art as a picture', () => {
    const view = render(<IconButton icon="map" art={{ src: 'map.png', size: 26 }} label="Map" active quiet />)
    const button = () => screen.getByRole('button', { name: 'Map' })
    expect(button().getAttribute('aria-pressed')).toBe('true')
    expect(button().querySelector('img')).toBeNull()
    expect(button().firstElementChild?.getAttribute('style')).toContain('map.png')
    expect((button().firstElementChild as HTMLElement).style.width).toBe('26px')
    view.rerender(<IconButton icon="map" art={{ src: 'map.png', size: 26, color: true }} label="Map" />)
    const art = button().querySelector('img')
    expect(art?.getAttribute('src')).toBe('map.png')
    expect(art?.getAttribute('width')).toBe('26')
    expect(art?.getAttribute('alt')).toBe('')
  })

  it("draws the player's picture in place of the glyph, with the initial while there is no picture", () => {
    const view = render(<IconButton icon="profile" label="Me" />)
    const plain = screen.getByRole('button', { name: 'Me' }).className
    view.rerender(<IconButton icon="profile" label="Me" avatar={{ name: 'Mojito', color: '#c640cd' }} />)
    const button = screen.getByRole('button', { name: 'Me' })
    expect(button.textContent).toBe('M')
    expect(button.className).not.toBe(plain)
  })

  it('draws children after its icon, and reports no press while disabled', () => {
    const onClick = vi.fn()
    render(
      <IconButton icon="chat" label="Chat" onClick={onClick} disabled>
        <b data-testid="extra" />
      </IconButton>
    )
    const button = screen.getByRole('button', { name: 'Chat' })
    expect(button.children[1]).toBe(screen.getByTestId('extra'))
    fireEvent.click(button)
    expect(onClick).not.toHaveBeenCalled()
  })
})
