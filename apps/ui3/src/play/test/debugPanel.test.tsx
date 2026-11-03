import { act, fireEvent, render, screen } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import { PopupHost, resetPopups } from '../design/popups'
import { Sidebar } from '../features/sidebar/Sidebar'
import { requestDebugPanel } from '../features/debug/useDebugPanel'
import { fakeSession } from './harness'

it('discovers plugin sections, samples expanded sections and dispatches their actions', async () => {
  vi.useFakeTimers()
  const session = fakeSession()
  const sections = [
    { title: 'Transport', query: '/transport_test', fields: [['Connected', 'room.connected']], actions: [] },
    { title: 'Clock', query: '/clock_test', fields: [['Seconds', 'seconds']], actions: [['Advance', '/advance_test']] }
  ]
  let seconds = 1
  const command = vi.fn(async (line: string) => {
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
    expect(command.mock.calls).toEqual([['/debug_panel']])
    const clock = screen.getByRole('button', { name: 'Clock' })
    await act(async () => { fireEvent.click(clock) })
    expect(screen.getByText('1')).toBeInTheDocument()
    seconds = 2
    await act(async () => { await vi.advanceTimersByTimeAsync(1000) })
    expect(screen.getByText('2')).toBeInTheDocument()
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Advance' })) })
    expect(command).toHaveBeenCalledWith('/advance_test')
    await act(async () => { await vi.advanceTimersByTimeAsync(1000) })
    expect(screen.getByText('12')).toBeInTheDocument()
    fireEvent.click(clock)
    command.mockClear()
    await act(async () => { await vi.advanceTimersByTimeAsync(3000) })
    expect(command).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Close' }))
    expect(screen.queryByRole('dialog', { name: 'Debug panel' })).toBeNull()
  } finally {
    resetPopups()
    vi.useRealTimers()
  }
})

it('opens a named section from chat and applies plugin-declared values', async () => {
  const session = fakeSession()
  const section = { title: 'Graphics', query: '/metrics', fields: [['GPU', 'gpu']], actions: [], inputs: [
    { label: 'Scale', field: 'scale', command: '/scale {value}', kind: { type: 'number', options: { min: 1, max: 10, step: 1 } } },
    { label: 'Fog', field: 'fog', command: '/fog {value}', kind: { type: 'toggle' } },
    { label: 'Mode', field: 'mode', command: '/mode {value}', kind: { type: 'select', options: [['Low', 'low'], ['High', 'high']] } }
  ] }
  session.runDebugCommand = vi.fn(async (line: string) => line === '/debug_panel' ? JSON.stringify([section]) :
    line === '/metrics' ? JSON.stringify({ scale: 2, fog: true, mode: 'low', gpu: { samples: [1, 3, 2], unit: 'ms' } }) : 'Updated')
  render(<><Sidebar session={session} /><PopupHost /></>)
  await act(async () => requestDebugPanel('Graphics', vi.fn()))
  expect(screen.getByRole('img', { name: 'GPU history' })).toBeInTheDocument()
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
