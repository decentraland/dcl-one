import { describe, it, expect, afterEach, vi } from 'vitest'
import { act, fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { enterAsGuest, fakeSession, renderSession } from './harness'
import { PopupHost, ToastHost, resetPopups } from '../design'
import { Sidebar } from '../features/sidebar/Sidebar'
import { openSmartWearableAuthorization } from '../features/permissions/SmartWearableAuthorizationDialog'
import type { SmartWearableAuthRequestMessage, SmartWearableStateMessage } from '../engine/protocol'
import type { SmartWearableEntryInfo, SmartWearableState } from '../engine/generated'

const JETPACK = 'urn:decentraland:matic:collections-v2:0xjet:0'
const PET = 'urn:decentraland:matic:collections-v2:0xpet:0'

const AUTH: SmartWearableAuthRequestMessage = {
  kind: 'smartWearableAuthRequest',
  id: 3,
  urn: JETPACK,
  name: 'Jetpack',
  permissions: ['USE_FETCH', 'USE_WEBSOCKET', 'USE_WEB3_API', 'OPEN_EXTERNAL_LINK', 'USE_SOMETHING_NEW'],
  reason: 'equip'
}

const entry = (urn: string, name: string, state: SmartWearableState): SmartWearableEntryInfo => ({ urn, name, state, reason: null, sceneHash: null, source: 'equipped' })

const report = (entries: SmartWearableEntryInfo[], currentSceneAllows = true): SmartWearableStateMessage => ({
  kind: 'smartWearableState',
  entries,
  currentSceneAllows,
  currentSceneHidesUi: false
})

// Toasts are a module-level store that outlives a test; each test reads only the ones it caused.
const toasts = (): string[] => screen.queryAllByRole('status').map((t) => t.textContent ?? '')
const newToasts = (before: string[]): string[] => toasts().slice(before.length)

afterEach(resetPopups)

// DOMAIN: smart wearables — the engine's authorization prompt and the state it reports.
describe('smart wearables domain', () => {
  it('holds an authorization request until it is answered, then posts the answer', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit(AUTH)
    expect(h.session().smartWearables.pendingAuth).toMatchObject({ id: 3, name: 'Jetpack' })
    act(() => h.session().smartWearables.answerAuth(3, true))
    expect(h.driver.last('smartWearableAuthAnswer')).toEqual({ kind: 'smartWearableAuthAnswer', id: 3, allow: true })
    expect(h.session().smartWearables.pendingAuth).toBeNull()
  })

  it('keeps the reported state; Stop and Restart run the console command without echoing it to chat', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit(report([entry(JETPACK, 'Jetpack', 'running')], false))
    expect(h.session().smartWearables.entries.map((e) => e.state)).toEqual(['running'])
    expect(h.session().smartWearables.currentSceneAllows).toBe(false)

    act(() => h.session().smartWearables.kill(JETPACK))
    expect(h.driver.last('consoleCommand')).toEqual({ kind: 'consoleCommand', command: 'smart_wearable', args: ['kill', JETPACK] })
    act(() => h.session().smartWearables.restart(JETPACK))
    expect(h.driver.last('consoleCommand')).toEqual({ kind: 'consoleCommand', command: 'smart_wearable', args: ['restart', JETPACK] })

    const lines = h.session().chat.nearby.length
    h.driver.emit({ kind: 'consoleReply', command: 'smart_wearable', args: ['kill', JETPACK], ok: true, output: `killed ${JETPACK}` })
    expect(h.session().chat.nearby).toHaveLength(lines)
    // the same command typed in chat still answers there
    h.driver.emit({ kind: 'consoleReply', command: 'smart_wearable', args: ['kill', JETPACK], ok: true, output: `not running: ${JETPACK}` })
    expect(h.session().chat.nearby).toHaveLength(lines + 1)
  })

  it('a failed Stop is toasted as an error', async () => {
    render(<ToastHost />)
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().smartWearables.kill(PET))
    const before = toasts()
    h.driver.emit({ kind: 'consoleReply', command: 'smart_wearable', args: ['kill', PET], ok: false, output: `not running: ${PET}` })
    expect(newToasts(before)).toEqual([`not running: ${PET}`])
  })

  it('toasts each state change once the player is in world, from the state they arrived with', async () => {
    render(<ToastHost />)
    const h = renderSession()
    // reported while logging in: the baseline, never toasted
    h.driver.emit(report([entry(JETPACK, 'Jetpack', 'running')]))
    await enterAsGuest(h)
    let before = toasts()
    h.driver.emit(report([entry(JETPACK, 'Jetpack', 'running'), entry(PET, 'Pocket Pet', 'loading')]))
    expect(newToasts(before)).toEqual([])

    before = toasts()
    h.driver.emit(report([entry(JETPACK, 'Jetpack', 'blocked'), entry(PET, 'Pocket Pet', 'running')]))
    expect(newToasts(before)).toEqual(['Jetpack is disabled in this scene', 'Pocket Pet is running'])

    before = toasts()
    h.driver.emit(report([entry(JETPACK, 'Jetpack', 'killed'), entry(PET, 'Pocket Pet', 'denied')]))
    expect(newToasts(before)).toEqual(['Jetpack stopped', 'Pocket Pet was not allowed to run'])
  })

  it('does not toast the first report after logging in when none came before', async () => {
    render(<ToastHost />)
    const h = renderSession()
    await enterAsGuest(h)
    let before = toasts()
    h.driver.emit(report([entry(JETPACK, 'Jetpack', 'running'), entry(PET, 'Pocket Pet', 'running')]))
    expect(newToasts(before)).toEqual([])
    before = toasts()
    h.driver.emit(report([entry(JETPACK, 'Jetpack', 'killed'), entry(PET, 'Pocket Pet', 'running')]))
    expect(newToasts(before)).toEqual(['Jetpack stopped'])
  })
})

