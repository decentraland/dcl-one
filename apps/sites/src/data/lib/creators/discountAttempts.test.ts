import { afterEach, describe, expect, it, vi } from 'vitest'
import { Network } from '@dcl/schemas'
import { type CouponCreation } from '~/lib/coupons'
import {
  acknowledgeUnconfirmedCreate,
  clearPendingEnd,
  clearUnconfirmedCreate,
  pendingEndOf,
  resetDiscountAttempts,
  savePendingEnd,
  saveUnconfirmedCreate,
  unconfirmedCreateOf
} from './discountAttempts'

const ADDRESS = '0x00000000000000000000000000000000000000AA'
const NOW = Date.UTC(2026, 9, 16)
const end = { discountId: 'd1', txHash: '0xhash', chainId: 80002, sentAt: NOW, expiresAt: Date.now() + 86_400_000 }
const payloadEnding = (expiration: number, signature = '0xsigned') =>
  ({
    signer: ADDRESS,
    signature,
    chainId: 80002,
    discount: 300_000,
    collections: ['0xcc'],
    checks: { expiration }
  }) as unknown as CouponCreation

afterEach(() => {
  resetDiscountAttempts()
  vi.restoreAllMocks()
})

describe('when recording a cancellation that was sent', () => {
  // A reload must not forget it, or End would be offered, and paid for, again.
  it('should still have it after a reload', async () => {
    savePendingEnd(ADDRESS, end)
    vi.resetModules()
    const reloaded = await import('./discountAttempts')
    expect(reloaded.pendingEndOf(ADDRESS.toLowerCase(), 'd1')).toEqual(end)
  })

  it('should forget it once it is settled', () => {
    savePendingEnd(ADDRESS, end)
    clearPendingEnd(ADDRESS, 'd1')
    expect(pendingEndOf(ADDRESS, 'd1')).toBeUndefined()
  })

  it('should still hold it for this tab when the browser keeps no site data', () => {
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    savePendingEnd(ADDRESS, end)
    expect(pendingEndOf(ADDRESS, 'd1')).toEqual(end)
  })

  it('should forget a cancellation once its discount has ended anyway', () => {
    savePendingEnd(ADDRESS, { ...end, expiresAt: Date.now() - 1 })
    expect(pendingEndOf(ADDRESS, 'd1')).toBeUndefined()
  })
})

describe('when recording a coupon whose storing is unconfirmed', () => {
  it('should hand it back for a retry while its window is open', () => {
    const create = { collectionId: 'c1', payload: payloadEnding(NOW + 1000) }
    saveUnconfirmedCreate(ADDRESS, create)
    expect(unconfirmedCreateOf(ADDRESS, 'c1', NOW)).toEqual(create)
  })

  // Past its window it can apply nowhere, so there is nothing left to settle.
  it('should drop it once its window closed', () => {
    saveUnconfirmedCreate(ADDRESS, { collectionId: 'c1', payload: payloadEnding(NOW) })
    expect(unconfirmedCreateOf(ADDRESS, 'c1', NOW)).toBeUndefined()
    expect(unconfirmedCreateOf(ADDRESS, 'c1', NOW - 5000)).toBeUndefined()
  })
})

describe('when more than one signed coupon is unconfirmed for a collection', () => {
  // Each one may be on sale: recording a second must never forget the first.
  it('should keep both and forget only the one settled', () => {
    saveUnconfirmedCreate(ADDRESS, { collectionId: 'c1', payload: payloadEnding(NOW + 1000, '0xa') })
    saveUnconfirmedCreate(ADDRESS, { collectionId: 'c1', payload: payloadEnding(NOW + 1000, '0xb') })
    clearUnconfirmedCreate(ADDRESS, '0xa')
    expect(unconfirmedCreateOf(ADDRESS, 'c1', NOW)?.payload.signature).toBe('0xb')
  })
})

describe('when the stored attempts are not what this build writes', () => {
  it('should drop the malformed entries instead of trusting them', () => {
    localStorage.setItem(
      `collection-discount-attempts:${ADDRESS.toLowerCase()}`,
      JSON.stringify({
        ends: [{ discountId: 'd1' }],
        creates: [
          { collectionId: 'c1', payload: {} },
          { collectionId: 'c1', payload: { ...payloadEnding(NOW + 1000), collections: [42] } }
        ]
      })
    )
    expect(pendingEndOf(ADDRESS, 'd1')).toBeUndefined()
    expect(unconfirmedCreateOf(ADDRESS, 'c1', NOW)).toBeUndefined()
  })
})

