import { describe, it, expect, vi, beforeEach } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { SettingsPanel } from '../../features/settings/SettingsPanel'
import type { Setting } from '@explorer-core/engine/protocol'
import type { SettingsState } from '../../features/session/useEngineSession'
import { fakeProfileState, fakeSession } from '../harness'

const base = (over: Partial<Setting>): Setting => ({
  name: 'x',
  category: 'general',
  description: '',
  minValue: 0,
  maxValue: 100,
  namedVariants: [],
  value: 0,
  default: 0,
  stepSize: 1,
  disabled: false,
  ...over
})

const TOGGLE = base({ name: 'fullscreen', category: 'general', namedVariants: [{ name: 'Off', description: '' }, { name: 'On', description: '' }], value: 0, default: 1, maxValue: 1 })
const SLIDER = base({ name: 'fov', category: 'general', value: 70, default: 60, minValue: 60, maxValue: 100, stepSize: 5 })
const SELECT = base({ name: 'quality', category: 'graphics', description: 'Overall quality.\n\nLow: fastest.', namedVariants: [{ name: 'Low', description: 'Overall quality.\n\nLow: fastest.' }, { name: 'Med', description: 'Overall quality.\n\nMed: balanced.' }, { name: 'High', description: 'Overall quality.\n\nHigh: prettiest.' }], value: 0, default: 1 })
// The engine's own panel has no label, so its descriptions often lead with the name.
const DESCRIBED = base({ name: 'Camera Sensitivity', category: 'controls', description: 'Camera Sensitivity\n\nControls the sensitivity of camera inputs.\n\nNOTE: not pointer inputs.' })
// f32 values widened to f64 by the bridge
const FLOAT = base({ name: 'Jump Height', category: 'controls', value: 19.950000762939453, minValue: 0.05000000074505806, maxValue: 20, stepSize: 0.05000000074505806 })

function renderPanel(): SettingsState {
  const settings: SettingsState = { ...fakeSession().settings, open: true, list: [TOGGLE, SLIDER, SELECT, DESCRIBED, FLOAT], set: vi.fn() }
  render(
    <SettingsPanel
      settings={settings}
      bindings={fakeSession().bindings}
      profile={fakeProfileState()}
      onNavigate={vi.fn()}
    />
  )
  return settings
}

describe('settings panel controls', () => {
  let settings: SettingsState
  beforeEach(() => {
    settings = renderPanel()
  })

  it('toggle control sets 1 when turned on', async () => {
    await userEvent.click(screen.getByRole('switch', { name: 'fullscreen' }))
    expect(vi.mocked(settings.set)).toHaveBeenCalledWith('fullscreen', 1)
  })

  it('slider arrows step the value by stepSize', async () => {
    await userEvent.click(screen.getByRole('button', { name: 'increase' }))
    expect(vi.mocked(settings.set)).toHaveBeenCalledWith('fov', 75)
    await userEvent.click(screen.getByRole('button', { name: 'decrease' }))
    expect(vi.mocked(settings.set)).toHaveBeenCalledWith('fov', 65)
  })

  it('reset all defaults sets every field in the active category to its default', async () => {
    await userEvent.click(screen.getByRole('button', { name: /Reset all defaults/i }))
    expect(vi.mocked(settings.set)).toHaveBeenCalledWith('fullscreen', 1)
    expect(vi.mocked(settings.set)).toHaveBeenCalledWith('fov', 60)
  })

  it('a description is the label tooltip, minus the leading name line; none for an empty one', async () => {
    await userEvent.click(screen.getByRole('tab', { name: 'Controls' }))
    const tips = screen.getAllByRole('tooltip').map((t) => t.textContent)
    expect(tips).toEqual(['Controls the sensitivity of camera inputs.\n\nNOTE: not pointer inputs.'])
  })

  it('shows a slider value at f32 precision, not the widened f64', async () => {
    await userEvent.click(screen.getByRole('tab', { name: 'Controls' }))
    expect(screen.getByText('19.95')).toBeTruthy()
    expect(screen.queryByText('19.950000762939453')).toBeNull()
  })

  it('switching category tab + selecting an option sets the variant index', async () => {
    await userEvent.click(screen.getByRole('tab', { name: 'Graphics' }))
    await userEvent.click(screen.getByRole('button', { name: 'quality' }))
    await userEvent.click(screen.getByRole('option', { name: 'High' }))
    expect(vi.mocked(settings.set)).toHaveBeenCalledWith('quality', 2)
  })

  it("an enum's tooltip is the selected variant's description, from the list already held", async () => {
    await userEvent.click(screen.getByRole('tab', { name: 'Graphics' }))
    expect(screen.getByRole('tooltip').textContent).toBe('Overall quality.\n\nLow: fastest.')
    cleanup()
    render(
      <SettingsPanel
        settings={{ ...settings, list: [{ ...SELECT, value: 2 }] }}
        bindings={fakeSession().bindings}
        profile={fakeProfileState()}
        onNavigate={vi.fn()}
      />
    )
    expect(screen.getByRole('tooltip').textContent).toBe('Overall quality.\n\nHigh: prettiest.')
  })
})

