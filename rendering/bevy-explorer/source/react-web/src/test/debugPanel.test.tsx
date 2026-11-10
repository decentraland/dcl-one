import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import { PopupHost, resetPopups } from '@hud/design/popups'
import { Sidebar } from '../features/sidebar/Sidebar'
import { requestDebugPanel } from '../features/debug/useDebugPanel'
import { fakeSession } from './harness'

it.each([true, false])('gates the direct debug launcher on the engine development build (%s)', async (development) => {
  const session = fakeSession()
  session.runDebugCommand = vi.fn(async (line: string) =>
    line === '/debug_panel --dev-build' ? String(development) : '[]')
  render(<><Sidebar session={session} /><PopupHost /></>)
  await waitFor(() => expect(session.runDebugCommand).toHaveBeenCalledWith('/debug_panel --dev-build'))
  const button = { name: 'Toggle debug panel' }
  if (!development) {
    expect(screen.queryByRole('button', button)).toBeNull()
    return
  }
  const launcher = () => screen.getByRole('button', button)
  expect(await screen.findByRole('button', button)).toHaveAttribute('aria-pressed', 'false')
  await act(async () => { fireEvent.click(launcher()) })
  expect(screen.getByRole('dialog', { name: 'Debug panel' })).toBeInTheDocument()
  expect(launcher()).toHaveAttribute('aria-pressed', 'true')
  fireEvent.click(launcher())
  expect(screen.queryByRole('dialog', { name: 'Debug panel' })).toBeNull()
  await act(async () => requestDebugPanel('', vi.fn()))
  expect(launcher()).toHaveAttribute('aria-pressed', 'true')
  fireEvent.click(screen.getByRole('button', { name: 'Close' }))
  expect(launcher()).toHaveAttribute('aria-pressed', 'false')
})

