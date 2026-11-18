import { afterEach, describe, expect, it, vi } from 'vitest'
import { ContractName, getContract, getCouponManager } from '~/lib/chain/contracts'

vi.mock('~/lib/auth', async () => {
  const actual = await vi.importActual<typeof import('~/lib/auth')>('~/lib/auth')
  return { ...actual, readContract: vi.fn(), signedFetch: vi.fn() }
})

import { signedFetch } from '~/lib/auth'
import {
  decodeCall,
  recoverTypedDataSigner,
  signTypedDataWithTestKey,
  testAccount,
  toTransaction,
  typedDataDigest
} from '~/test/chain'
import {
  COUPON_TYPES,
  DiscountError,
  DiscountInputError,
  UNCAPPED_USES,
  buildEndDiscountCall,
  collectionsRoot,
  couponTypedValues,
  createDiscount,
  encodeCouponData,
  endDiscount,
  fetchCreatorDiscounts,
  getCouponContracts,
  liveDiscountStatus,
  openDiscountsFor,
  postCoupon,
  storeSignedDiscount,
  validateDiscountTerms,
  type CollectionDiscount,
  type CouponCreation,
  type CreateDiscountDeps
} from './coupons'

const CHAIN_ID = 80002
const NOW = Date.UTC(2026, 9, 16, 12)
const DAY = 24 * 60 * 60 * 1000
const COLLECTION = '0x00000000000000000000000000000000000000CC'
const V3_MANAGER = getCouponManager(ContractName.OffChainMarketplaceV3, CHAIN_ID).address
const V2_MANAGER = getCouponManager(ContractName.OffChainMarketplaceV2, CHAIN_ID).address
const wallet = testAccount
const SALT = '0x' + '22'.repeat(32)
// The chain id as a 32-byte domain salt, Decentraland's EIP-712 convention.
const CHAIN_SALT = '0x' + CHAIN_ID.toString(16).padStart(64, '0')

type CancelledCoupon = {
  signature: string
  checks: Record<string, unknown>
  couponAddress: string
  data: string
}

const fetchMock = vi.fn()
vi.stubGlobal('fetch', fetchMock)
afterEach(() => {
  fetchMock.mockReset()
  vi.mocked(signedFetch).mockReset()
})

function stored(payload: CouponCreation, overrides: Partial<CollectionDiscount> = {}): CollectionDiscount {
  return {
    ...payload,
    id: 'coupon-1',
    couponManager: V3_MANAGER.toLowerCase(),
    root: collectionsRoot(payload.collections),
    createdAt: NOW,
    state: null,
    status: 'active',
    ...overrides
  }
}

function makeDeps(overrides: Partial<CreateDiscountDeps> = {}): CreateDiscountDeps {
  return {
    readIndexes: vi.fn().mockResolvedValue({ contractSignatureIndex: 2, signerSignatureIndex: 5 }),
    signTypedData: vi.fn(signTypedDataWithTestKey),
    postCoupon: vi.fn(payload => Promise.resolve(stored(payload))),
    findStored: vi.fn().mockResolvedValue(undefined),
    onSigned: vi.fn(),
    now: () => NOW,
    randomSalt: () => SALT,
    ...overrides
  }
}

const terms = { collections: [COLLECTION], discountPct: 30, endsAtMs: NOW + 7 * DAY }

describe('when picking the coupon contracts a new discount is signed against', () => {
  // Each marketplace version trusts its own manager; the chain-wide name points at an older one on mainnet.
  it('should use the manager of the marketplace this app lists on', () => {
    expect(getCouponContracts(CHAIN_ID)?.couponManager.address).toBe(V3_MANAGER)
    expect(getCouponContracts(CHAIN_ID)?.coupon).toBe(
      getContract(ContractName.CollectionDiscountCoupon, CHAIN_ID).address
    )
  })

  it('should answer nothing on a chain without coupons', () => {
    expect(getCouponContracts(999_999)).toBeNull()
  })
})