describe('settings panel disabled controls', () => {
  it('a disabled select or toggle does not set', async () => {
    const FPS = base({ name: 'Target Frame Rate', category: 'Performance', namedVariants: [{ name: '30 fps', description: '' }, { name: '60 fps', description: '' }, { name: 'Uncapped', description: '' }], value: 1, default: 1, maxValue: 3, disabled: true })
    const VSYNC = base({ name: 'VSync', category: 'Performance', namedVariants: [{ name: 'Off', description: '' }, { name: 'On', description: '' }], value: 1, maxValue: 2, disabled: true })
    const settings: SettingsState = { ...fakeSession().settings, open: true, list: [FPS, VSYNC], set: vi.fn() }
    render(<SettingsPanel settings={settings} bindings={fakeSession().bindings} profile={fakeProfileState()} onNavigate={vi.fn()} />)
    const select = screen.getByRole('button', { name: 'Target Frame Rate' })
    expect(select).toBeDisabled()
    expect(screen.getByRole('switch', { name: 'VSync' })).toBeDisabled()
    await userEvent.click(select)
    expect(vi.mocked(settings.set)).not.toHaveBeenCalled()
  })

  it('reset skips a negative default', async () => {
    const RESOLUTION = base({ name: 'Resolution', category: 'Graphics', namedVariants: [{ name: 'Desktop', description: '' }], value: -1, default: -1, maxValue: 1 })
    const settings: SettingsState = { ...fakeSession().settings, open: true, list: [RESOLUTION, { ...SELECT, category: 'Graphics' }], set: vi.fn() }
    render(<SettingsPanel settings={settings} bindings={fakeSession().bindings} profile={fakeProfileState()} onNavigate={vi.fn()} />)
    await userEvent.click(screen.getByRole('button', { name: /Reset all defaults/i }))
    expect(vi.mocked(settings.set)).toHaveBeenCalledTimes(1)
    expect(vi.mocked(settings.set)).toHaveBeenCalledWith('quality', 1)
  })

  it('a disabled slider has no live arrows and does not set', async () => {
    const FOV = { ...SLIDER, disabled: true }
    const settings: SettingsState = { ...fakeSession().settings, open: true, list: [FOV], set: vi.fn() }
    render(<SettingsPanel settings={settings} bindings={fakeSession().bindings} profile={fakeProfileState()} onNavigate={vi.fn()} />)
    expect(screen.getByRole('slider', { name: 'fov' })).toBeDisabled()
    for (const name of ['increase', 'decrease']) {
      const arrow = screen.getByRole('button', { name })
      expect(arrow).toBeDisabled()
      await userEvent.click(arrow)
    }
    expect(vi.mocked(settings.set)).not.toHaveBeenCalled()
  })

  it('a field re-renders when its disabled state changes at the same value', () => {
    const FPS = base({ name: 'Target Frame Rate', category: 'Performance', namedVariants: [{ name: '30 fps', description: '' }, { name: '60 fps', description: '' }], value: 1, maxValue: 2 })
    // one `set` across renders, as the session gives, so the memo compares only the setting
    const set = vi.fn()
    const panel = (list: Setting[]) => <SettingsPanel settings={{ ...fakeSession().settings, open: true, list, set }} bindings={fakeSession().bindings} profile={fakeProfileState()} onNavigate={vi.fn()} />
    const { rerender } = render(panel([FPS]))
    expect(screen.getByRole('button', { name: 'Target Frame Rate' })).not.toBeDisabled()
    rerender(panel([{ ...FPS, disabled: true }]))
    expect(screen.getByRole('button', { name: 'Target Frame Rate' })).toBeDisabled()
  })

  it('an open list closes when its field is disabled and stays closed when re-enabled', async () => {
    const RESOLUTION = base({ name: 'Resolution', category: 'Graphics', namedVariants: [{ name: '2560x1440', description: '' }, { name: '1920x1080', description: '' }], value: 1, maxValue: 2 })
    const set = vi.fn()
    const panel = (list: Setting[]) => <SettingsPanel settings={{ ...fakeSession().settings, open: true, list, set }} bindings={fakeSession().bindings} profile={fakeProfileState()} onNavigate={vi.fn()} />
    const { rerender } = render(panel([RESOLUTION]))
    await userEvent.click(screen.getByRole('button', { name: 'Resolution' }))
    expect(screen.getAllByRole('option')).toHaveLength(2)
    // e.g. Window mode switched to Windowed from the keyboard: no outside mousedown
    rerender(panel([{ ...RESOLUTION, disabled: true }]))
    expect(screen.queryByRole('option')).toBeNull()
    expect(screen.getByRole('button', { name: 'Resolution' })).toHaveAttribute('aria-expanded', 'false')
    rerender(panel([RESOLUTION]))
    expect(screen.queryByRole('option')).toBeNull()
    expect(set).not.toHaveBeenCalled()
  })
})

