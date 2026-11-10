import { render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const image = 'https://profile-images.decentraland.org/avatar%2Fface?q=1'
const profile = { address: '0x1234', name: 'Guest', isGuest: true, hasClaimedName: false, bodyImage: image }
const params = (base: string): void => {
  vi.stubGlobal('__NATIVE_HUD__', true)
  history.replaceState(null, '', '?' + new URLSearchParams({ native: '1', nativeProfileImageBase: base }))
}

beforeEach(() => vi.resetModules())
afterEach(() => {
  history.replaceState(null, '', '/')
  vi.unstubAllGlobals()
})

describe('native HUD snapshot presentation', () => {
  it.each(['https://gateway.example.org/profile-images', 'https://example.org/mount/profile-images'])(
    'loads friend, passport and stored-login snapshots under %s', async base => {
      vi.stubGlobal('cef', { emit: vi.fn(), listen: vi.fn() })
      params(base)
      vi.stubGlobal('fetch', async () => Response.json({ avatars: [{ avatar: { snapshots: { body: image } } }] }))
      const { Avatar } = await import('@hud/design/Avatar')
      const { fetchStoredProfile } = await import('../features/login/useStoredProfile')
      const { ProfilePassport } = await import('../features/profile/ProfilePassport')
      const expected = base + '/avatar%2Fface?q=1'
      render(
        <>
          <Avatar name="Friend" src={image} />
          <ProfilePassport profile={profile} onClose={vi.fn()} />
        </>
      )
      expect(screen.getByRole('img', { name: 'Friend' })).toHaveAttribute('src', expected)
      expect(screen.getAllByRole('img').some(img => img.getAttribute('src') === expected && img.getAttribute('alt') !== 'Friend')).toBe(true)
      expect((await fetchStoredProfile('default1'))?.body).toBe(expected)
      expect(profile.bodyImage).toBe(image)
    }
  )

  it.each([
    { runtime: 'native', events: 'https://gateway.example.org/events' },
    { runtime: 'native', events: 'https://example.org/mount/events' },
    { runtime: 'web', events: 'https://custom.example/events' }
  ])(
    'fetches Events from the $runtime service at $events', async ({ runtime, events }) => {
      const query = new URLSearchParams({ events })
      if (runtime === 'native') {
        vi.stubGlobal('cef', { emit: vi.fn(), listen: vi.fn() })
        vi.stubGlobal('__NATIVE_HUD__', true)
        query.set('native', '1')
      }
      history.replaceState(null, '', '?' + query)
      const fetcher = vi.fn(async () => Response.json({ ok: true, data: [] }))
      vi.stubGlobal('fetch', fetcher)
      const { fetchEvents } = await import('../features/events/eventsApi')
      await fetchEvents('upcoming')
      expect(fetcher).toHaveBeenCalledWith(events + '/api/events?list=upcoming', { signal: undefined })
    }
  )

  it('uses the base-domain Events service in native without a gateway', async () => {
    vi.stubGlobal('cef', {emit: vi.fn(), listen: vi.fn()})
    vi.stubGlobal('__NATIVE_HUD__', true)
    history.replaceState(null, '', '?native=1&baseDomain=decentraland.zone')
    const fetcher = vi.fn(async () => Response.json({ok:true,data:[]}))
    vi.stubGlobal('fetch', fetcher)
    const { fetchEvents } = await import('../features/events/eventsApi')
    await fetchEvents('live')
    expect(fetcher).toHaveBeenCalledWith('https://events.decentraland.zone/api/events?list=live', {signal:undefined})
  })

  it('routes the friendship notification sender image without modifying its metadata', async () => {
    const base = 'https://gateway.example.org/prefix/profile-images'
    params(base)
    vi.stubGlobal('cef', { emit: vi.fn(), listen: vi.fn() })
    const { NotificationsPanel } = await import('../features/notifications/NotificationsPanel')
    const notification = {
      id: '1', type: 'social_service_friendship_request', read: false, timestamp: '2026-01-01T00:00:00Z',
      metadata: { sender: { name: 'Friend', address: '0x1234', profileImageUrl: image } }
    }
    const { container } = render(<NotificationsPanel notifications={{
      open: true, list: [notification], unread: 1, toggle: vi.fn(), markAllRead: vi.fn()
    }} />)
    expect(container.querySelector('img')).toHaveAttribute('src', base + '/avatar%2Fface?q=1')
    expect(notification.metadata.sender.profileImageUrl).toBe(image)
  })

  it('routes a sale notice face supplied in metadata.image and preserves an external thumbnail', async () => {
    const base = 'https://gateway.example.org/profile-images'
    params(base)
    vi.stubGlobal('cef', { emit: vi.fn(), listen: vi.fn() })
    const { NotificationsPanel } = await import('../features/notifications/NotificationsPanel')
    const sold = { id: 'sale', type: 'item_sold', read: true, timestamp: '2026-01-01T00:00:00Z', metadata: { title: 'Item sold', image } }
    const other = { ...sold, id: 'other', metadata: { thumbnailUrl: 'https://cdn.example.org/promo.png' } }
    const { container } = render(<NotificationsPanel notifications={{
      open: true, list: [sold, other], unread: 0, toggle: vi.fn(), markAllRead: vi.fn()
    }} />)
    const [sale, promo] = container.querySelectorAll('img')
    expect(sale).toHaveAttribute('src', base + '/avatar%2Fface?q=1')
    expect(promo).toHaveAttribute('src', other.metadata.thumbnailUrl)
  })

  it('ignores a browser-supplied image route', async () => {
    params('https://gateway.example.org/profile-images')
    const { Avatar } = await import('@hud/design/Avatar')
    render(<Avatar name="Friend" src={image} />)
    expect(screen.getByRole('img')).toHaveAttribute('src', image)
  })

  it('ignores image routing in a web build even if its embedder provides CEF', async () => {
    params('https://gateway.example.org/profile-images')
    vi.stubGlobal('cef', { emit: vi.fn(), listen: vi.fn() })
    vi.stubGlobal('__NATIVE_HUD__', false)
    const { Avatar } = await import('@hud/design/Avatar')
    render(<Avatar name="Friend" src={image} />)
    expect(screen.getByRole('img')).toHaveAttribute('src', image)
  })

  it('retains external or ambiguous image URLs in CEF', async () => {
    params('https://gateway.example.org/profile-images')
    vi.stubGlobal('cef', { emit: vi.fn(), listen: vi.fn() })
    const { Avatar } = await import('@hud/design/Avatar')
    const { rerender } = render(<Avatar name="Friend" src={image} />)
    for (const raw of [
      'https://cdn.example.org/a',
      'https://profile-images.decentraland.zone/a',
      'https://profile-images.decentraland.org:443/a',
      'https://user@profile-images.decentraland.org/a',
      'https://profile-images.decentraland.org/a%2f..%2f..%2fauth'
    ]) {
      rerender(<Avatar name="Friend" src={raw} />)
      expect(screen.getByRole('img')).toHaveAttribute('src', raw)
    }
  })
})
