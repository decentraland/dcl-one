import { afterEach, describe, expect, it, vi } from 'vitest'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { siteUrl } from '@hud-host'
import { ShopPanel } from '../features/sidebar/panels/ShopPanel'
import { Sidebar } from '../features/sidebar/Sidebar'
import { fakeSession, renderSettled } from './harness'

// The bar's cases below are for a host with no in-world Shop. The Play host has one, and this file
// runs in its project too, so the switch is set here and not read from the host.
vi.mock('@hud-host', async (original) => ({ ...(await original<object>()), IN_WORLD_SHOP: false }))

afterEach(() => {
  vi.restoreAllMocks()
  history.replaceState(null, '', location.pathname)
})

// DOMAIN: the navigation bar's SHOP panel — the Credits balance, BUY CREDITS and the section banners.
describe('SHOP panel', () => {
  it('is the dialog "Shop" with its title, its text and BUY CREDITS', () => {
    render(<ShopPanel />)
    const panel = screen.getByRole('dialog', { name: 'Shop' })
    expect(within(panel).getByRole('heading', { level: 2, name: 'SHOP' })).toBeInTheDocument()
    expect(within(panel).getByText('Shop your favorite looks and moves to show off your unique style!')).toBeInTheDocument()
    expect(within(panel).getByRole('button', { name: 'BUY CREDITS' })).toBeEnabled()
  })

  it('shows the coin and the amount only once the balance is known', () => {
    const view = render(<ShopPanel />)
    expect(screen.queryByRole('img', { name: 'Credits' })).toBeNull()
    expect(screen.getByRole('dialog', { name: 'Shop' })).not.toHaveTextContent(/\d/)

    view.rerender(<ShopPanel credits={1250} />)
    expect(screen.getByRole('img', { name: 'Credits' })).toBeInTheDocument()
    // The amount is written as the design writes it: no thousands separator.
    expect(screen.getByText('1250')).toBeInTheDocument()

    // A known balance of nothing is still a balance.
    view.rerender(<ShopPanel credits={0} />)
    expect(screen.getByRole('img', { name: 'Credits' })).toBeInTheDocument()
    expect(screen.getByText('0')).toBeInTheDocument()
  })

  it('BUY CREDITS opens the credits page in a new tab and closes the panel', async () => {
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    const onClose = vi.fn()
    render(<ShopPanel credits={3} onClose={onClose} />)
    await userEvent.click(screen.getByRole('button', { name: 'BUY CREDITS' }))
    expect(open).toHaveBeenCalledTimes(1)
    const [url, target, features] = open.mock.calls[0]!
    expect(String(url).startsWith(`${siteUrl('/shop/credits')}?utm_source=`)).toBe(true)
    expect(target).toBe('_blank')
    expect(features).toBe('noopener')
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('draws no banner for a section the caller cannot open', () => {
    const view = render(<ShopPanel />)
    expect(screen.queryByRole('heading', { level: 3 })).toBeNull()
    expect(screen.queryByRole('button', { name: /EXPLORE/ })).toBeNull()

    view.rerender(<ShopPanel explore={{ wearables: () => undefined }} />)
    // The title is two lines: its name carries the line break as white space.
    expect(screen.getByRole('heading', { level: 3, name: /^FIND\s+YOUR LOOK$/ })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'EXPLORE WEARABLES' })).toBeInTheDocument()
    expect(screen.queryByRole('heading', { level: 3, name: /^EXPRESS\s+WITH STYLE$/ })).toBeNull()
    expect(screen.queryByRole('button', { name: 'EXPLORE EMOTES' })).toBeNull()
  })

  it('shows the emotes banner before the wearables one, each with its button', () => {
    render(<ShopPanel explore={{ wearables: () => undefined, emotes: () => undefined }} />)
    expect(screen.getAllByRole('heading', { level: 3 }).map((h) => h.textContent)).toEqual(['EXPRESS\nWITH STYLE', 'FIND\nYOUR LOOK'])
    expect(screen.getAllByRole('button').map((b) => b.textContent)).toEqual(['BUY CREDITS', 'EXPLORE EMOTES', 'EXPLORE WEARABLES'])
    // The banner pictures are decoration: no accessible picture but the coin.
    expect(screen.queryAllByRole('img')).toHaveLength(0)
  })

  it("a banner's button opens its own section and closes the panel", async () => {
    const emotes = vi.fn()
    const wearables = vi.fn()
    const onClose = vi.fn()
    render(<ShopPanel explore={{ emotes, wearables }} onClose={onClose} />)
    await userEvent.click(screen.getByRole('button', { name: 'EXPLORE EMOTES' }))
    expect(emotes).toHaveBeenCalledTimes(1)
    expect(wearables).not.toHaveBeenCalled()
    expect(onClose).toHaveBeenCalledTimes(1)
    await userEvent.click(screen.getByRole('button', { name: 'EXPLORE WEARABLES' }))
    expect(wearables).toHaveBeenCalledTimes(1)
    expect(emotes).toHaveBeenCalledTimes(1)
    expect(onClose).toHaveBeenCalledTimes(2)
  })
})

// A host with no in-world Shop: its bar opens the web Shop. The mock HUD has no
// engine, and shows the panel all the same so it can be seen beside the bar.
describe('SHOP panel from the bar of the mock HUD', () => {
  it('?mock=1 opens the panel in place of the web Shop, with the Shop button pressed', async () => {
    history.replaceState(null, '', '?mock=1')
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    await renderSettled(<Sidebar session={fakeSession()} />)
    const shop = screen.getByRole('button', { name: 'Shop' })
    expect(shop).toHaveAttribute('aria-pressed', 'false')
    await userEvent.click(shop)
    expect(screen.getByRole('dialog', { name: 'Shop' })).toBeInTheDocument()
    expect(shop).toHaveAttribute('aria-pressed', 'true')
    expect(open).not.toHaveBeenCalled()
  })

  it.each(['', '?mock=0', '?mockhold=welcome'])('without the mock switch (%s) the Shop button opens the web Shop and no panel', async (search) => {
    history.replaceState(null, '', search === '' ? location.pathname : search)
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    await renderSettled(<Sidebar session={fakeSession()} />)
    await userEvent.click(screen.getByRole('button', { name: 'Shop' }))
    expect(open).toHaveBeenCalledTimes(1)
    expect(String(open.mock.calls[0]![0]).startsWith(`${siteUrl('/shop')}?utm_source=`)).toBe(true)
    expect(screen.queryByRole('dialog', { name: 'Shop' })).toBeNull()
  })
})
