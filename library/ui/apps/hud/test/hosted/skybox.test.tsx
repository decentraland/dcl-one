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

    // Frozen: the menu still reads `/time` for the lock, and the slider stops following it.
    act(() => h.session().skybox.setProgressing(false))
    await waitFor(() => expect(h.driver.commands).toContain('/time 0.25 0'))
    h.driver.commandReply = () => 'time 5:0 -> 5:0, speed 0 (elapsed: 18000), locked: false'
    await new Promise((r) => setTimeout(r, 1200))
    expect(h.session().skybox.hours).toBe(0.25)
  })

  it('is off, with the reason, where the scene holds the time of day', () => {
    const skybox = { ...fakeSession().skybox, open: true, progressing: false, sceneLocked: true }
    render(<SkyboxMenu skybox={skybox} />)
    expect(screen.getByRole('switch')).toBeDisabled()
    expect(screen.getByRole('slider')).toBeDisabled()
    expect(screen.getByRole('note')).toHaveTextContent('Allow overriding scene controls')
  })

  it('says nothing about scenes where the player holds the time of day', () => {
    render(<SkyboxMenu skybox={{ ...fakeSession().skybox, open: true }} />)
    expect(screen.queryByRole('note')).toBeNull()
  })

  it('takes the lock from /time, and reads the stopped clock again when the lock ends', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.commandReply = () => 'time 22:0 -> 22:0, speed 0 (elapsed: 79200), locked: true'
    act(() => h.session().skybox.toggle())
    await waitFor(() => expect(h.session().skybox.sceneLocked).toBe(true))
    expect(h.session().skybox.hours).toBe(22)

    // The player walks out: the reply is the stopped clock's time again.
    h.driver.commandReply = () => 'time 6:0 -> 6:0, speed 0 (elapsed: 21600), locked: false'
    await waitFor(() => expect(h.session().skybox.sceneLocked).toBe(false), { timeout: 2000 })
    expect(h.session().skybox.hours).toBe(6)
    expect(h.session().skybox.progressing).toBe(false)
  })

  it('formats hours as HH:MM', () => {
    expect(formatHours(0)).toBe('00:00')
    expect(formatHours(9.75)).toBe('09:45')
    expect(formatHours(24)).toBe('00:00')
  })
})
