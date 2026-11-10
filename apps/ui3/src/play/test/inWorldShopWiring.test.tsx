import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useQueryClient } from '@tanstack/react-query'
import { fakeSession } from './harness'
import type { EngineSession } from '../features/session/useEngineSession'

vi.mock('../features/session/useEngineSession', async (original) => ({ ...await original<typeof import('../features/session/useEngineSession')>(), useEngineSession: vi.fn() }))
vi.mock('../features/engine/EngineHost', () => ({ EngineHost: () => null }))
vi.mock('@hud/engine/engineRpc', () => ({ EngineRpc: class {} }))
vi.mock('@hud/lib/isMobile', async (original) => ({ ...await original<typeof import('@hud/lib/isMobile')>(), isMobile: () => false, isChromiumBased: () => true }))
vi.mock('@hud/lib/gpu', () => ({ probeGpu: async () => ({ usable: true, adapter: null }) }))
// The new Shop reads server data, so it needs the HUD's query client: useQueryClient throws without one.
vi.mock('../features/shop/InWorldShop', () => ({
  default: () => {
    useQueryClient()
    return <div data-testid="in-world-shop" />
  }
}))
vi.mock('../features/shop/ShopPage', () => ({ ShopPage: () => <div data-testid="catalogue-shop" /> }))

/** App reads its entry url when it loads, so each case loads it for its own url. */
async function renderApp(search: string, session: EngineSession): Promise<void> {
  window.history.replaceState({}, '', `/${search}`)
  vi.resetModules()
  const { useEngineSession } = await import('../features/session/useEngineSession')
  vi.mocked(useEngineSession).mockImplementation(() => session)
  const { App } = await import('../App')
  render(<App />)
}

describe('when the HUD mounts the Shop page', () => {
  let session: EngineSession
  beforeEach(() => {
    session = fakeSession()
    session.phase = 'world'
    session.shop.open = true
  })
  afterEach(() => {
    window.history.replaceState({}, '', '/')
    vi.restoreAllMocks()
  })

  it('should open the in-world Shop, inside the HUD query client, by default', async () => {
    await renderApp('', session)
    expect(await screen.findByTestId('in-world-shop')).toBeInTheDocument()
    expect(screen.queryByTestId('catalogue-shop')).toBeNull()
  })

  it('should still open it with ?shop=new, as links from before it was the default ask', async () => {
    await renderApp('?shop=new', session)
    expect(await screen.findByTestId('in-world-shop')).toBeInTheDocument()
    expect(screen.queryByTestId('catalogue-shop')).toBeNull()
  })

  it('should keep the catalogue page with ?shop=old, while it is retired', async () => {
    await renderApp('?shop=old', session)
    expect(await screen.findByTestId('catalogue-shop')).toBeInTheDocument()
    expect(screen.queryByTestId('in-world-shop')).toBeNull()
  })
})

describe('when the sidebar offers the Shop', () => {
  afterEach(() => {
    window.history.replaceState({}, '', '/')
    vi.restoreAllMocks()
  })

  it('should open the in-world Shop page by default, not a web tab', async () => {
    window.history.replaceState({}, '', '/')
    vi.resetModules()
    const { Sidebar } = await import('../features/sidebar/Sidebar')
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    const session = fakeSession()
    render(<Sidebar session={session} />)
    await userEvent.click(screen.getByRole('button', { name: 'Shop' }))
    expect(session.shop.toggle).toHaveBeenCalled()
    expect(open).not.toHaveBeenCalled()
  })

  it('should open the web Shop in a new tab, with the web client utm source, with ?shop=old', async () => {
    window.history.replaceState({}, '', '/?shop=old')
    vi.resetModules()
    const { Sidebar } = await import('../features/sidebar/Sidebar')
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    const session = fakeSession()
    render(<Sidebar session={session} />)
    await userEvent.click(screen.getByRole('button', { name: 'Shop' }))
    expect(open).toHaveBeenCalledWith('https://decentraland.org/shop?utm_source=bevy-web-client', '_blank', 'noopener')
    expect(session.shop.toggle).not.toHaveBeenCalled()
  })
})
