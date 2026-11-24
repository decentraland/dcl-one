import { describe, it, expect, vi } from 'vitest'
import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Sidebar } from '../../features/sidebar/Sidebar'
import { SkyboxMenu, formatHours } from '../../features/skybox/SkyboxMenu'
import { enterAsGuest, fakeSession, renderSession } from '../harness'

describe('skybox', () => {
  it('sidebar has Skybox after Scene permissions, before Volume, and it toggles the menu', async () => {
    const s = fakeSession()
    render(<Sidebar session={s} />)
    const labels = screen.getAllByRole('button').map((b) => b.getAttribute('aria-label'))
    expect(labels.slice(labels.indexOf('Scene permissions'), labels.indexOf('Volume') + 1)).toEqual(['Scene permissions', 'Skybox', 'Volume'])
    await userEvent.click(screen.getByRole('button', { name: 'Skybox' }))
    expect(vi.mocked(s.skybox.toggle)).toHaveBeenCalledTimes(1)
  })

  it('locks the slider while time progression is on', async () => {
    const skybox = { ...fakeSession().skybox, open: true, hours: 18.5 }
    render(<SkyboxMenu skybox={skybox} />)
    expect(screen.getByText('18:30')).toBeInTheDocument()
    expect(screen.getByRole('slider')).toBeDisabled()
    await userEvent.click(screen.getByRole('switch'))
    expect(skybox.setProgressing).toHaveBeenCalledWith(false)
  })

  it('turns the switch and the slider off where the scene sets the time of day', () => {
    const skybox = { ...fakeSession().skybox, open: true, progressing: false, sceneLocked: true }
    render(<SkyboxMenu skybox={skybox} />)
    expect(screen.getByRole('switch')).toBeDisabled()
    expect(screen.getByRole('slider')).toBeDisabled()
  })

  it('reads the clock again where a scene starts or stops setting the time of day', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    const none = { cameraLocked: false, avatarHidden: false, avatarMovementsBlocked: false, passportCannotBeOpened: false, experiencesBlocked: false, skyboxTimeUiBlocked: false, nearbyVoiceChatBlocked: false }
    const times = (): string[] => h.driver.commands.filter((command) => command.startsWith('/time'))
    // nothing changed: nothing read
    h.driver.emit({ kind: 'sceneRestrictions', state: none })
    expect(times()).toEqual([])

    // in the scene the engine reports the scene's hour
    h.driver.commandReply = () => 'time 22:0 -> 22:0, speed 0 (elapsed: 79200)'
    h.driver.emit({ kind: 'sceneRestrictions', state: { ...none, skyboxTimeUiBlocked: true } })
    await waitFor(() => expect(h.session().skybox.hours).toBe(22))
    expect(h.session().skybox.sceneLocked).toBe(true)

    // out of it, the player's own stopped clock: the menu must not keep the scene's hour
    h.driver.commandReply = () => 'time 6:0 -> 6:0, speed 0 (elapsed: 21600)'
    h.driver.emit({ kind: 'sceneRestrictions', state: none })
    await waitFor(() => expect(h.session().skybox.hours).toBe(6))
    expect(h.session().skybox.progressing).toBe(false)
    expect(times()).toEqual(['/time', '/time'])
  })

  it('reads the live clock on open, freezes it where it is, and drives /time off the chat path', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.commandReply = () => 'time 18:30 -> 18:30, speed 12 (elapsed: 66600)'
    act(() => h.session().skybox.toggle())
    await waitFor(() => expect(h.session().skybox.hours).toBe(18.5))
    expect(h.session().skybox.progressing).toBe(true)

    // The clock ran on since the menu opened: freezing re-reads it rather than using the slider.
    h.driver.commandReply = () => 'time 19:0 -> 19:0, speed 12 (elapsed: 68400)'
    act(() => h.session().skybox.setProgressing(false))
    await waitFor(() => expect(h.driver.commands).toContain('/time 19.00 0'))
    expect(h.session().skybox.hours).toBe(19)

    act(() => h.session().skybox.setHours(20))
    act(() => h.session().skybox.setProgressing(true))
    expect(h.driver.commands.filter(command => command.startsWith('/time'))).toEqual(['/time', '/time', '/time 19.00 0', '/time 20.00 0', '/time 20.00 12'])
    expect(h.driver.sentOf('consoleCommand')).toHaveLength(0)
  })

  it('closes on a click outside it, but not on itself or the rail', async () => {
    const skybox = { ...fakeSession().skybox, open: true }
    render(
      <>
        <nav aria-label="Main navigation"><button type="button">Skybox</button></nav>
        <SkyboxMenu skybox={skybox} />
        <div data-testid="world" />
      </>
    )
    await userEvent.click(screen.getByRole('switch'))
    await userEvent.click(screen.getByRole('button', { name: 'Skybox' }))
    expect(skybox.toggle).not.toHaveBeenCalled()
    await userEvent.click(screen.getByTestId('world'))
    expect(skybox.toggle).toHaveBeenCalledTimes(1)
  })

  it('follows the running clock while open, wrapping past midnight', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.commandReply = () => 'time 23:45 -> 23:45, speed 12 (elapsed: 85500)'
    act(() => h.session().skybox.toggle())
    await waitFor(() => expect(h.session().skybox.hours).toBe(23.75))

    h.driver.commandReply = () => 'time 0:15 -> 0:15, speed 12 (elapsed: 900)'
    await waitFor(() => expect(h.session().skybox.hours).toBe(0.25), { timeout: 2000 })

    // Frozen: the menu stops reading the clock.
    act(() => h.session().skybox.setProgressing(false))
    await waitFor(() => expect(h.driver.commands).toContain('/time 0.25 0'))
    const reads = h.driver.commands.filter(command => command.startsWith('/time')).length
    await new Promise((r) => setTimeout(r, 1200))
    expect(h.driver.commands.filter(command => command.startsWith('/time'))).toHaveLength(reads)
  })

  it('formats hours as HH:MM', () => {
    expect(formatHours(0)).toBe('00:00')
    expect(formatHours(9.75)).toBe('09:45')
    expect(formatHours(24)).toBe('00:00')
  })
})
