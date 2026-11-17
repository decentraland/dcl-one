import { describe, it, expect, vi, afterEach } from 'vitest'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { siteUrl } from '@hud-host'
import { bugReportUrl } from '@explorer-core/lib/bugReport'
import { DISCORD_URL, SystemPanel, type SystemPanelProps } from '../../features/sidebar/panels/SystemPanel'

// The panel the System button opens: the settings rows, the rule, the help rows and what each does.
function panel(over: Partial<SystemPanelProps> = {}): SystemPanelProps {
  const props: SystemPanelProps = { onSettings: vi.fn(), onClose: vi.fn(), ...over }
  render(<SystemPanel {...props} />)
  return props
}

const rows = (): (string | null)[] => within(screen.getByRole('dialog', { name: 'System' })).getAllByRole('button').map((r) => r.textContent)

describe('System panel', () => {
  afterEach(() => vi.restoreAllMocks())

  it('is the dialog "System" with no heading: Settings, one rule, then the four help rows', () => {
    panel({ settingsKey: 'P' })
    expect(screen.queryByRole('heading')).toBeNull()
    expect(rows()).toEqual(['Settings[P]', 'FAQ', 'Report Bug', 'Contact Support', 'Discord'])
    expect(screen.getByRole('button', { name: 'Settings' })).toBeInTheDocument()
    expect(screen.getAllByRole('separator')).toHaveLength(1)
  })

  it('draws Mouse / Key Controls and Create after Settings only when the bar gives their actions', () => {
    panel({ onControls: vi.fn(), controlsKey: 'H', onCreate: vi.fn() })
    expect(rows()).toEqual(['Settings', 'Mouse / Key Controls[H]', 'Create', 'FAQ', 'Report Bug', 'Contact Support', 'Discord'])
  })

  it.each([
    ['Settings', 'onSettings'],
    ['Mouse / Key Controls', 'onControls'],
    ['Create', 'onCreate']
  ] as const)('%s runs its action once and closes the panel', async (name, action) => {
    const props = panel({ onControls: vi.fn(), onCreate: vi.fn() })
    await userEvent.click(screen.getByRole('button', { name }))
    expect(props[action]).toHaveBeenCalledTimes(1)
    expect(props.onClose).toHaveBeenCalledTimes(1)
    for (const other of ['onSettings', 'onControls', 'onCreate'] as const) if (other !== action) expect(props[other]).not.toHaveBeenCalled()
  })

  it.each([
    ['FAQ', () => siteUrl('/help/')],
    ['Report Bug', () => bugReportUrl()],
    ['Contact Support', () => siteUrl('/help/')],
    ['Discord', () => DISCORD_URL]
  ])('%s opens its page in a new tab and closes the panel', async (name, url) => {
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    const props = panel()
    await userEvent.click(screen.getByRole('button', { name }))
    expect(open).toHaveBeenCalledTimes(1)
    expect(open).toHaveBeenCalledWith(url(), '_blank', 'noopener')
    expect(props.onClose).toHaveBeenCalledTimes(1)
    expect(props.onSettings).not.toHaveBeenCalled()
  })
})
