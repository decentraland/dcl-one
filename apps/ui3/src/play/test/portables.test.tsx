import { describe, it, expect, afterEach } from 'vitest'
import { act, fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { enterAsGuest, fakeSession, renderSession } from './harness'
import { ToastHost, resetPopups } from '../design'
import { Sidebar } from '../features/sidebar/Sidebar'
import type { PortablesMessage } from '../engine/protocol'
import type { PortableInfo, PortableState } from '../engine/generated'

const JETPACK = 'urn:decentraland:matic:collections-v2:0xjet:0'
const PET = 'urn:decentraland:matic:collections-v2:0xpet:0'

const wearable = (pid: string, name: string, state: PortableState, reason: string | null = null): PortableInfo => ({
  hash: state === 'stopped' ? null : `${name.toLowerCase().replace(/ /g, '')}-sw-male`,
  pid,
  name,
  kind: 'smartWearable',
  state,
  reason,
  source: 'equipped'
})
const scenePortable = (name: string, state: PortableState, reason: string | null = null): PortableInfo => ({
  hash: 'bafkminigame',
  pid: 'minigame.dcl.eth',
  name,
  kind: 'scene',
  state,
  reason,
  source: null
})

const list = (portables: PortableInfo[]): PortablesMessage => ({ kind: 'portables', portables })

// Toasts are a module-level store that outlives a test; each test reads only the ones it caused.
const toasts = (): string[] => screen.queryAllByRole('status').map((t) => t.textContent ?? '')
const newToasts = (before: string[]): string[] => toasts().slice(before.length)

afterEach(resetPopups)

// DOMAIN: portables — the engine's list of the portables the player can see and stop.
describe('portables domain', () => {
  it('keeps the list the engine sends; Stop and Restart run the console command without echoing it to chat', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit(list([wearable(JETPACK, 'Jetpack', 'running'), scenePortable('Minigame', 'blocked')]))
    expect(h.session().portables.list.map((p) => [p.kind, p.state])).toEqual([
      ['smartWearable', 'running'],
      ['scene', 'blocked']
    ])

    act(() => h.session().portables.stop('bafkminigame'))
    expect(h.driver.last('consoleCommand')).toEqual({ kind: 'consoleCommand', command: 'stop_portable', args: ['bafkminigame'] })
    act(() => h.session().portables.restart(JETPACK))
    expect(h.driver.last('consoleCommand')).toEqual({ kind: 'consoleCommand', command: 'smart_wearable', args: ['restart', JETPACK] })

    const lines = h.session().chat.nearby.length
    h.driver.emit({ kind: 'consoleReply', command: 'stop_portable', args: ['bafkminigame'], ok: true, output: 'stopped bafkminigame' })
    expect(h.session().chat.nearby).toHaveLength(lines)
    // the same command typed in chat still answers there
    h.driver.emit({ kind: 'consoleReply', command: 'stop_portable', args: ['bafkminigame'], ok: false, output: 'not a listed portable: bafkminigame' })
    expect(h.session().chat.nearby).toHaveLength(lines + 1)
  })

  it('a failed Stop is toasted as an error', async () => {
    render(<ToastHost />)
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().portables.stop('bafkgone'))
    const before = toasts()
    h.driver.emit({ kind: 'consoleReply', command: 'stop_portable', args: ['bafkgone'], ok: false, output: 'not a listed portable: bafkgone' })
    expect(newToasts(before)).toEqual(['not a listed portable: bafkgone'])
  })

  it('toasts each state change once the player is in world, from the list they arrived with', async () => {
    render(<ToastHost />)
    const h = renderSession()
    // sent while logging in: the baseline, never toasted
    h.driver.emit(list([wearable(JETPACK, 'Jetpack', 'running')]))
    await enterAsGuest(h)
    let before = toasts()
    h.driver.emit(list([wearable(JETPACK, 'Jetpack', 'running'), wearable(PET, 'Pocket Pet', 'loading')]))
    expect(newToasts(before)).toEqual([])

    before = toasts()
    h.driver.emit(list([wearable(JETPACK, 'Jetpack', 'blocked'), wearable(PET, 'Pocket Pet', 'running'), scenePortable('Minigame', 'loading')]))
    expect(newToasts(before)).toEqual(['Jetpack is paused in this scene', 'Pocket Pet is running'])

    before = toasts()
    h.driver.emit(list([wearable(JETPACK, 'Jetpack', 'stopped'), wearable(PET, 'Pocket Pet', 'running'), scenePortable('Minigame', 'failed', 'main.js missing')]))
    expect(newToasts(before)).toEqual(['Jetpack stopped', 'Minigame failed to run'])
  })

  it('does not toast the first list after logging in when none came before', async () => {
    render(<ToastHost />)
    const h = renderSession()
    await enterAsGuest(h)
    let before = toasts()
    h.driver.emit(list([wearable(JETPACK, 'Jetpack', 'running'), wearable(PET, 'Pocket Pet', 'running')]))
    expect(newToasts(before)).toEqual([])
    before = toasts()
    h.driver.emit(list([wearable(JETPACK, 'Jetpack', 'stopped'), wearable(PET, 'Pocket Pet', 'running')]))
    expect(newToasts(before)).toEqual(['Jetpack stopped'])
  })

  it('forgets the list on logout', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit(list([wearable(JETPACK, 'Jetpack', 'running')]))
    act(() => h.session().logout())
    expect(h.session().portables.list).toEqual([])
  })
})

