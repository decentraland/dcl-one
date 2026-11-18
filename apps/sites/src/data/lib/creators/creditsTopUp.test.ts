import { beforeEach, describe, expect, it, vi } from 'vitest'
import { clearTopUpResume, parseTopUpReturn, readTopUpResume, saveTopUpResume, stripTopUpReturn } from './creditsTopUp'

const resume = {
  collectionId: 'c1',
  orderId: 'order-1',
  paymentMethod: 'credits' as const,
  termsAccepted: true,
  email: 'jane.doe@example.com'
}

beforeEach(() => sessionStorage.clear())

describe('the top-up hand-off record', () => {
  it('survives a round trip through session storage', () => {
    saveTopUpResume(resume)
    expect(readTopUpResume()).toEqual(resume)
  })

  it('is gone once cleared, and absent on a fresh visit', () => {
    saveTopUpResume(resume)
    clearTopUpResume()
    expect(readTopUpResume()).toBeNull()
  })

  it('reports a record it could not store, and reads nothing back, when storage refuses it', () => {
    const setItem = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new DOMException('blocked', 'SecurityError')
    })
    const getItem = vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new DOMException('blocked', 'SecurityError')
    })
    expect(saveTopUpResume(resume)).toBe(false)
    expect(readTopUpResume()).toBeNull()
    expect(() => clearTopUpResume()).not.toThrow()
    setItem.mockRestore()
    getItem.mockRestore()
    expect(saveTopUpResume(resume)).toBe(true)
  })

  it('ignores a record it cannot trust', () => {
    sessionStorage.setItem('wemotes-builder.credits-top-up', 'not json')
    expect(readTopUpResume()).toBeNull()
    sessionStorage.setItem('wemotes-builder.credits-top-up', JSON.stringify({ collectionId: 'c1' }))
    expect(readTopUpResume()).toBeNull()
    sessionStorage.setItem(
      'wemotes-builder.credits-top-up',
      JSON.stringify({ ...resume, paymentMethod: 'cash', termsAccepted: 'yes', email: 42 })
    )
    expect(readTopUpResume()).toEqual({ ...resume, paymentMethod: null, termsAccepted: false, email: null })
  })

  it('drops a malformed email so the wizard asks for it again', () => {
    sessionStorage.setItem('wemotes-builder.credits-top-up', JSON.stringify({ ...resume, email: 'jane.doe@' }))
    expect(readTopUpResume()?.email).toBeNull()
  })
})

describe('the Stripe return query', () => {
  it('reads the order and whether the buyer backed out', () => {
    expect(parseTopUpReturn(new URLSearchParams('order=order-1'))).toEqual({ orderId: 'order-1', canceled: false })
    expect(parseTopUpReturn(new URLSearchParams('order=order-1&canceled=1'))).toEqual({
      orderId: 'order-1',
      canceled: true
    })
    expect(parseTopUpReturn(new URLSearchParams('page=2'))).toBeNull()
  })

  it('ignores an order id that does not look like one', () => {
    expect(parseTopUpReturn(new URLSearchParams('order=a%20b%26x'))).toBeNull()
    expect(parseTopUpReturn(new URLSearchParams(`order=${'x'.repeat(129)}`))).toBeNull()
  })

  it('strips only its own params', () => {
    expect(stripTopUpReturn(new URLSearchParams('page=2&order=order-1&canceled=1')).toString()).toBe('page=2')
  })
})
