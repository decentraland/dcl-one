import { describe, expect, it } from 'vitest'
import { notificationArtwork, notificationMedia } from './notification-media'
import mainnet from '../assets/notifications/ManaMainnet.svg'
const image = 'https://example.org/item.png'
describe('notification media', () => {
  it.each(['events_started', 'events_starts_soon', 'rental_started'] as const)(
    'uses standalone artwork for %s',
    (type) => {
      expect(notificationMedia(type, {}, image)).toEqual({
        image: notificationArtwork[type],
        standalone: true
      })
    }
  )
  it.each(['reward_assignment', 'item_sold', 'referral_new_tier_reached'] as const)(
    'badges the supplied item or tier image for %s',
    (type) => {
      expect(notificationMedia(type, {}, image)).toEqual({
        image,
        badge: notificationArtwork[type]
      })
      expect(notificationMedia(type, {}, null)).toEqual({
        image: notificationArtwork[type],
        standalone: true
      })
    }
  )
  it('uses the network and permission from the notification', () => {
    expect(notificationMedia('royalties_earned', { network: 'ethereum' }, image).badge).toBe(
      mainnet
    )
    expect(
      notificationMedia('worlds_permission_revoked', { permissions: ['access'] }, null).image
    ).toBe(notificationArtwork.worlds_access_restricted)
  })
  it('discards insecure images and accepts unknown types without an artwork', () => {
    expect(notificationMedia('item_sold', {}, 'http://example.org/item').image).toBe(
      notificationArtwork.item_sold
    )
    expect(notificationMedia('unrecognized', {}, null).image).toBeUndefined()
  })
})