describe('when building the collections root', () => {
  it('should not depend on casing or repeats, so the server rebuilds the same root', () => {
    expect(collectionsRoot([COLLECTION, COLLECTION.toLowerCase()])).toBe(collectionsRoot([COLLECTION.toLowerCase()]))
  })

  it('should refuse an empty set', () => {
    expect(() => collectionsRoot([])).toThrow(DiscountInputError)
  })
})

describe('when validating the terms of a discount', () => {
  it.each([
    ['a discount under the minimum', { discountPct: 4 }, 'pct'],
    ['a discount over the maximum', { discountPct: 71 }, 'pct'],
    ['a fractional discount', { discountPct: 12.5 }, 'pct'],
    ['an end in the past', { endsAtMs: NOW - 1 }, 'window'],
    ['a window longer than 30 days', { endsAtMs: NOW + 31 * DAY }, 'duration'],
    ['a zero purchase limit', { uses: 0 }, 'uses'],
    ['no collection', { collections: [] }, 'collections']
  ])('should refuse %s', (_, patch, problem) => {
    expect(() => validateDiscountTerms({ ...terms, ...patch }, NOW)).toThrow(
      expect.objectContaining({ problem }) as Error
    )
  })

  it('should accept the bounds themselves', () => {
    expect(() => validateDiscountTerms({ ...terms, discountPct: 5, endsAtMs: NOW + 30 * DAY }, NOW)).not.toThrow()
    expect(() => validateDiscountTerms({ ...terms, discountPct: 70, uses: 1 }, NOW)).not.toThrow()
  })
})

