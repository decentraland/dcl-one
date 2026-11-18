import { beforeEach, describe, expect, it, vi } from 'vitest'
import { trackPage } from '~/lib/analytics'
import { sendOverviewPage } from '~/lib/overviewSegment'
import { trackPageView } from './pageViews'

vi.mock('~/lib/analytics', () => ({ trackPage: vi.fn() }))
vi.mock('~/lib/overviewSegment', () => ({ sendOverviewPage: vi.fn(), sendOverviewTrack: vi.fn() }))

beforeEach(() => vi.clearAllMocks())

describe('trackPageView', () => {
  it('names the overview view after its full path on the site, as sites did', () => {
    trackPageView('/')
    expect(sendOverviewPage).toHaveBeenCalledWith('/create')
    expect(trackPage).not.toHaveBeenCalled()
  })

  it('sends nothing for a redirect-only path, so an old link is counted once', () => {
    trackPageView('/overview')
    trackPageView('/overview/')
    expect(sendOverviewPage).not.toHaveBeenCalled()
    expect(trackPage).not.toHaveBeenCalled()
  })

  it('groups the app routes under stable names', () => {
    trackPageView('/collections/')
    trackPageView('/collections/0xabc')
    trackPageView('/somewhere')
    expect(vi.mocked(trackPage).mock.calls).toEqual([['collections'], ['collection_detail'], ['other']])
  })
})