describe('when the browser reads storage but cannot write it (a full quota)', () => {
  const failWrites = () =>
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new DOMException('full', 'QuotaExceededError')
    })
  const create = { collectionId: 'c1', payload: payloadEnding(Date.now() + 86_400_000, '0xnew') }

  it('should still hold a new cancellation and coupon for this tab when nothing was stored before', () => {
    failWrites()
    savePendingEnd(ADDRESS, end)
    saveUnconfirmedCreate(ADDRESS, create)
    expect(pendingEndOf(ADDRESS, 'd1')).toEqual(end)
    expect(unconfirmedCreateOf(ADDRESS, 'c1')).toEqual(create)
  })

  // Reading the older stored copy back would drop the guard the write just tried to add.
  it('should prefer this tab copy over an older stored one', () => {
    localStorage.setItem(
      `collection-discount-attempts:${ADDRESS.toLowerCase()}`,
      JSON.stringify({ ends: [], creates: [] })
    )
    failWrites()
    savePendingEnd(ADDRESS, end)
    saveUnconfirmedCreate(ADDRESS, create)
    expect(pendingEndOf(ADDRESS, 'd1')).toEqual(end)
    expect(unconfirmedCreateOf(ADDRESS, 'c1')).toEqual(create)
  })
})

describe('when the creator chooses to sign a new coupon next to an unconfirmed one', () => {
  it('should stop blocking the form but keep the first on record', () => {
    const first = { collectionId: 'c1', payload: payloadEnding(Date.now() + 86_400_000, '0xfirst') }
    saveUnconfirmedCreate(ADDRESS, first)
    acknowledgeUnconfirmedCreate(ADDRESS, '0xfirst')
    expect(unconfirmedCreateOf(ADDRESS, 'c1')).toBeUndefined()
    const stored = JSON.parse(localStorage.getItem(`collection-discount-attempts:${ADDRESS.toLowerCase()}`)!)
    expect(stored.creates).toEqual([{ ...first, acknowledged: true }])
  })
})

describe('when the attempts are written to storage', () => {
  // A later build reads these back after a reload: their encoding must not drift.
  it('should store them as plain JSON', () => {
    const payload: CouponCreation = {
      signer: ADDRESS.toLowerCase(),
      chainId: 80002,
      network: Network.MATIC,
      checks: {
        uses: 1_000_000,
        expiration: NOW + 7 * 86_400_000,
        effective: NOW,
        salt: '0x' + '22'.repeat(32),
        contractSignatureIndex: 2,
        signerSignatureIndex: 5,
        allowedRoot: '0x',
        externalChecks: []
      },
      couponAddress: '0x00000000000000000000000000000000000000dd',
      discountType: 1,
      discount: 300_000,
      collections: ['0x00000000000000000000000000000000000000cc'],
      signature: '0x' + 'ab'.repeat(65)
    }
    savePendingEnd(ADDRESS, { ...end, nonce: 4, expiresAt: NOW + 86_400_000 })
    saveUnconfirmedCreate(ADDRESS, { collectionId: 'c1', payload })
    expect(localStorage.getItem(`collection-discount-attempts:${ADDRESS.toLowerCase()}`)).toBe(
      JSON.stringify({
        ends: [
          {
            discountId: 'd1',
            txHash: '0xhash',
            chainId: 80002,
            sentAt: 1792108800000,
            expiresAt: 1792195200000,
            nonce: 4
          }
        ],
        creates: [
          {
            collectionId: 'c1',
            payload: {
              signer: '0x00000000000000000000000000000000000000aa',
              chainId: 80002,
              network: 'MATIC',
              checks: {
                uses: 1000000,
                expiration: 1792713600000,
                effective: 1792108800000,
                salt: '0x2222222222222222222222222222222222222222222222222222222222222222',
                contractSignatureIndex: 2,
                signerSignatureIndex: 5,
                allowedRoot: '0x',
                externalChecks: []
              },
              couponAddress: '0x00000000000000000000000000000000000000dd',
              discountType: 1,
              discount: 300000,
              collections: ['0x00000000000000000000000000000000000000cc'],
              signature:
                '0xababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababab'
            }
          }
        ]
      })
    )
  })
})