describe('when creating a discount', () => {
  it('should store a coupon the creator signed over the collection, against the current manager', async () => {
    const deps = makeDeps()

    const discount = await createDiscount({ ...terms, signer: wallet.address, chainId: CHAIN_ID }, deps)

    const payload = vi.mocked(deps.postCoupon).mock.calls[0][0]
    expect(payload).toMatchObject({
      signer: wallet.address.toLowerCase(),
      chainId: CHAIN_ID,
      collections: [COLLECTION.toLowerCase()],
      discountType: 1,
      discount: 300_000,
      checks: {
        uses: UNCAPPED_USES,
        effective: NOW,
        expiration: NOW + 7 * DAY,
        contractSignatureIndex: 2,
        signerSignatureIndex: 5
      }
    })
    const [domain, , , value] = vi.mocked(deps.signTypedData).mock.calls[0]
    expect(domain.verifyingContract).toBe(V3_MANAGER)
    // The contract reads the window in seconds.
    expect(value).toMatchObject({ checks: { effective: NOW / 1000, expiration: (NOW + 7 * DAY) / 1000 } })
    expect(await recoverTypedDataSigner(domain, COUPON_TYPES, value, payload.signature)).toBe(wallet.address)
    expect(deps.onSigned).toHaveBeenCalledTimes(1)
    expect(discount.id).toBe('coupon-1')
  })

  it('should sign the exact coupon and post it as plain JSON', async () => {
    const deps = makeDeps()
    await createDiscount({ ...terms, signer: wallet.address, chainId: CHAIN_ID }, deps)
    const [domain, types, , value] = vi.mocked(deps.signTypedData).mock.calls[0]
    expect(value.data).toBe(
      '0x0000000000000000000000000000000000000000000000000000000000000001' +
        '00000000000000000000000000000000000000000000000000000000000493e0' +
        'ee692dfecdec95964d7a9b1aead170d6cdd7ecaf4b863082e6b9ee269631ee1c'
    )
    expect(typedDataDigest(domain, types, value)).toBe(
      '0xbf8a27de61bc320a808dd3f6c6d105812d8cc7b8eacfb7227478d409a6508a1e'
    )
    expect(JSON.stringify(vi.mocked(deps.postCoupon).mock.calls[0][0])).toBe(
      JSON.stringify({
        signer: '0x19e7e376e7c213b7e7e7e46cc70a5dd086daff2a',
        chainId: 80002,
        network: 'MATIC',
        checks: {
          uses: 1000000,
          expiration: 1792756800000,
          effective: 1792152000000,
          salt: '0x2222222222222222222222222222222222222222222222222222222222222222',
          contractSignatureIndex: 2,
          signerSignatureIndex: 5,
          allowedRoot: '0x',
          externalChecks: []
        },
        couponAddress: '0x4ee8f6b87f4917a3bbc7c8bb3a06db8555f83db9',
        discountType: 1,
        discount: 300000,
        collections: ['0x00000000000000000000000000000000000000cc'],
        signature:
          '0x59cfee677887f7b0cabd5ba995b6e94e507de9eb4e9ac4e4f3a2985ab50869e556b28a0fcda4c0ad9ea22d3c77c48317a6e440be4aff7fa757a785c68711f9f41b'
      })
    )
  })

  it('should cap the discounted purchases when the creator set a limit', async () => {
    const deps = makeDeps()
    await createDiscount({ ...terms, uses: 25, signer: wallet.address, chainId: CHAIN_ID }, deps)
    expect(vi.mocked(deps.postCoupon).mock.calls[0][0].checks.uses).toBe(25)
  })

  it('should refuse invalid terms before asking for a signature', async () => {
    const deps = makeDeps()
    await expect(
      createDiscount({ ...terms, discountPct: 90, signer: wallet.address, chainId: CHAIN_ID }, deps)
    ).rejects.toBeInstanceOf(DiscountInputError)
    expect(deps.signTypedData).not.toHaveBeenCalled()
  })

  it('should read a dismissed wallet prompt as the creator backing out, storing nothing', async () => {
    const deps = makeDeps({ signTypedData: vi.fn().mockRejectedValue({ code: 4001, message: 'User rejected' }) })
    await expect(createDiscount({ ...terms, signer: wallet.address, chainId: CHAIN_ID }, deps)).rejects.toMatchObject({
      reason: 'rejected'
    })
    expect(deps.postCoupon).not.toHaveBeenCalled()
  })

  // The POST may have been committed with its answer lost: retrying it must never sign a second coupon.
  it('should find the coupon stored when its POST went unanswered', async () => {
    const deps = makeDeps({
      postCoupon: vi.fn().mockRejectedValue(new TypeError('Failed to fetch')),
      findStored: vi.fn(payload => Promise.resolve(stored(payload, { id: 'found' })))
    })
    await expect(createDiscount({ ...terms, signer: wallet.address, chainId: CHAIN_ID }, deps)).resolves.toMatchObject({
      id: 'found'
    })
  })

  it('should keep the signed coupon unconfirmed when the server neither stored nor refused it', async () => {
    const onSignedPayload = vi.fn()
    const deps = makeDeps({ postCoupon: vi.fn().mockRejectedValue(new TypeError('Failed to fetch')), onSignedPayload })
    await expect(createDiscount({ ...terms, signer: wallet.address, chainId: CHAIN_ID }, deps)).rejects.toMatchObject({
      reason: 'unconfirmed'
    })
    expect(onSignedPayload).toHaveBeenCalledWith(vi.mocked(deps.postCoupon).mock.calls[0][0])
  })

  it('should take a refusal as final only once a lookup confirms the coupon is not stored', async () => {
    const deps = makeDeps({ postCoupon: vi.fn().mockRejectedValue(new DiscountError('refused', 'expired')) })
    await expect(createDiscount({ ...terms, signer: wallet.address, chainId: CHAIN_ID }, deps)).rejects.toMatchObject({
      reason: 'refused'
    })
    expect(deps.findStored).toHaveBeenCalled()
  })

  // An earlier POST may have stored it with its answer lost; a later refusal proves nothing without the lookup.
  it('should keep a refused retry unconfirmed when the lookup cannot answer', async () => {
    const postCoupon = vi.fn().mockRejectedValue(new DiscountError('refused', 'invalid'))
    const findStored = vi.fn().mockRejectedValue(new TypeError('Failed to fetch'))
    await expect(
      storeSignedDiscount({ signature: '0xa' } as CouponCreation, { postCoupon, findStored })
    ).rejects.toMatchObject({
      reason: 'unconfirmed'
    })
  })

  it('should find a coupon stored earlier even when its retry is refused', async () => {
    const postCoupon = vi.fn().mockRejectedValue(new DiscountError('refused', 'invalid'))
    const findStored = vi.fn().mockResolvedValue({ id: 'stored-before' })
    await expect(
      storeSignedDiscount({ signature: '0xa' } as CouponCreation, { postCoupon, findStored })
    ).resolves.toEqual({
      id: 'stored-before'
    })
  })

  it('should retry an unconfirmed coupon by storing that same one again, a repost of a stored one included', async () => {
    const first = makeDeps({ postCoupon: vi.fn().mockRejectedValue(new TypeError('Failed to fetch')) })
    await createDiscount({ ...terms, signer: wallet.address, chainId: CHAIN_ID }, first).catch(() => undefined)
    const signed = vi.mocked(first.postCoupon).mock.calls[0][0]
    const postCoupon = vi.fn().mockRejectedValue(new DiscountError('unconfirmed', 'conflict'))
    const findStored = vi.fn(payload => Promise.resolve(stored(payload)))

    await expect(storeSignedDiscount(signed, { postCoupon, findStored })).resolves.toMatchObject({
      signature: signed.signature
    })
    expect(postCoupon).toHaveBeenCalledWith(signed)
  })
})