describe('smart wearable authorization dialog', () => {
  const open = (): ReturnType<typeof vi.fn> => {
    const answer = vi.fn()
    render(<PopupHost />)
    act(() => {
      openSmartWearableAuthorization(AUTH, answer)
    })
    return answer
  }

  it('names the wearable, why it asks, and its permissions in the scene permission wording', () => {
    open()
    const dialog = within(screen.getByRole('dialog', { name: 'Smart wearable permissions' }))
    expect(dialog.getByText(/wants to run while you wear it/)).toHaveTextContent('The smart wearable Jetpack wants to run while you wear it')
    expect(dialog.getByText(/You equipped/)).toHaveTextContent('You equipped Jetpack.')
    expect(dialog.getAllByRole('listitem').map((li) => li.textContent)).toEqual([
      'fetch data from a remote server',
      'open a web socket to communicate with a remote server',
      'initiate a web3 transaction with your wallet',
      'open a url in your browser',
      'USE_SOMETHING_NEW'
    ])
  })

  it('says the player logged in wearing it', () => {
    const answer = vi.fn()
    render(<PopupHost />)
    act(() => {
      openSmartWearableAuthorization({ ...AUTH, reason: 'login', permissions: [] }, answer)
    })
    expect(screen.getByText(/You logged in wearing/)).toHaveTextContent('You logged in wearing Jetpack.')
    expect(screen.queryByRole('list')).toBeNull()
  })

  it.each([
    ['Allow', true],
    ['Deny', false]
  ])('%s answers the request and closes', async (button, allow) => {
    const answer = open()
    await userEvent.click(screen.getByRole('button', { name: button }))
    expect(answer).toHaveBeenCalledExactlyOnceWith(3, allow)
    expect(screen.queryByRole('dialog', { name: 'Smart wearable permissions' })).toBeNull()
  })
})

describe('sidebar smart wearables button', () => {
  const sessionWith = (entries: SmartWearableEntryInfo[], currentSceneAllows = true): ReturnType<typeof fakeSession> => {
    const s = fakeSession()
    s.smartWearables = { ...s.smartWearables, entries, currentSceneAllows }
    return s
  }
  const tooltip = (): string | null => screen.getByRole('button', { name: 'Smart wearables' }).parentElement!.querySelector('[role="tooltip"]')!.textContent

  it('is absent while no smart wearable runs, is held back or waits', () => {
    render(<Sidebar session={sessionWith([entry(JETPACK, 'Jetpack', 'loading'), entry(PET, 'Pocket Pet', 'notSmart')])} />)
    expect(screen.queryByRole('button', { name: 'Smart wearables' })).toBeNull()
  })

  it('shows while one runs, with the active and inactive counts', () => {
    render(<Sidebar session={sessionWith([entry(JETPACK, 'Jetpack', 'running'), entry(PET, 'Pocket Pet', 'killed')])} />)
    expect(screen.getByRole('button', { name: 'Smart wearables' })).toHaveAttribute('aria-pressed', 'false')
    expect(tooltip()).toBe('1 active / 1 inactive')
  })

  it('says when the scene disables them', () => {
    render(<Sidebar session={sessionWith([entry(JETPACK, 'Jetpack', 'blocked')], false)} />)
    expect(tooltip()).toBe('Smart wearables are disabled in this scene')
  })

  it('opens the list: Stop a running one, Restart a stopped one', async () => {
    const s = sessionWith([entry(JETPACK, 'Jetpack', 'running'), entry(PET, 'Pocket Pet', 'killed')])
    render(<Sidebar session={s} />)
    await userEvent.click(screen.getByRole('button', { name: 'Smart wearables' }))
    expect(screen.getByRole('button', { name: 'Smart wearables' })).toHaveAttribute('aria-pressed', 'true')
    const popover = within(screen.getByRole('dialog', { name: 'Smart wearables' }))
    expect(popover.getByRole('heading', { name: 'Smart wearables' })).toBeInTheDocument()
    expect(popover.queryByText('Smart wearables are disabled in this scene')).toBeNull()
    expect(popover.queryByRole('button', { name: 'Restart Jetpack' })).toBeNull()
    await userEvent.click(popover.getByRole('button', { name: 'Stop Jetpack' }))
    expect(s.smartWearables.kill).toHaveBeenCalledWith(JETPACK)
    await userEvent.click(popover.getByRole('button', { name: 'Restart Pocket Pet' }))
    expect(s.smartWearables.restart).toHaveBeenCalledWith(PET)
    fireEvent.pointerDown(document.body)
    expect(screen.queryByRole('dialog', { name: 'Smart wearables' })).toBeNull()
  })

  it('the list says when the scene disables them', async () => {
    render(<Sidebar session={sessionWith([entry(JETPACK, 'Jetpack', 'blocked')], false)} />)
    await userEvent.click(screen.getByRole('button', { name: 'Smart wearables' }))
    const popover = within(screen.getByRole('dialog', { name: 'Smart wearables' }))
    expect(popover.getByText('Smart wearables are disabled in this scene')).toBeInTheDocument()
    expect(popover.getByText('Disabled in this scene')).toBeInTheDocument()
  })
})
