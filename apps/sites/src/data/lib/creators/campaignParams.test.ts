import { afterEach, describe, expect, it } from 'vitest'
import { collectCampaignParams, normalizeCampaignParamValue } from './campaignParams'

afterEach(() => window.history.replaceState({}, '', '/'))

describe('collectCampaignParams', () => {
  it('reads the partner params off the current URL, normalized the way sites does', () => {
    window.history.replaceState({}, '', '/?utm_source=SheFi%20Summit&utm_campaign=Q3!!&utm_org=dcl&other=x')
    expect(collectCampaignParams()).toEqual({ utm_source: 'shefi_summit', utm_campaign: 'q3', utm_org: 'dcl' })
  })

  it('leaves out params that are absent or normalize to nothing', () => {
    expect(collectCampaignParams('?utm_medium=%20%21%20&utm_term=')).toEqual({})
    expect(collectCampaignParams('')).toEqual({})
  })
})

describe('normalizeCampaignParamValue', () => {
  it('keeps dashes and underscores, collapses repeats and caps the length', () => {
    expect(normalizeCampaignParamValue('  __Creator--Week__  ')).toBe('creator--week')
    expect(normalizeCampaignParamValue('a'.repeat(300))).toHaveLength(256)
  })
})
