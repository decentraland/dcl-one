import { describe, it, expect, vi } from 'vitest'
import { act, render, screen } from '@testing-library/react'
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

  it('watches the time of day while open, freezes the clock at the hour shown, and drives /time off the chat path', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    expect(h.driver.sentOf('watchSkyTime')).toHaveLength(0)
    act(() => h.session().skybox.toggle())
    expect(h.driver.sentOf('watchSkyTime')).toEqual([{ kind: 'watchSkyTime', on: true }])
    h.driver.emit({ kind: 'skyTime', state: { hours: 18.5, locked: false, speed: 12, followsClock: true } })
    expect(h.session().skybox.hours).toBe(18.5)
    expect(h.session().skybox.progressing).toBe(true)

    // the clock ran on since the menu opened
    h.driver.emit({ kind: 'skyTime', state: { hours: 19, locked: false, speed: 12, followsClock: true } })
    act(() => h.session().skybox.setProgressing(false))
    expect(h.session().skybox.hours).toBe(19)
    act(() => h.session().skybox.setHours(20))
    act(() => h.session().skybox.setProgressing(true))
    expect(h.driver.commands.filter(command => command.startsWith('/time'))).toEqual(['/time 19.00 0', '/time 20.00 0', '/time 20.00 12'])
    expect(h.driver.sentOf('consoleCommand')).toHaveLength(0)

    act(() => h.session().skybox.toggle())
    expect(h.driver.sentOf('watchSkyTime')).toEqual([{ kind: 'watchSkyTime', on: true }, { kind: 'watchSkyTime', on: false }])
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

  it('takes the lock, the hour and the clock from the engine as they change, without reading /time', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().skybox.toggle())
    // the day cycle runs and the player walks into a scene that sets the time
    h.driver.emit({ kind: 'skyTime', state: { hours: 23.75, locked: false, speed: 12, followsClock: true } })
    expect(h.session().skybox.sceneLocked).toBe(false)
    expect(h.session().skybox.hours).toBe(23.75)
    h.driver.emit({ kind: 'skyTime', state: { hours: 0.25, locked: false, speed: 12, followsClock: true } })
    expect(h.session().skybox.hours).toBe(0.25)
    h.driver.emit({ kind: 'skyTime', state: { hours: 22, locked: true, speed: 12, followsClock: false } })
    expect(h.session().skybox.sceneLocked).toBe(true)
    expect(h.session().skybox.hours).toBe(22)
    expect(h.session().skybox.progressing).toBe(true)

    // the player walks out with the clock stopped at six
    h.driver.emit({ kind: 'skyTime', state: { hours: 6, locked: false, speed: 0, followsClock: true } })
    expect(h.session().skybox.sceneLocked).toBe(false)
    expect(h.session().skybox.progressing).toBe(false)
    // the sky skips there, and the slider with it
    expect(h.session().skybox.hours).toBe(6)
    h.driver.emit({ kind: 'skyTime', state: { hours: 6.5, locked: false, speed: 0, followsClock: true } })
    expect(h.session().skybox.hours).toBe(6.5)
    expect(h.driver.commands.filter((command) => command.startsWith('/time'))).toHaveLength(0)
  })

  it('follows a stopped clock until the player drags the slider, then keeps the hour dragged to', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().skybox.toggle())
    h.driver.emit({ kind: 'skyTime', state: { hours: 6, locked: false, speed: 0, followsClock: true } })
    expect(h.session().skybox.hours).toBe(6)
    act(() => h.session().skybox.setHours(9))
    await new Promise((r) => setTimeout(r, 350))
    // the sky is still on its way to nine
    h.driver.emit({ kind: 'skyTime', state: { hours: 7.5, locked: false, speed: 0, followsClock: true } })
    expect(h.session().skybox.hours).toBe(9)

    // a scene's time takes the sky, and after it the stopped clock is followed again
    h.driver.emit({ kind: 'skyTime', state: { hours: 22, locked: false, speed: 0, followsClock: false } })
    expect(h.session().skybox.hours).toBe(22)
    h.driver.emit({ kind: 'skyTime', state: { hours: 9, locked: false, speed: 0, followsClock: true } })
    expect(h.session().skybox.hours).toBe(9)

    // reopened after another drag, it shows where the sky is
    act(() => h.session().skybox.setHours(12))
    await new Promise((r) => setTimeout(r, 350))
    act(() => h.session().skybox.toggle())
    act(() => h.session().skybox.toggle())
    h.driver.emit({ kind: 'skyTime', state: { hours: 11, locked: false, speed: 0, followsClock: true } })
    expect(h.session().skybox.hours).toBe(11)
  })

  it("shows a scene's hour over a stopped clock where overriding is allowed", async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().skybox.toggle())
    h.driver.emit({ kind: 'skyTime', state: { hours: 10, locked: false, speed: 0, followsClock: true } })
    // the player walks into a scene at 22:00: its time takes the sky and the menu stays usable
    h.driver.emit({ kind: 'skyTime', state: { hours: 22, locked: false, speed: 0, followsClock: false } })
    expect(h.session().skybox.hours).toBe(22)
    expect(h.session().skybox.sceneLocked).toBe(false)
  })

  it('drops a report read before the engine applied what the menu just sent', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().skybox.toggle())
    h.driver.emit({ kind: 'skyTime', state: { hours: 10, locked: false, speed: 12, followsClock: true } })
    act(() => h.session().skybox.setProgressing(false))
    // the clock as it was a moment before
    h.driver.emit({ kind: 'skyTime', state: { hours: 10, locked: false, speed: 12, followsClock: true } })
    expect(h.session().skybox.progressing).toBe(false)
    await new Promise((r) => setTimeout(r, 350))
    h.driver.emit({ kind: 'skyTime', state: { hours: 10, locked: false, speed: 0, followsClock: true } })
    expect(h.session().skybox.progressing).toBe(false)
  })

  it('formats hours as HH:MM', () => {
    expect(formatHours(0)).toBe('00:00')
    expect(formatHours(9.75)).toBe('09:45')
    expect(formatHours(24)).toBe('00:00')
  })
})
