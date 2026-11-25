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

    // Frozen: the menu stops reading the clock.
    act(() => h.session().skybox.setProgressing(false))
    await waitFor(() => expect(h.driver.commands).toContain('/time 0.25 0'))
    const reads = h.driver.commands.filter(command => command.startsWith('/time')).length
    await new Promise((r) => setTimeout(r, 1200))
    expect(h.driver.commands.filter(command => command.startsWith('/time'))).toHaveLength(reads)
  })

  it('is off, with the reason, where the scene holds the time of day', () => {
    const skybox = { ...fakeSession().skybox, open: true, progressing: false, sceneLocked: true }
    render(<SkyboxMenu skybox={skybox} />)
    expect(screen.getByRole('switch')).toBeDisabled()
    expect(screen.getByRole('slider')).toBeDisabled()
    const note = screen.getByRole('note')
    expect(note).toHaveTextContent('Allow overriding scene controls')
    expect(screen.getByRole('switch')).toHaveAttribute('aria-describedby', note.id)
    expect(screen.getByRole('slider')).toHaveAttribute('aria-describedby', note.id)
  })

  it('says nothing about scenes where the player holds the time of day', () => {
    render(<SkyboxMenu skybox={{ ...fakeSession().skybox, open: true }} />)
    expect(screen.queryByRole('note')).toBeNull()
  })

  it('takes the lock, the hour and the clock from the engine as they change, without asking', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    // the day cycle runs and the player walks into a scene that sets the time
    h.driver.emit({ kind: 'skyTime', state: { locked: false, speed: 12, fixedHours: null } })
    expect(h.session().skybox.sceneLocked).toBe(false)
    h.driver.emit({ kind: 'skyTime', state: { locked: true, speed: 12, fixedHours: 22 } })
    expect(h.session().skybox.sceneLocked).toBe(true)
    expect(h.session().skybox.hours).toBe(22)
    expect(h.session().skybox.progressing).toBe(true)

    // locked and open, the menu does not read the clock
    h.driver.commandReply = () => 'time 22:0 -> 22:0, speed 12 (elapsed: 79200)'
    act(() => h.session().skybox.toggle())
    const reads = (): number => h.driver.commands.filter((command) => command.startsWith('/time')).length
    await waitFor(() => expect(reads()).toBe(1))
    await new Promise((r) => setTimeout(r, 1200))
    expect(reads()).toBe(1)

    // the player walks out with the clock stopped at six
    h.driver.emit({ kind: 'skyTime', state: { locked: false, speed: 0, fixedHours: 6 } })
    expect(h.session().skybox.sceneLocked).toBe(false)
    expect(h.session().skybox.hours).toBe(6)
    expect(h.session().skybox.progressing).toBe(false)
  })

  it("keeps the slider where the player drags it when the stopped clock's own reports come back", async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit({ kind: 'skyTime', state: { locked: false, speed: 0, fixedHours: 6 } })
    expect(h.session().skybox.hours).toBe(6)
    act(() => h.session().skybox.setHours(8))
    act(() => h.session().skybox.setHours(9))
    // the engine reports the first value after the slider has moved on
    h.driver.emit({ kind: 'skyTime', state: { locked: false, speed: 0, fixedHours: 8 } })
    expect(h.session().skybox.hours).toBe(9)
    h.driver.emit({ kind: 'skyTime', state: { locked: false, speed: 0, fixedHours: 9 } })
    expect(h.session().skybox.hours).toBe(9)
  })

  it('shows an hour it did not send, with the clock stopped and nothing locked', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit({ kind: 'skyTime', state: { locked: false, speed: 0, fixedHours: 10 } })
    act(() => h.session().skybox.setHours(10))
    h.driver.emit({ kind: 'skyTime', state: { locked: false, speed: 0, fixedHours: 10 } })
    // overriding is allowed and the player walks into a scene at 22:00, or types `/time 22 0`
    h.driver.emit({ kind: 'skyTime', state: { locked: false, speed: 0, fixedHours: 22 } })
    expect(h.session().skybox.hours).toBe(22)
  })

  it('forgets the hours it sent once a scene holds the sky, and matches them across midnight', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit({ kind: 'skyTime', state: { locked: false, speed: 0, fixedHours: 6 } })
    // sent as the scene takes the sky: the engine never reports this hour back
    act(() => h.session().skybox.setHours(8))
    h.driver.emit({ kind: 'skyTime', state: { locked: true, speed: 0, fixedHours: 22 } })
    expect(h.session().skybox.hours).toBe(22)
    // out of the scene, the stopped clock really is at eight
    h.driver.emit({ kind: 'skyTime', state: { locked: false, speed: 0, fixedHours: 8 } })
    expect(h.session().skybox.hours).toBe(8)

    // 24 is midnight: its report is the slider's own, and leaves nothing behind
    act(() => h.session().skybox.setHours(24))
    h.driver.emit({ kind: 'skyTime', state: { locked: false, speed: 0, fixedHours: 0 } })
    expect(h.session().skybox.hours).toBe(24)
    h.driver.emit({ kind: 'skyTime', state: { locked: false, speed: 0, fixedHours: 0 } })
    expect(h.session().skybox.hours).toBe(0)
  })

  it('does not wait for a report of the hour the engine already has', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit({ kind: 'skyTime', state: { locked: false, speed: 0, fixedHours: 10 } })
    // the same hour again: the engine sends nothing back
    act(() => h.session().skybox.setHours(10))
    h.driver.emit({ kind: 'skyTime', state: { locked: false, speed: 0, fixedHours: 22 } })
    expect(h.session().skybox.hours).toBe(22)
    // a later ten is news, not that hour's report
    h.driver.emit({ kind: 'skyTime', state: { locked: false, speed: 0, fixedHours: 10 } })
    expect(h.session().skybox.hours).toBe(10)
  })

  it('does not let the read on open overwrite an hour the engine pushed', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit({ kind: 'skyTime', state: { locked: true, speed: 12, fixedHours: 22 } })
    h.driver.commandReply = () => 'time 10:0 -> 10:0, speed 12 (elapsed: 36000)'
    act(() => h.session().skybox.toggle())
    await waitFor(() => expect(h.driver.commands).toContain('/time'))
    await new Promise((r) => setTimeout(r, 50))
    expect(h.session().skybox.hours).toBe(22)
  })

  it('formats hours as HH:MM', () => {
    expect(formatHours(0)).toBe('00:00')
    expect(formatHours(9.75)).toBe('09:45')
    expect(formatHours(24)).toBe('00:00')
  })
})