describe('when storing a signed discount', () => {
  const payload = { signer: '0xaa' } as CouponCreation

  it('should post it as the builder and answer the stored discount', async () => {
    vi.mocked(signedFetch).mockResolvedValue(
      new Response(JSON.stringify({ ok: true, data: { id: 'coupon-1' } }), { status: 201 })
    )
    await expect(postCoupon('0xaa', payload)).resolves.toEqual({ id: 'coupon-1' })
    expect(signedFetch).toHaveBeenCalledWith(
      '0xaa',
      'https://marketplace-api.decentraland.zone',
      '/v1/coupons',
      expect.objectContaining({ method: 'POST', body: JSON.stringify(payload) }),
      { signer: 'dcl:builder', intent: 'dcl:create-coupon' }
    )
  })

  // These are about the request, not the coupon: an earlier POST of it may still have been stored.
  it.each([401, 403, 408, 409, 429, 500])('should leave a %s unconfirmed', async status => {
    vi.mocked(signedFetch).mockResolvedValue(new Response(JSON.stringify({ ok: false }), { status }))
    await expect(postCoupon('0xaa', payload)).rejects.toMatchObject({ reason: 'unconfirmed' })
  })

  it("should fail with the server's message when it refuses", async () => {
    vi.mocked(signedFetch).mockResolvedValue(
      new Response(JSON.stringify({ ok: false, message: 'The signer is not the creator' }), { status: 400 })
    )
    await expect(postCoupon('0xaa', payload)).rejects.toThrow('The signer is not the creator')
  })
})