describe('sidebar portables button', () => {
  const sessionWith = (portables: PortableInfo[]): ReturnType<typeof fakeSession> => {
    const s = fakeSession()
    s.portables = { ...s.portables, list: portables }
    return s
  }
  const button = (): HTMLElement => screen.getByRole('button', { name: 'Portable experiences' })
  const tooltip = (): string | null => button().parentElement!.querySelector('[role="tooltip"]')!.textContent
  const open = async (portables: PortableInfo[]): Promise<{ s: ReturnType<typeof fakeSession>; popover: ReturnType<typeof within> }> => {
    const s = sessionWith(portables)
    render(<Sidebar session={s} />)
    await userEvent.click(button())
    return { s, popover: within(screen.getByRole('dialog', { name: 'Portable experiences' })) }
  }

  it('is absent while the engine lists no portable', () => {
    render(<Sidebar session={sessionWith([])} />)
    expect(screen.queryByRole('button', { name: 'Portable experiences' })).toBeNull()
  })

  it('shows while one is listed, with how many run and how many the scene pauses', () => {
    render(<Sidebar session={sessionWith([wearable(JETPACK, 'Jetpack', 'running'), wearable(PET, 'Pocket Pet', 'stopped')])} />)
    expect(button()).toHaveAttribute('aria-pressed', 'false')
    expect(tooltip()).toBe('1 running')
  })

  it('says how many the scene pauses', () => {
    render(<Sidebar session={sessionWith([wearable(JETPACK, 'Jetpack', 'blocked'), scenePortable('Minigame', 'blocked')])} />)
    expect(tooltip()).toBe('0 running, 2 paused in this scene')
  })

  it('lists exactly what the engine sends, one row per portable of either kind', async () => {
    const { popover } = await open([wearable(JETPACK, 'Jetpack', 'running'), scenePortable('Minigame', 'running')])
    expect(button()).toHaveAttribute('aria-pressed', 'true')
    expect(popover.getByRole('heading', { name: 'Portable experiences' })).toBeInTheDocument()
    expect(popover.getByText('Smart wearable')).toBeInTheDocument()
    expect(popover.getByText('Portable experience')).toBeInTheDocument()
    expect(popover.getAllByRole('button').map((b: HTMLElement) => b.getAttribute('aria-label'))).toEqual(['Stop Jetpack', 'Stop Minigame'])
  })

  it('Stop sends the scene hash of any kind; Restart the urn of a stopped smart wearable', async () => {
    const { s, popover } = await open([wearable(JETPACK, 'Jetpack', 'stopped'), scenePortable('Minigame', 'running')])
    expect(popover.queryByRole('button', { name: 'Stop Jetpack' })).toBeNull()
    await userEvent.click(popover.getByRole('button', { name: 'Stop Minigame' }))
    expect(s.portables.stop).toHaveBeenCalledWith('bafkminigame')
    await userEvent.click(popover.getByRole('button', { name: 'Restart Jetpack' }))
    expect(s.portables.restart).toHaveBeenCalledWith(JETPACK)
    fireEvent.pointerDown(document.body)
    expect(screen.queryByRole('dialog', { name: 'Portable experiences' })).toBeNull()
  })

  it('shows stopped, failed with its reason, and paused; a failed or paused one can still be stopped', async () => {
    const { popover } = await open([
      wearable(JETPACK, 'Jetpack', 'stopped'),
      scenePortable('Minigame', 'failed', 'main.js missing'),
      wearable(PET, 'Pocket Pet', 'blocked')
    ])
    expect(popover.getByText('Stopped')).toBeInTheDocument()
    expect(popover.getByText('Failed: main.js missing')).toBeInTheDocument()
    expect(popover.getByText('Paused in this scene')).toBeInTheDocument()
    expect(popover.getByRole('button', { name: 'Stop Minigame' })).toBeInTheDocument()
    expect(popover.getByRole('button', { name: 'Stop Pocket Pet' })).toBeInTheDocument()
  })

  it('a smart wearable still waiting for its data has no Stop', async () => {
    const { popover } = await open([{ ...wearable(PET, 'Pocket Pet', 'loading'), hash: null }])
    expect(popover.getByText('Loading')).toBeInTheDocument()
    expect(popover.queryAllByRole('button')).toEqual([])
  })
})
