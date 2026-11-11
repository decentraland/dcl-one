import { describe, it, expect, vi, afterEach } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { ToastHost } from '../design'
import { Sidebar } from '../features/sidebar/Sidebar'
import { fakeSession } from './harness'

function renderSidebar(over: { enabled?: boolean; available?: boolean; sceneBlocked?: boolean } = {}) {
  const s = fakeSession()
  s.mic = { ...s.mic, available: true, enabled: false, toggle: vi.fn(), ...over }
  s.settings = { ...s.settings, list: [{ name: 'Voice Volume', category: 'audio', description: '', minValue: 0, maxValue: 100, namedVariants: [], value: 70, default: 100, stepSize: 1 }], set: vi.fn(), load: vi.fn() }
  const view = render(<><Sidebar session={s} /><ToastHost /></>)
  return Object.assign(s, { rerender: (mic: Partial<typeof s.mic>) => view.rerender(<><Sidebar session={{ ...s, mic: { ...s.mic, ...mic } }} /><ToastHost /></>) })
}

afterEach(() => localStorage.clear())

describe('nearby voice', () => {
  it('the voice button opens the popover instead of toggling the mic', async () => {
    const s = renderSidebar()
    await userEvent.click(screen.getByRole('button', { name: 'Nearby Voice' }))
    expect(screen.getByRole('dialog', { name: 'Nearby voice' })).toBeInTheDocument()
    expect(vi.mocked(s.mic.toggle)).not.toHaveBeenCalled()
    expect(vi.mocked(s.settings.load)).toHaveBeenCalled()
  })

  it('Speak opens the mic, and the hint follows it', async () => {
    const s = renderSidebar()
    await userEvent.click(screen.getByRole('button', { name: 'Nearby Voice' }))
    await userEvent.click(screen.getByRole('button', { name: 'Speak' }))
    expect(vi.mocked(s.mic.toggle)).toHaveBeenCalledTimes(1)
  })

  it('the slider sets the incoming voice volume', async () => {
    const s = renderSidebar()
    await userEvent.click(screen.getByRole('button', { name: 'Nearby Voice' }))
    const slider = screen.getByRole('slider', { name: 'Voice volume' })
    expect(slider).toHaveValue('70')
    fireEvent.change(slider, { target: { value: '40' } })
    expect(vi.mocked(s.settings.set)).toHaveBeenLastCalledWith('Voice Volume', 40)
  })

  it('turning hearing off mutes voices, closes the popover, and turning it on restores the volume', async () => {
    const s = renderSidebar({ enabled: true })
    await userEvent.click(screen.getByRole('button', { name: 'Nearby Voice' }))
    await userEvent.click(screen.getByRole('switch', { name: 'Hear others' }))
    expect(vi.mocked(s.settings.set)).toHaveBeenLastCalledWith('Voice Volume', 0)
    expect(vi.mocked(s.mic.toggle)).toHaveBeenCalledTimes(1)
    expect(screen.queryByRole('dialog', { name: 'Nearby voice' })).toBeNull()
    expect(screen.getByRole('button', { name: 'Nearby Voice' })).toHaveAttribute('data-voice', 'off')
    await userEvent.click(screen.getByRole('button', { name: 'Nearby Voice' }))
    await userEvent.click(screen.getByRole('switch', { name: 'Hear others' }))
    expect(vi.mocked(s.settings.set)).toHaveBeenLastCalledWith('Voice Volume', 70)
  })

  it('without a microphone, Speak is disabled and says why', async () => {
    renderSidebar({ available: false })
    await userEvent.click(screen.getByRole('button', { name: 'Nearby Voice' }))
    expect(screen.getByRole('button', { name: 'Speak' })).toBeDisabled()
    expect(screen.getByText('No microphone available')).toBeInTheDocument()
  })

  it('in a scene that turns nearby voice off, the button says so instead of opening the popover', async () => {
    renderSidebar({ sceneBlocked: true })
    const button = screen.getByRole('button', { name: 'Nearby Voice' })
    expect(button).toHaveAttribute('data-voice', 'blocked')
    expect(button.querySelector('[data-indicator]')).toBeNull()
    await userEvent.click(button)
    expect(screen.queryByRole('dialog', { name: 'Nearby voice' })).toBeNull()
    expect(screen.getByRole('status')).toHaveTextContent('Nearby voice chat unavailable in this scene.')
  })

  it('walking into a scene that turns nearby voice off closes the open popover', async () => {
    const s = renderSidebar()
    await userEvent.click(screen.getByRole('button', { name: 'Nearby Voice' }))
    expect(screen.getByRole('dialog', { name: 'Nearby voice' })).toBeInTheDocument()
    s.rerender({ sceneBlocked: true })
    expect(screen.queryByRole('dialog', { name: 'Nearby voice' })).toBeNull()
  })
})
