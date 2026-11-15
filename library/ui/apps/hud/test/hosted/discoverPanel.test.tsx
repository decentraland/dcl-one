import { describe, it, expect, vi } from 'vitest'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { DiscoverPanel, type DiscoverPanelProps } from '../../features/sidebar/panels/DiscoverPanel'

// The panel the Discover button opens: its three rows, their keys and marks, and what each does.
function panel(over: Partial<DiscoverPanelProps> = {}): DiscoverPanelProps {
  const props: DiscoverPanelProps = { onEvents: vi.fn(), onPlaces: vi.fn(), onCommunities: vi.fn(), onClose: vi.fn(), ...over }
  render(<DiscoverPanel {...props} />)
  return props
}

const rows = (): HTMLElement[] => within(screen.getByRole('dialog', { name: 'Discover' })).getAllByRole('button')

describe('Discover panel', () => {
  it('is the dialog "Discover" under the heading DISCOVER, with Events, Places and Communities in the design\'s order', () => {
    panel()
    expect(screen.getByRole('heading', { name: 'DISCOVER' })).toBeInTheDocument()
    expect(rows().map((r) => r.textContent)).toEqual(['Events', 'Places', 'Communities'])
    for (const name of ['Events', 'Places', 'Communities']) expect(screen.getByRole('button', { name })).toBeInTheDocument()
  })

  it('draws each bound key after its label, outside the row\'s name, and none for an unbound page', () => {
    panel({ eventsKey: 'X', communitiesKey: 'O' })
    expect(rows().map((r) => r.textContent)).toEqual(['Events[X]', 'Places', 'Communities[O]'])
    expect(screen.getByRole('button', { name: 'Events' })).toBeInTheDocument()
    expect(screen.getByText('[X]')).toHaveAttribute('aria-hidden', 'true')
  })

  it.each([
    ['Events', 'onEvents'],
    ['Places', 'onPlaces'],
    ['Communities', 'onCommunities']
  ] as const)('%s runs its action once and closes the panel', async (name, action) => {
    const props = panel()
    await userEvent.click(screen.getByRole('button', { name }))
    expect(props[action]).toHaveBeenCalledTimes(1)
    expect(props.onClose).toHaveBeenCalledTimes(1)
    for (const other of ['onEvents', 'onPlaces', 'onCommunities'] as const) if (other !== action) expect(props[other]).not.toHaveBeenCalled()
  })

  it('Events carries the count of live events, and no count while none is live', () => {
    panel({ liveEvents: 3 })
    expect(screen.getByRole('button', { name: 'Events' })).toHaveTextContent('Events3')
    expect(screen.getByRole('button', { name: 'Places' })).toHaveTextContent(/^Places$/)
  })

  it.each([
    [true, true],
    [false, false]
  ])('Communities carries the live mark when a member community streams (%s), and keeps its name', (communityStreamLive, shown) => {
    panel({ communityStreamLive })
    const mark = within(screen.getByRole('button', { name: 'Communities' })).queryByRole('img', { name: 'A community stream is live' })
    expect(mark != null).toBe(shown)
  })
})
