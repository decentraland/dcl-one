import { afterEach, describe, expect, it, vi } from 'vitest'
import { fetchCampaign } from './campaign'

const fetchMock = vi.fn()
vi.stubGlobal('fetch', fetchMock)
afterEach(() => fetchMock.mockReset())

const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status })
const link = (id: string) => ({ sys: { type: 'Link', linkType: 'Entry', id } })

function entry(id: string, contentType: string, fields: Record<string, unknown>) {
  return { sys: { id, contentType: { sys: { id: contentType } } }, fields }
}

describe('fetchCampaign', () => {
  it('follows the admin entry to the campaign it links and returns its name and tag', async () => {
    fetchMock.mockImplementation((url: string) =>
      Promise.resolve(
        url.includes('/entries/c1')
          ? json(entry('c1', 'marketingCampaign', { name: 'Pride', mainTag: 'pride2026' }))
          : url.includes('/entries/b1')
            ? json(entry('b1', 'banner', { title: 'Hero' }))
            : json({ sys: { id: 'admin' }, fields: { banner: link('b1'), campaign: link('c1') } })
      )
    )
    await expect(fetchCampaign()).resolves.toEqual({ name: 'Pride', mainTag: 'pride2026' })
  })

  it('is null when nothing is running, the entry is incomplete or the CMS fails', async () => {
    fetchMock.mockResolvedValueOnce(json({ sys: { id: 'admin' }, fields: { banner: link('b1') } }))
    fetchMock.mockResolvedValueOnce(json(entry('b1', 'banner', {})))
    await expect(fetchCampaign()).resolves.toBeNull()

    fetchMock.mockResolvedValueOnce(json({ sys: { id: 'admin' }, fields: { campaign: link('c1') } }))
    fetchMock.mockResolvedValueOnce(json(entry('c1', 'marketingCampaign', { name: 'Pride' })))
    await expect(fetchCampaign()).resolves.toBeNull()

    fetchMock.mockRejectedValueOnce(new Error('offline'))
    await expect(fetchCampaign()).resolves.toBeNull()
  })
})
