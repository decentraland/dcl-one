import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { FigmaClient } from '../figma/client.js'
import { RestSource } from './rest-source.js'

const FILE = 'FAKEKEY000example000000'
const SVG = '<svg viewBox="0 0 1 1"><path d="M0 0h1v1z"></path></svg>'

// A REST API that lists an Icons page with `count` icons and answers each export URL with `exported`.
function source(count: number, exportUrl: (index: number) => string, exported: () => Response) {
  let active = 0
  const seen = { maxActive: 0, downloads: 0 }
  const json = (body: unknown) => new Response(JSON.stringify(body), { status: 200 })
  const fetchFn = (async (input: string | URL) => {
    const url = new URL(String(input))
    if (url.pathname === `/v1/files/${FILE}`)
      return json({ document: { children: [{ id: '0:1', name: 'Icons', type: 'CANVAS' }] } })
    if (url.pathname === `/v1/files/${FILE}/nodes`) {
      const children = Array.from({ length: count }, (_, index) => ({
        id: `1:${index}`,
        name: `nav/icon-${index}`,
        type: 'COMPONENT',
        absoluteBoundingBox: { x: 0, y: 0, width: 24, height: 24 }
      }))
      const holder = { id: '0:3', name: 'Frame', type: 'FRAME', children }
      const section = { id: '0:2', name: 'Nav', type: 'FRAME', children: [holder] }
      return json({ nodes: { '0:1': { document: { children: [section] } } } })
    }
    if (url.pathname === `/v1/images/${FILE}`) {
      const ids = url.searchParams.get('ids')!.split(',')
      return json({
        images: Object.fromEntries(ids.map((id) => [id, exportUrl(Number(id.split(':')[1]))]))
      })
    }
    seen.downloads++
    active++
    seen.maxActive = Math.max(seen.maxActive, active)
    await new Promise((resolve) => setTimeout(resolve, 1))
    active--
    return exported()
  }) as unknown as typeof fetch
  const client = new FigmaClient({ getToken: async () => 'token' }, fetchFn, {
    sleep: async () => {},
    log: () => {}
  })
  return { rest: new RestSource(client, FILE), seen }
}

const figmaExport = (index: number) => `https://s3-alpha.figma.com/export/${index}.svg`

describe('RestSource.getIcons', () => {
  // The global fetch must never be reached: it would bypass the client's host, status and size checks.
  let globalFetch: ReturnType<typeof vi.spyOn>
  beforeEach(() => {
    globalFetch = vi.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('global fetch used'))
  })
  afterEach(() => vi.restoreAllMocks())

  describe('when every export is a valid SVG on a Figma host', () => {
    it('should download through the client, a few at a time, never with the global fetch', async () => {
      const { rest, seen } = source(20, figmaExport, () => new Response(SVG, { status: 200 }))

      const icons = await rest.getIcons()

      expect(icons).toHaveLength(20)
      expect(icons[0]!.svg).toBe(SVG)
      expect(seen.downloads).toBe(20)
      expect(seen.maxActive).toBeLessThanOrEqual(8)
      expect(globalFetch).not.toHaveBeenCalled()
    })
  })

  describe('when an export URL is on another host', () => {
    it('should reject it without downloading', async () => {
      const { rest, seen } = source(
        1,
        () => 'https://downloads.example.com/icon.svg',
        () => new Response(SVG, { status: 200 })
      )

      await expect(rest.getIcons()).rejects.toThrow('Unexpected export host')
      expect(seen.downloads).toBe(0)
    })
  })

  describe('when an export answers with an error status', () => {
    it('should fail instead of storing the error body as the icon', async () => {
      const { rest } = source(1, figmaExport, () => new Response('Not found', { status: 404 }))

      await expect(rest.getIcons()).rejects.toThrow('Export download failed')
    })
  })

  describe('when an export is not a complete SVG document', () => {
    it('should fail with the SVG problem', async () => {
      const { rest } = source(1, figmaExport, () => new Response('<html></html>', { status: 200 }))

      await expect(rest.getIcons()).rejects.toThrow('Invalid svg export for nav/icon-0')
    })
  })
})