describe('when listing a creator discounts', () => {
  it('should ask for the lower-cased signer and answer its discounts', async () => {
    fetchMock.mockResolvedValue(new Response(JSON.stringify({ ok: true, data: [{ id: 'c1' }] })))
    await expect(fetchCreatorDiscounts('0xAB')).resolves.toEqual([{ id: 'c1' }])
    expect(new URL(fetchMock.mock.calls[0][0] as string).searchParams.get('signer')).toBe('0xab')
  })

  // The server orders a creator's discounts across all collections, so a running one can sit on a later page.
  it('should read every page, so an older running discount is not lost behind newer ones', async () => {
    const page = (ids: string[]) => new Response(JSON.stringify({ ok: true, data: ids.map(id => ({ id })) }))
    const full = Array.from({ length: 100 }, (_, i) => `newer-${i}`)
    fetchMock.mockResolvedValueOnce(page(full)).mockResolvedValueOnce(page(['older-running']))

    const discounts = await fetchCreatorDiscounts('0xab')

    expect(discounts).toHaveLength(101)
    expect(discounts[100].id).toBe('older-running')
    const offsets = fetchMock.mock.calls.map(([url]) => new URL(url as string).searchParams.get('offset'))
    expect(offsets).toEqual(['0', '100'])
  })

  it('should not list a discount twice when one created mid-walk shifts the pages', async () => {
    const page = (ids: string[]) => new Response(JSON.stringify({ ok: true, data: ids.map(id => ({ id })) }))
    const full = Array.from({ length: 100 }, (_, i) => `c-${i}`)
    fetchMock.mockResolvedValueOnce(page(full)).mockResolvedValueOnce(page(['c-99', 'c-100']))
    const discounts = await fetchCreatorDiscounts('0xab')
    expect(discounts.map(d => d.id).filter(id => id === 'c-99')).toHaveLength(1)
    expect(discounts).toHaveLength(101)
  })

  it('should fail on a server error', async () => {
    fetchMock.mockResolvedValue(new Response('boom', { status: 500 }))
    await expect(fetchCreatorDiscounts('0xab')).rejects.toThrow('500')
  })
})

describe("when reading a discount's status", () => {
  const at = (effective: number, expiration: number, status: CollectionDiscount['status'] = 'active') => ({
    status,
    checks: { effective, expiration } as CollectionDiscount['checks']
  })

  it('should correct a server status the clock has overtaken', () => {
    expect(liveDiscountStatus(at(NOW - DAY, NOW - 1), NOW)).toBe('ended')
    expect(liveDiscountStatus(at(NOW + DAY, NOW + 2 * DAY, 'scheduled'), NOW)).toBe('scheduled')
    expect(liveDiscountStatus(at(NOW - DAY, NOW + DAY, 'scheduled'), NOW)).toBe('active')
  })

  it('should keep a status no clock can undo', () => {
    expect(liveDiscountStatus(at(NOW - DAY, NOW + DAY, 'cancelled'), NOW)).toBe('cancelled')
    expect(liveDiscountStatus(at(NOW - DAY, NOW + DAY, 'exhausted'), NOW)).toBe('exhausted')
  })
})

describe("when picking a collection's open discounts", () => {
  const discount = (
    id: string,
    collections: string[],
    effective: number,
    expiration: number,
    status = 'active',
    discountType = 1
  ) => ({ id, collections, status, discountType, checks: { effective, expiration } }) as unknown as CollectionDiscount

  it('should keep the running and scheduled ones over that collection, soonest-ending first', () => {
    const open = openDiscountsFor(
      [
        discount('later', [COLLECTION.toLowerCase()], NOW - DAY, NOW + 5 * DAY),
        discount('other-collection', ['0xdd'], NOW - DAY, NOW + DAY),
        discount('ended', [COLLECTION.toLowerCase()], NOW - 2 * DAY, NOW - DAY),
        discount('cancelled', [COLLECTION.toLowerCase()], NOW - DAY, NOW + DAY, 'cancelled'),
        discount('scheduled', [COLLECTION.toLowerCase()], NOW + DAY, NOW + 3 * DAY)
      ],
      COLLECTION,
      NOW
    )
    expect(open.map(d => d.id)).toEqual(['scheduled', 'later'])
  })

  // A flat-amount coupon signed elsewhere has no percentage to show or rebuild here.
  it('should leave out discounts that are not a percentage', () => {
    const flat = discount('flat', [COLLECTION.toLowerCase()], NOW - DAY, NOW + DAY, 'active', 2)
    expect(openDiscountsFor([flat], COLLECTION, NOW)).toEqual([])
  })
})

