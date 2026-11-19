// The navigation bar's SHOP panel on /play: BUY CREDITS opens the same funding page as the in-world checkout's, the
// site whose node holds the balance it reads, even when the Explorer runs against another deployment (?baseDomain=).
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

/** The panel with the /play host loaded on a page Sites serves at `search`; BUY CREDITS pressed. */
async function buyCreditsOnSites(search: string) {
  vi.resetModules()
  window.__DCL_PUBLIC__ = {}
  window.history.replaceState(null, '', search)
  const { ShopPanel } = await import('@hud/features/sidebar/panels/ShopPanel')
  const open = vi.spyOn(window, 'open').mockReturnValue(null)
  const onClose = vi.fn()
  render(<ShopPanel credits={3} onClose={onClose} />)
  await userEvent.click(screen.getByRole('button', { name: 'BUY CREDITS' }))
  expect(open).toHaveBeenCalledTimes(1)
  const [url, target, features] = open.mock.calls[0]!
  expect(target).toBe('_blank')
  expect(features).toBe('noopener')
  expect(onClose).toHaveBeenCalledTimes(1)
  const opened = new URL(String(url))
  expect(opened.searchParams.get('utm_source')).not.toBeNull()
  return opened
}

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  delete window.__DCL_PUBLIC__
  window.history.replaceState(null, '', '/')
})

describe('BUY CREDITS in the SHOP panel on /play', () => {
  it('opens this site’s Credits page while the Explorer runs against another deployment', async () => {
    const opened = await buyCreditsOnSites('/?baseDomain=decentraland.org')
    expect(`${opened.origin}${opened.pathname}`).toBe(`${window.location.origin}/shop/credits`)
  })

  it('opens this site’s Credits page with no other deployment selected', async () => {
    const opened = await buyCreditsOnSites('/')
    expect(`${opened.origin}${opened.pathname}`).toBe(`${window.location.origin}/shop/credits`)
  })
})