it('discovers plugin sections, samples expanded sections and dispatches their actions', async () => {
  vi.useFakeTimers()
  const session = fakeSession()
  const sections = [
    { title: 'Transport', query: '/transport_test', fields: [['Connected', 'room.connected']], actions: [] },
    { title: 'Clock', query: '/clock_test', fields: [['Seconds', 'seconds']], actions: [['Advance', '/advance_test']] }
  ]
  let seconds = 1
  const command = vi.fn(async (line: string) => {
    if (line === '/debug_panel --dev-build') return 'false'
    if (line === '/debug_panel') return JSON.stringify(sections)
    if (line === '/clock_test') return JSON.stringify({ seconds })
    if (line === '/advance_test') { seconds += 10; return 'Clock advanced' }
    throw new Error('Unexpected command ' + line)
  })
  session.runDebugCommand = command
  try {
    render(<><Sidebar session={session} /><PopupHost /></>)
    fireEvent.click(screen.getByRole('button', { name: 'Sidebar settings' }))
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Debug panel' })) })
    expect(screen.getByRole('dialog', { name: 'Debug panel' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Transport' })).toHaveAttribute('aria-expanded', 'false')
    expect(command.mock.calls).toEqual([['/debug_panel --dev-build'], ['/debug_panel']])
    const clock = screen.getByRole('button', { name: 'Clock' })
    const sectionHeaders = () => screen.getAllByRole('button', { name: /^(Transport|Clock|Session)$/ })
    const originalOrder = sectionHeaders()
    await act(async () => { fireEvent.click(clock) })
    expect(sectionHeaders()).toEqual(originalOrder)
    expect(screen.getByText('1')).toBeInTheDocument()
    seconds = 2
    await act(async () => { await vi.advanceTimersByTimeAsync(1000) })
    expect(screen.getByText('2')).toBeInTheDocument()
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Advance' })) })
    expect(command).toHaveBeenCalledWith('/advance_test')
    await act(async () => { await vi.advanceTimersByTimeAsync(1000) })
    expect(screen.getByText('12')).toBeInTheDocument()
    fireEvent.click(clock)
    expect(sectionHeaders()).toEqual(originalOrder)
    command.mockClear()
    await act(async () => { await vi.advanceTimersByTimeAsync(3000) })
    expect(command).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Close' }))
    expect(screen.queryByRole('dialog', { name: 'Debug panel' })).toBeNull()
  } finally {
    act(() => resetPopups())
    vi.useRealTimers()
  }
})

it('opens a named section from chat and applies plugin-declared values', async () => {
  const session = fakeSession()
  const section = { title: 'Graphics', query: '/metrics', fields: [['GPU', 'gpu']], actions: [], inputs: [
    { label: 'Scale', field: 'scale', command: '/scale {value}', kind: { type: 'number', options: { min: 0.01, max: 10, step: 0.01 } } },
    { label: 'Fog', field: 'fog', command: '/fog {value}', kind: { type: 'toggle' } },
    { label: 'Mode', field: 'mode', command: '/mode {value}', kind: { type: 'select', options: [['Low', 'low'], ['High', 'high']] } }
  ] }
  session.runDebugCommand = vi.fn(async (line: string) => line === '/debug_panel' ? JSON.stringify([section]) :
    line === '/metrics' ? JSON.stringify({ scale: 0.800000011920929, fog: true, mode: 'low', gpu: { samples: [1, 3, 2], unit: 'ms' } }) : 'Updated')
  render(<><Sidebar session={session} /><PopupHost /></>)
  await act(async () => requestDebugPanel('Graphics', vi.fn()))
  expect(screen.getByRole('img', { name: 'GPU history' })).toBeInTheDocument()
  expect(screen.getByRole('spinbutton', { name: 'Scale' })).toHaveDisplayValue('0.8')
  fireEvent.change(screen.getByRole('spinbutton', { name: 'Scale' }), { target: { value: '4' } })
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Apply Scale' })) })
  expect(session.runDebugCommand).toHaveBeenCalledWith('/scale 4')
  await act(async () => { fireEvent.click(screen.getByRole('switch', { name: 'Fog' })) })
  expect(session.runDebugCommand).toHaveBeenCalledWith('/fog false')
  fireEvent.click(screen.getByRole('button', { name: 'Mode' }))
  await act(async () => { fireEvent.click(screen.getByRole('option', { name: 'High' })) })
  expect(session.runDebugCommand).toHaveBeenCalledWith('/mode high')
  fireEvent.click(screen.getByRole('button', { name: 'Close' }))
})

it('shows only the next connection action and follows changes from the engine', async () => {
  vi.useFakeTimers()
  const session = fakeSession()
  let enabled: boolean | null = true
  let refuseDisconnect = true
  const input = { label: 'Connection', field: 'enabled', command: '/connection {value}', kind: {
    type: 'actionButton', options: { enable: ['Connect', 'connect'], disable: ['Disconnect', 'disconnect'] }
  } }
  session.runDebugCommand = vi.fn(async (line: string) => {
    if (line === '/debug_panel --dev-build') return 'true'
    if (line === '/debug_panel') return JSON.stringify([{ title: 'Pulse', query: '/state', fields: [], actions: [], inputs: [input] }])
    if (line === '/state') return JSON.stringify({ enabled, unavailable: { enabled: 'No session' } })
    if (line === '/connection disconnect') {
      if (refuseDisconnect) throw new Error('Connection policy changed')
      enabled = false
    }
    if (line === '/connection connect') enabled = true
    return 'Applied'
  })
  try {
    render(<><Sidebar session={session} /><PopupHost /></>)
    await act(async () => requestDebugPanel('Pulse', vi.fn()))
    const action = (name: string) => screen.queryByRole('button', { name })
    expect(action('Connect')).toBeNull()
    const click = (name: string) => act(async () => { fireEvent.click(screen.getByRole('button', { name })) })
    await click('Disconnect')
    expect(screen.getByText(/Connection policy changed/)).toBeInTheDocument()
    expect(action('Disconnect')).toBeEnabled()
    expect(action('Connect')).toBeNull()
    refuseDisconnect = false
    await click('Disconnect')
    expect(enabled).toBe(false)
    expect(action('Disconnect')).toBeNull()
    await click('Connect')
    expect(enabled).toBe(true)
    expect(action('Disconnect')).toBeEnabled()
    enabled = null
    await act(async () => { await vi.advanceTimersByTimeAsync(1000) })
    expect(action('Connect')).toBeDisabled()
    expect(screen.getByText(/No session/)).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Close' }))
  } finally {
    act(() => resetPopups())
    vi.useRealTimers()
  }
})

it('keeps the caption with each number and separates unlabelled tuples too', async () => {
  const session = fakeSession()
  const fields = [['Scene definitions per batch (min / mean / max)', 'batch'], ['Samples', 'pair']]
  session.runDebugCommand = vi.fn(async (line: string) => line === '/debug_panel'
    ? JSON.stringify([{ title: 'Entity Requests', query: '/counts', fields, actions: [] }])
    : JSON.stringify({ batch: [9, 269.114, 430], pair: [0, -2.5] }))
  render(<><Sidebar session={session} /><PopupHost /></>)
  await act(async () => requestDebugPanel('Entity Requests', vi.fn()))
  const group = within(screen.getByRole('list', { name: 'Scene definitions per batch' })).getAllByRole('listitem')
  expect(group.map(cell => cell.textContent?.trim())).toEqual(['min 9', 'mean 269.11', 'max 430'])
  expect(within(screen.getByRole('list', { name: 'Samples' })).getAllByRole('listitem')).toHaveLength(2)
  fireEvent.click(screen.getByRole('button', { name: 'Close' }))
})

it('explains absent readings once per cause and replaces the explanation when data arrives', async () => {
  const session = fakeSession()
  const fields = [['Peer', 'room.peer'], ['Rate', 'room.rate'], ['Peak rate', 'room.peak'],
    ['Other room', 'roommate'], ['Packets', 'count'], ['Muted', 'muted']]
  let room: Record<string, unknown> | null = null
  session.runDebugCommand = vi.fn(async (line: string) => line === '/debug_panel'
    ? JSON.stringify([{ title: 'Room', query: '/metrics', fields, actions: [] }])
    : JSON.stringify({ room, count: 0, muted: false, unavailable: {
      'room.peer': 'Select a peer', room: 'Waiting for a room', '': 'No sample yet'
    } }))
  render(<><Sidebar session={session} /><PopupHost /></>)
  await act(async () => requestDebugPanel('Room', vi.fn()))
  expect(screen.getAllByText('Waiting for a room')).toHaveLength(1)
  expect(screen.getByText('Select a peer')).toBeInTheDocument()
  expect(screen.getByText('No sample yet')).toBeInTheDocument()
  expect(screen.getByText('0')).toBeInTheDocument()
  expect(screen.getByText('No')).toBeInTheDocument()
  room = { peer: 'Dawn', rate: 5, peak: 6 }
  expect(await screen.findByText('Dawn', {}, { timeout: 2500 })).toBeInTheDocument()
  expect(screen.queryByText('Waiting for a room')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Close' }))
})

it('closes the panel and its subscriptions when the world sidebar unmounts', async () => {
  const session = fakeSession()
  session.runDebugCommand = vi.fn(async () => '[]')
  const tree = render(<><Sidebar session={session} /><PopupHost /></>)
  fireEvent.click(screen.getByRole('button', { name: 'Sidebar settings' }))
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Debug panel' })) })
  expect(screen.getByRole('button', { name: 'Session' })).toBeInTheDocument()
  tree.rerender(<><PopupHost /></>)
  expect(screen.queryByRole('dialog', { name: 'Debug panel' })).toBeNull()
})