describe('when ending a discount', () => {
  async function signedDiscount(overrides: Partial<CollectionDiscount> = {}) {
    const deps = makeDeps({ postCoupon: vi.fn(payload => Promise.resolve(stored(payload, overrides))) })
    return createDiscount({ ...terms, signer: wallet.address, chainId: CHAIN_ID }, deps)
  }

  // Cancelling on any other manager succeeds on chain and leaves the coupon redeemable.
  it('should cancel it on the manager it was signed against', async () => {
    expect((await buildEndDiscountCall(await signedDiscount())).contract.address).toBe(V3_MANAGER)
    const v3 = await signedDiscount()
    const v2 = getCouponManager(ContractName.OffChainMarketplaceV2, CHAIN_ID)
    const v2Domain = {
      name: v2.name,
      version: v2.version,
      salt: CHAIN_SALT,
      verifyingContract: v2.address
    }
    const older = {
      ...v3,
      couponManager: V2_MANAGER.toLowerCase(),
      signature: await signTypedDataWithTestKey(
        v2Domain,
        COUPON_TYPES,
        'Coupon',
        couponTypedValues(v3.checks, v3.couponAddress, encodeCouponData(v3.discount, v3.root))
      )
    }
    expect((await buildEndDiscountCall(older)).contract.address).toBe(V2_MANAGER)
  })

  // The V3 manager keys cancellations by the coupon's EIP-712 digest: any other struct is a silent no-op.
  it('should cancel the exact coupon the creator signed', async () => {
    const discount = await signedDiscount()
    const call = await buildEndDiscountCall(discount)
    const [[coupon]] = decodeCall(call) as [CancelledCoupon[]]
    const checks = coupon.checks
    const value = {
      checks: {
        uses: checks.uses,
        expiration: checks.expiration,
        effective: checks.effective,
        salt: checks.salt,
        contractSignatureIndex: checks.contractSignatureIndex,
        signerSignatureIndex: checks.signerSignatureIndex,
        allowedRoot: checks.allowedRoot,
        externalChecks: []
      },
      couponAddress: coupon.couponAddress,
      data: coupon.data
    }
    const domain = {
      name: getCouponContracts(CHAIN_ID)!.couponManager.name,
      version: getCouponContracts(CHAIN_ID)!.couponManager.version,
      salt: CHAIN_SALT,
      verifyingContract: V3_MANAGER
    }
    expect(await recoverTypedDataSigner(domain, COUPON_TYPES, value, coupon.signature)).toBe(wallet.address)
  })

  it('should send the exact cancellation transaction', async () => {
    expect(toTransaction(await buildEndDiscountCall(await signedDiscount()))).toEqual({
      to: '0x6c956587d9fe70032781edcdc626310648575382',
      data:
        '0xd41feedf' +
        '0000000000000000000000000000000000000000000000000000000000000020' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000020' +
        '00000000000000000000000000000000000000000000000000000000000000a0' +
        '0000000000000000000000000000000000000000000000000000000000000120' +
        '0000000000000000000000004ee8f6b87f4917a3bbc7c8bb3a06db8555f83db9' +
        '0000000000000000000000000000000000000000000000000000000000000280' +
        '0000000000000000000000000000000000000000000000000000000000000300' +
        '0000000000000000000000000000000000000000000000000000000000000041' +
        '59cfee677887f7b0cabd5ba995b6e94e507de9eb4e9ac4e4f3a2985ab50869e5' +
        '56b28a0fcda4c0ad9ea22d3c77c48317a6e440be4aff7fa757a785c68711f9f4' +
        '1b00000000000000000000000000000000000000000000000000000000000000' +
        '00000000000000000000000000000000000000000000000000000000000f4240' +
        '000000000000000000000000000000000000000000000000000000006adb4c40' +
        '000000000000000000000000000000000000000000000000000000006ad211c0' +
        '2222222222222222222222222222222222222222222222222222222222222222' +
        '0000000000000000000000000000000000000000000000000000000000000002' +
        '0000000000000000000000000000000000000000000000000000000000000005' +
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000120' +
        '0000000000000000000000000000000000000000000000000000000000000140' +
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000060' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '00000000000000000000000000000000000000000000000000000000000493e0' +
        'ee692dfecdec95964d7a9b1aead170d6cdd7ecaf4b863082e6b9ee269631ee1c' +
        '0000000000000000000000000000000000000000000000000000000000000000'
    })
  })

  // The manager records any digest it is handed, so a payload that is not the signed coupon ends nothing.
  it('should refuse to send a cancellation that does not rebuild to the signed coupon', async () => {
    const tampered = { ...(await signedDiscount()), discount: 500_000 }
    await expect(buildEndDiscountCall(tampered)).rejects.toThrow('does not rebuild')
  })

  it('should refuse to send anything to a manager this build does not know', async () => {
    const discount = await signedDiscount({ couponManager: '0x00000000000000000000000000000000000000ee' })
    await expect(buildEndDiscountCall(discount)).rejects.toThrow('Unknown coupon manager')
  })

  it('should report the wallet prompt and fail when the transaction reverts', async () => {
    const discount = await signedDiscount()
    const onSigned = vi.fn()
    await expect(
      endDiscount(discount, {
        sendTransaction: vi.fn().mockResolvedValue({ hash: '0xhash' }),
        waitForTransaction: vi.fn().mockResolvedValue(false),
        onSigned
      })
    ).rejects.toMatchObject({ reason: 'generic' })
    expect(onSigned).toHaveBeenCalledTimes(1)
  })

  // It may still mine, so it must not read as a failure the creator retries (and pays for) again.
  it('should report a cancellation that is taking too long as still pending', async () => {
    const discount = await signedDiscount()
    await expect(
      endDiscount(discount, {
        sendTransaction: vi.fn().mockResolvedValue({ hash: '0xhash' }),
        waitForTransaction: vi.fn().mockRejectedValue(Object.assign(new Error('timeout exceeded'), { code: 'TIMEOUT' }))
      })
    ).rejects.toMatchObject({ reason: 'pending' })
  })

  it.each([
    ['an RPC error while polling for the receipt', { waitForTransaction: vi.fn().mockRejectedValue(new Error('429')) }],
    [
      'a send that fails after the wallet returned the hash',
      {
        sendTransaction: vi.fn(async (_call: unknown, onSent: (hash: string) => void) => {
          onSent('0xhash')
          throw new Error('nonce read failed')
        }),
        waitForTransaction: vi.fn()
      }
    ]
  ])('should treat %s as still pending, never as a failure to retry', async (_, deps) => {
    const discount = await signedDiscount()
    const onBroadcast = vi.fn()
    await expect(
      endDiscount(
        discount,
        Object.assign(
          { sendTransaction: vi.fn().mockResolvedValue({ hash: '0xhash' }), waitForTransaction: vi.fn(), onBroadcast },
          deps
        )
      )
    ).rejects.toMatchObject({ reason: 'pending' })
    expect(onBroadcast.mock.calls.map(([hash]) => hash as string)).toContain('0xhash')
  })

  it('should record the cancellation and leave the wallet step as soon as the hash is back, then record the nonce', async () => {
    const discount = await signedDiscount()
    const onBroadcast = vi.fn()
    const onSigned = vi.fn()
    let recordedWhileSending: unknown[][] = []
    let signedWhileSending = 0
    await endDiscount(discount, {
      sendTransaction: async (_call, onSent) => {
        onSent('0xhash')
        recordedWhileSending = [...onBroadcast.mock.calls]
        signedWhileSending = onSigned.mock.calls.length
        return { hash: '0xhash', nonce: 7 }
      },
      waitForTransaction: vi.fn().mockResolvedValue(true),
      onBroadcast,
      onSigned
    })
    expect(recordedWhileSending).toEqual([['0xhash']])
    expect(signedWhileSending).toBe(1)
    expect(onSigned).toHaveBeenCalledTimes(1)
    expect(onBroadcast).toHaveBeenLastCalledWith('0xhash', 7)
  })

  it('should read a dismissed wallet prompt as the creator backing out', async () => {
    const discount = await signedDiscount()
    await expect(
      endDiscount(discount, {
        sendTransaction: vi.fn().mockRejectedValue({ code: 4001 }),
        waitForTransaction: vi.fn()
      })
    ).rejects.toMatchObject({ reason: 'rejected' })
  })
})
