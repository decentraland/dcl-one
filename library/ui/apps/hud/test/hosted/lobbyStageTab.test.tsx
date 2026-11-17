import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { LobbyStageTab } from '../../features/settings/LobbyStageTab'

describe('Settings → Lobby', () => {
  it('lists every look, marks the chosen one and picks another', () => {
    const onPick = vi.fn()
    render(<LobbyStageTab preset="halloween-2026" onPick={onPick} />)
    const looks = screen.getAllByRole('radio')
    expect(looks.map((l) => l.getAttribute('title'))).toEqual(['Halloween 2026', 'Game World'])
    expect(screen.getByRole('radio', { name: 'Halloween 2026' })).toHaveAttribute('aria-checked', 'true')
    fireEvent.click(screen.getByRole('radio', { name: 'Game World' }))
    expect(onPick).toHaveBeenCalledWith('game-world')
  })

  it('is one tab stop, and the arrow keys pick the next look, wrapping around', () => {
    const onPick = vi.fn()
    render(<LobbyStageTab preset="halloween-2026" onPick={onPick} />)
    const chosen = screen.getByRole('radio', { name: 'Halloween 2026' })
    expect(chosen).toHaveAttribute('tabindex', '0')
    expect(screen.getByRole('radio', { name: 'Game World' })).toHaveAttribute('tabindex', '-1')
    chosen.focus()
    fireEvent.keyDown(chosen, { key: 'ArrowRight' })
    expect(onPick).toHaveBeenLastCalledWith('game-world')
    expect(screen.getByRole('radio', { name: 'Game World' })).toHaveFocus()
    fireEvent.keyDown(document.activeElement!, { key: 'ArrowDown' })
    expect(onPick).toHaveBeenLastCalledWith('halloween-2026')
  })

  it('shows the first look for an id it does not know', () => {
    render(<LobbyStageTab preset="retired-look" onPick={vi.fn()} />)
    expect(screen.getByRole('radio', { name: 'Halloween 2026' })).toHaveAttribute('aria-checked', 'true')
  })
})
