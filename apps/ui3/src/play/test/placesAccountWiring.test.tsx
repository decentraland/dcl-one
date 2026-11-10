import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { fakeSession } from '@hud/test/harness'
import { useEngineSession, type EngineSession } from '@hud/features/session/useEngineSession'

vi.mock('@hud/features/session/useEngineSession', async (original) => ({ ...await original<typeof import('@hud/features/session/useEngineSession')>(), useEngineSession: vi.fn() }))
vi.mock('@hud/features/engine/EngineHost', () => ({ EngineHost: () => null }))
vi.mock('@explorer-core/engine/engineRpc', () => ({ EngineRpc: class {} }))
vi.mock('@explorer-core/lib/isMobile', async (original) => ({ ...await original<typeof import('@explorer-core/lib/isMobile')>(), isMobile: () => false, isChromiumBased: () => true }))
vi.mock('@explorer-core/lib/gpu', () => ({ probeGpu: async () => ({ usable: true, adapter: null }) }))
vi.mock('@hud/features/places/PlacesPicker', () => ({ PlacesPicker: ({ account }: { account?: string }) => <div data-testid="picker-account">{account ?? 'pending'}</div> }))
vi.mock('@hud/features/places/PlacesPage', () => ({ PlacesPage: ({ account }: { account?: string }) => <div data-testid="places-account">{account ?? 'pending'}</div> }))
import { App } from '../App'

describe('when App passes the active player into discovery', () => {
  let session: EngineSession
  beforeEach(() => {
    session = fakeSession()
    session.phase = 'picking'
    session.login.account = '0xremembered'
    vi.mocked(useEngineSession).mockImplementation(() => session)
  })
  afterEach(() => vi.restoreAllMocks())
  it('should leave identity discovery available before the first engine profile arrives', async () => {
    render(<App />)
    expect(await screen.findByTestId('picker-account')).toHaveTextContent('pending')
  })
  it('should pass the current engine profile instead of the remembered SSO wallet', async () => {
    session.profile.data = { address: '0xactive', name: 'Active', hasClaimedName: false, isGuest: false }
    render(<App />)
    expect(await screen.findByTestId('picker-account')).toHaveTextContent('0xactive')
  })
  it('should use the same active identity for the in-world Places panel', async () => {
    session.phase = 'world'
    session.places.open = true
    session.profile.data = { address: '0xactive', name: 'Active', hasClaimedName: false, isGuest: false }
    render(<App />)
    expect(await screen.findByTestId('places-account')).toHaveTextContent('0xactive')
  })
})
