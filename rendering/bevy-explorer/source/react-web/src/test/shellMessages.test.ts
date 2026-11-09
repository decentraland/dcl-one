import { describe, expect, it, vi } from 'vitest'
import { onShellMessage } from '../lib/shell'

const fromShell = (data: unknown): MessageEvent => new MessageEvent('message', { data, origin: location.origin, source: window.parent })

describe('onShellMessage', () => {
  it('hands the shell its messages of one type until unsubscribed', () => {
    const handler = vi.fn()
    const unsubscribe = onShellMessage('shop:purchased', handler)
    window.dispatchEvent(fromShell({ type: 'shop:purchased', id: 1 }))
    window.dispatchEvent(fromShell({ type: 'other' }))
    expect(handler).toHaveBeenCalledTimes(1)
    unsubscribe()
    window.dispatchEvent(fromShell({ type: 'shop:purchased', id: 2 }))
    expect(handler).toHaveBeenCalledTimes(1)
  })
})