// A run-time list (Resolution: the monitor's fullscreen sizes) can hold a single choice, and holds
// one greyed-out placeholder until the monitor is read.
describe('settings panel run-time lists', () => {
  const RESOLUTION = base({ name: 'Resolution', category: 'Graphics', namedVariants: [{ name: '1366x768', description: '' }], value: 0, default: 0, maxValue: 1 })
  const panel = (list: Setting[], set = vi.fn()) => <SettingsPanel settings={{ ...fakeSession().settings, open: true, list, set }} bindings={fakeSession().bindings} profile={fakeProfileState()} onNavigate={vi.fn()} />

  it('a single choice is a select, not a slider', async () => {
    const set = vi.fn()
    render(panel([RESOLUTION], set))
    expect(screen.queryByRole('slider')).toBeNull()
    expect(screen.queryByRole('switch')).toBeNull()
    await userEvent.click(screen.getByRole('button', { name: 'Resolution' }))
    await userEvent.click(screen.getByRole('option', { name: '1366x768' }))
    expect(set).toHaveBeenCalledWith('Resolution', 0)
  })

  it('the placeholder before the monitor is read is a disabled select', () => {
    render(panel([{ ...RESOLUTION, namedVariants: [{ name: 'Desktop', description: '' }], value: -1, default: -1 }]))
    expect(screen.queryByRole('switch')).toBeNull()
    expect(screen.getByRole('button', { name: 'Resolution' })).toBeDisabled()
  })

  it('new choices at the same value re-render the options', async () => {
    const set = vi.fn()
    const { rerender } = render(panel([RESOLUTION], set))
    const sizes = [{ name: '2560x1440 (16:9)', description: '' }, { name: '1920x1080 (16:9)', description: '' }]
    rerender(panel([{ ...RESOLUTION, namedVariants: sizes, maxValue: 2 }], set))
    await userEvent.click(screen.getByRole('button', { name: 'Resolution' }))
    expect(screen.getAllByRole('option').map((o) => o.textContent)).toEqual(['2560x1440 (16:9)', '1920x1080 (16:9)'])
  })
})

// An engine value below zero means the setting is not known yet (server-owned, e.g. DM
// privacy before the social service answers): rendered, but not editable.
describe('settings panel unset controls', () => {
  it('a select with a negative value is disabled and does not set', async () => {
    const UNSET = base({ name: 'dm_privacy', category: 'chat', namedVariants: [{ name: 'Everyone', description: '' }, { name: 'Only friends', description: '' }], value: -1, default: 0, maxValue: 2 })
    const settings: SettingsState = { ...fakeSession().settings, open: true, list: [UNSET], set: vi.fn() }
    render(<SettingsPanel settings={settings} bindings={fakeSession().bindings} profile={fakeProfileState()} onNavigate={vi.fn()} />)
    // two named choices that are not off/on show their names, not a switch
    expect(screen.queryByRole('switch', { name: 'dm_privacy' })).toBeNull()
    const select = screen.getByRole('button', { name: 'dm_privacy' })
    expect(select).toBeDisabled()
    await userEvent.click(select)
    expect(vi.mocked(settings.set)).not.toHaveBeenCalled()
  })
})
