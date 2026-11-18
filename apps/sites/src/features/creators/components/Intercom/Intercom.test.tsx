import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { render, waitFor } from '@testing-library/react'
import type { ReactNode } from 'react'
import { MemoryRouter } from 'react-router-dom'

const wallet = vi.hoisted(() => ({ session: null as { address: string; providerType?: string } | null }))
vi.mock('~/store/wallet', () => ({
  useWallet: (selector: (state: unknown) => unknown) => selector({ session: wallet.session })
}))

const analytics = vi.hoisted(() => ({ anonymousId: undefined as string | undefined }))
vi.mock('~/lib/analytics', () => ({
  getAnonymousId: () => analytics.anonymousId,
  onAnalyticsReady: (cb: () => void) => cb()
}))

const settings = vi.hoisted(() => ({ appId: 'app-id-test' }))
vi.mock('~/config', () => ({
  config: { get: (key: string, fallback = '') => (key === 'INTERCOM_APP_ID' ? settings.appId : fallback) }
}))

const intercom = vi.fn()

/** Loads the widget script the way the real one does: by defining window.Intercom on load. */
function autoLoadScript({ failFirst = false } = {}) {
  let seen = 0
  const observer = new MutationObserver(() => {
    const script = document.head.querySelector<HTMLScriptElement>('script[src*="widget.intercom.io"]:not([data-seen])')
    if (!script) return
    script.dataset.seen = 'true'
    seen += 1
    if (failFirst && seen === 1) {
      script.dispatchEvent(new Event('error'))
      return
    }
    ;(window as unknown as { Intercom: unknown }).Intercom = intercom
    script.dispatchEvent(new Event('load'))
  })
  observer.observe(document.head, { childList: true })
  return observer
}

let observer: MutationObserver

function renderAt(ui: ReactNode, path = '/') {
  return render(ui, { wrapper: ({ children }) => <MemoryRouter initialEntries={[path]}>{children}</MemoryRouter> })
}

beforeEach(() => {
  wallet.session = null
  analytics.anonymousId = undefined
  settings.appId = 'app-id-test'
  document.head.innerHTML = ''
  delete (window as { Intercom?: unknown }).Intercom
  intercom.mockClear()
  observer = autoLoadScript()
})

const setReadyState = (state: DocumentReadyState) =>
  Object.defineProperty(document, 'readyState', { configurable: true, get: () => state })

afterEach(() => {
  setReadyState('complete')
  observer.disconnect()
  vi.resetModules()
})

describe('Intercom', () => {
  it('boots the widget for a visitor who has not signed in', async () => {
    const { Intercom } = await import('./Intercom')
    renderAt(<Intercom />)

    await waitFor(() =>
      expect(intercom).toHaveBeenCalledWith('update', { app_id: 'app-id-test', vertical_padding: 20 })
    )
  })

  it.each(['/collections/editor', '/live-preview'])(
    'lifts the launcher on %s so it clears the bottom controls',
    async path => {
      const { Intercom } = await import('./Intercom')
      renderAt(<Intercom />, path)

      await waitFor(() =>
        expect(intercom).toHaveBeenCalledWith('update', expect.objectContaining({ vertical_padding: 84 }))
      )
    }
  )

  it('tells support who the creator is and which visitor they are in the analytics', async () => {
    wallet.session = { address: '0xCreAtoR', providerType: 'injected' }
    analytics.anonymousId = 'anon-1'

    const { Intercom } = await import('./Intercom')
    renderAt(<Intercom />)

    await waitFor(() =>
      expect(intercom).toHaveBeenCalledWith('update', {
        app_id: 'app-id-test',
        vertical_padding: 20,
        Wallet: '0xcreator',
        'Wallet type': 'injected',
        anon_id: 'anon-1'
      })
    )
  })

  it('tries again after a failed load instead of leaving support unreachable for the visit', async () => {
    observer.disconnect()
    observer = autoLoadScript({ failFirst: true })
    vi.spyOn(console, 'error').mockImplementation(() => undefined)

    const { Intercom } = await import('./Intercom')
    const { rerender } = renderAt(<Intercom />)
    await waitFor(() => expect(console.error).toHaveBeenCalled())
    expect(intercom).not.toHaveBeenCalled()

    // Signing in re-runs the effect, which is the retry.
    wallet.session = { address: '0xCreAtoR' }
    rerender(<Intercom />)

    await waitFor(() =>
      expect(intercom).toHaveBeenCalledWith('update', expect.objectContaining({ Wallet: '0xcreator' }))
    )
  })

  it('waits for the page to finish loading before fetching the widget', async () => {
    setReadyState('loading')
    const { Intercom } = await import('./Intercom')
    renderAt(<Intercom />)

    await new Promise(resolve => setTimeout(resolve, 10))
    expect(document.head.querySelector('script[src*="widget.intercom.io"]')).toBeNull()

    window.dispatchEvent(new Event('load'))
    await waitFor(() =>
      expect(intercom).toHaveBeenCalledWith('update', expect.objectContaining({ app_id: 'app-id-test' }))
    )
  })

  it('stays out of the page entirely when no app id is configured', async () => {
    settings.appId = ''

    const { Intercom } = await import('./Intercom')
    renderAt(<Intercom />)

    // Past the point where the widget would have been fetched.
    await new Promise(resolve => setTimeout(resolve, 10))
    expect(document.head.querySelector('script')).toBeNull()
    expect(intercom).not.toHaveBeenCalled()
  })
})
