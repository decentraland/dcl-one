// @vitest-environment node
import { beforeEach, describe, expect, it, vi } from 'vitest'

const send = vi.hoisted(() => vi.fn())
vi.mock('@/core/lib/telemetry/track', () => ({ track: send }))

import { errorCode, setTrackContext, track } from './analytics'

beforeEach(() => {
  send.mockReset()
  setTrackContext(null)
})

describe('track', () => {
  it('drops an event reported before a creators page is on screen', () => {
    track('bd_modal_opened', { name: 'publish' })
    expect(send).not.toHaveBeenCalled()
  })

  it("reports under the page's session and story", () => {
    setTrackContext({ sid: 's1', story: 'creator-hub/collection' })
    track('bd_modal_opened', { name: 'publish' })
    expect(send).toHaveBeenCalledWith(
      'bd_modal_opened',
      { name: 'publish' },
      { sid: 's1', story: 'creator-hub/collection' }
    )
  })

  it('rolls a saved item up into the creator funnel', () => {
    setTrackContext({ sid: 's1', story: 'creator-hub/item-editor' })
    track('bd_item_editor_saved', { item_id: 'i1' } as never)
    expect(send.mock.calls.map(([event]) => event)).toEqual(['bd_item_editor_saved', 'creator_item_edited'])
  })
})

describe('errorCode', () => {
  it("buckets a failure by the domain error's reason, a wallet rejection, or unknown", () => {
    expect(errorCode({ reason: 'insufficient_funds' })).toBe('insufficient_funds')
    expect(errorCode({ code: 4001, message: 'User rejected the request.' })).toBe('rejected')
    expect(errorCode(new Error('boom'))).toBe('unknown')
  })
})
