import { describe, expect, it } from 'vitest'
import { notificationArtwork, notificationMedia } from './notification-media'
import mainnet from '../assets/notifications/ManaMainnet.svg'

const image = 'https://example.org/item.png'

describe('notificationMedia', () => {
  describe('when the type has standalone artwork', () => {
    it.each(['events_started', 'events_starts_soon', 'rental_started'] as const)(
      'should use the standalone artwork for %s',
      (type) => {
        expect(notificationMedia(type, {}, image)).toEqual({
          image: notificationArtwork[type],
          standalone: true
        })
      }
    )
  })

  describe('when the type badges an item or tier image', () => {
    describe('and an image is supplied', () => {
      it.each(['reward_assignment', 'item_sold', 'referral_new_tier_reached'] as const)(
        'should badge the supplied image for %s',
        (type) => {
          expect(notificationMedia(type, {}, image)).toEqual({
            image,
            badge: notificationArtwork[type]
          })
        }
      )
    })

    describe('and no image is supplied', () => {
      it.each(['reward_assignment', 'item_sold', 'referral_new_tier_reached'] as const)(
        'should fall back to the standalone artwork for %s',
        (type) => {
          expect(notificationMedia(type, {}, null)).toEqual({
            image: notificationArtwork[type],
            standalone: true
          })
        }
      )
    })

    describe('and the supplied image is insecure', () => {
      it('should discard it and use the artwork', () => {
        expect(notificationMedia('item_sold', {}, 'http://example.org/item').image).toBe(
          notificationArtwork.item_sold
        )
      })
    })
  })

  describe('when the notification carries a network', () => {
    it('should badge the network artwork', () => {
      expect(notificationMedia('royalties_earned', { network: 'ethereum' }, image).badge).toBe(
        mainnet
      )
    })
  })

  describe('when the notification carries a revoked permission', () => {
    it('should use the artwork for that permission', () => {
      expect(
        notificationMedia('worlds_permission_revoked', { permissions: ['access'] }, null).image
      ).toBe(notificationArtwork.worlds_access_restricted)
    })
  })

  describe('when the type is unknown', () => {
    it('should accept it without an artwork', () => {
      expect(notificationMedia('unrecognized', {}, null).image).toBeUndefined()
    })
  })
})
