// The event panel's "remind me" (features/map/panel/EventInfo): shown at once, and put back to
// what the services last held when they do not take it.
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { MapEvent, MapPlace } from '@explorer-core/features/map/mapApi'

const setEventAttending = vi.hoisted(() => vi.fn())
vi.mock('@explorer-core/features/map/mapApi', async (importOriginal) => ({ ...(await importOriginal<object>()), setEventAttending }))

import { EventInfo } from '../features/map/panel/EventInfo'

const PLACE: MapPlace = { id: 'plaza', title: 'Genesis Plaza', positions: ['0,0'], base_position: '0,0' }
const EVENT: MapEvent = { id: 'e', name: 'Party', x: 0, y: 0, start_at: '2099-01-01T10:00:00.000Z', attending: false, total_attendees: 4 }

function Panel(): React.JSX.Element {
  const [event, setEvent] = useState(EVENT)
  return <EventInfo view={{ event, place: PLACE, visit: 1 }} referrer={undefined} onEventChange={setEvent} onJumpIn={() => {}} />
}

const settle = () => act(async () => {})
const remind = (): HTMLElement => screen.getByRole('button', { name: /remind me/i })

describe('map event panel: remind me', () => {
  beforeEach(() => {
    setEventAttending.mockReset()
  })

  it('goes back to what the services held when two toggles in a row both fail', async () => {
    const fails: Array<() => void> = []
    setEventAttending.mockImplementation(() => new Promise<void>((_, reject) => fails.push(() => reject(new Error('offline')))))
    render(<Panel />)
    await userEvent.click(remind())
    await userEvent.click(remind())
    expect(remind().getAttribute('aria-pressed')).toBe('false')
    fails.forEach((fail) => fail())
    await settle()
    expect(remind().getAttribute('aria-pressed')).toBe('false')
    expect(screen.getByText('4')).toBeTruthy()
  })

  it('goes back to the last toggle the services took when a later one fails', async () => {
    let fail!: () => void
    setEventAttending.mockResolvedValueOnce(undefined).mockImplementationOnce(() => new Promise<void>((_, reject) => (fail = () => reject(new Error('offline')))))
    render(<Panel />)
    await userEvent.click(remind())
    await settle()
    expect(remind().getAttribute('aria-pressed')).toBe('true')
    await userEvent.click(remind())
    expect(remind().getAttribute('aria-pressed')).toBe('false')
    fail()
    await settle()
    expect(remind().getAttribute('aria-pressed')).toBe('true')
    expect(screen.getByText('5')).toBeTruthy()
  })
})
