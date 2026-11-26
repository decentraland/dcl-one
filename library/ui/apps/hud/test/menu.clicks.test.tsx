import { afterEach, beforeEach, describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MainMenuShell } from '../features/menu/MainMenuShell'
import { ProfileChip } from '../features/menu/ProfileChip'
import shellStyles from '../features/menu/MainMenuShell.module.css'

describe('main menu shell nav', () => {
  function shell() {
    const onNavigate = vi.fn()
    const onClose = vi.fn()
    render(
      <MainMenuShell active="settings" onNavigate={onNavigate} onClose={onClose}>
        <div>body</div>
      </MainMenuShell>
    )
    return { onNavigate, onClose }
  }

  it('clicking a non-active page navigates to it', async () => {
    const { onNavigate } = shell()
    await userEvent.click(screen.getByRole('button', { name: /Places/ }))
    expect(onNavigate).toHaveBeenCalledWith('places')
    await userEvent.click(screen.getByRole('button', { name: /Map/ }))
    expect(onNavigate).toHaveBeenCalledWith('map')
    await userEvent.click(screen.getByRole('button', { name: /Backpack/ }))
    expect(onNavigate).toHaveBeenCalledWith('backpack')
  })

  it('clicking the already-active page is a no-op', async () => {
    const { onNavigate } = shell()
    await userEvent.click(screen.getByRole('button', { name: /Settings/ }))
    expect(onNavigate).not.toHaveBeenCalled()
  })

  it('close button fires onClose', async () => {
    const { onClose } = shell()
    await userEvent.click(screen.getByRole('button', { name: 'Close' }))
    expect(onClose).toHaveBeenCalledTimes(1)
  })
})

describe('main menu shell entrance', () => {
  // Each test starts well after the previous one closed its shells.
  let clock = Date.now()
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ['Date'] })
    clock += 3_600_000
    vi.setSystemTime(clock)
  })
  afterEach(() => {
    vi.useRealTimers()
  })
  const darkFrames = (container: HTMLElement): number => container.getElementsByClassName(shellStyles.darkFrame).length
  const page = (active: string): React.JSX.Element => (
    <MainMenuShell key={active} active={active} onNavigate={vi.fn()} onClose={vi.fn()}>
      <div>{active}</div>
    </MainMenuShell>
  )

  it('a fresh open fades through a dark frame', () => {
    const { container } = render(
      <MainMenuShell active="map" onNavigate={vi.fn()} onClose={vi.fn()}>
        <div>body</div>
      </MainMenuShell>
    )
    expect(darkFrames(container)).toBe(1)
  })

  it('the lobby card opens without a dark frame', () => {
    const { container } = render(
      <MainMenuShell active="map" presentation="lobbyCard" onNavigate={vi.fn()} onClose={vi.fn()}>
        <div>body</div>
      </MainMenuShell>
    )
    expect(darkFrames(container)).toBe(0)
  })

  it('a page switch keeps the open view without a dark frame', () => {
    const { container, rerender } = render(page('map'))
    rerender(page('backpack'))
    expect(screen.getByText('backpack')).toBeTruthy()
    expect(darkFrames(container)).toBe(0)
  })

  it('a page that mounts just after the last one closed is a page switch', () => {
    const { container, rerender } = render(page('map'))
    rerender(<></>)
    rerender(page('create'))
    expect(darkFrames(container)).toBe(0)
  })

  it('a shell that opens a while after the last one closed fades through a dark frame', () => {
    const { container, rerender } = render(page('map'))
    rerender(<></>)
    vi.setSystemTime(Date.now() + 1000)
    rerender(page('backpack'))
    expect(darkFrames(container)).toBe(1)
  })
})

describe('profile chip dropdown', () => {
  function chip() {
    const onViewProfile = vi.fn()
    const onSignOut = vi.fn()
    const onExit = vi.fn()
    render(<ProfileChip name="Tester" address="0xabcdef123456" onViewProfile={onViewProfile} onSignOut={onSignOut} onExit={onExit} />)
    return { onViewProfile, onSignOut, onExit }
  }

  it('opens and routes View Profile / Sign Out / Exit', async () => {
    const { onViewProfile, onSignOut, onExit } = chip()
    await userEvent.click(screen.getByRole('button')) // the only button before open = the chip
    await userEvent.click(screen.getByRole('button', { name: /VIEW PROFILE/i }))
    expect(onViewProfile).toHaveBeenCalledTimes(1)

    await userEvent.click(screen.getByRole('button')) // reopen
    await userEvent.click(screen.getByRole('button', { name: /SIGN OUT/i }))
    expect(onSignOut).toHaveBeenCalledTimes(1)

    await userEvent.click(screen.getByRole('button', { name: /EXIT/i }))
    expect(onExit).toHaveBeenCalledTimes(1)
  })

  it('offers no Sign Out without a handler', async () => {
    render(<ProfileChip name="Tester" address="0xabcdef123456" onViewProfile={vi.fn()} onExit={vi.fn()} />)
    await userEvent.click(screen.getByRole('button'))
    expect(screen.queryByRole('button', { name: /SIGN OUT/i })).toBeNull()
  })

  it('copies the wallet address', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.assign(navigator, { clipboard: { writeText } })
    chip()
    await userEvent.click(screen.getByRole('button'))
    await userEvent.click(screen.getByTitle('Copy address'))
    expect(writeText).toHaveBeenCalledWith('0xabcdef123456')
  })
})
