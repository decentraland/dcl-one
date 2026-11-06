import { afterEach, describe, expect, it, vi } from 'vitest'
import { FigmaClient } from '../figma/client.js'
import { exportImageFills, exportNodes, imageType } from './exports.js'

describe('imageType', () => {
  it('should recognise the raster formats an SVG image may embed', () => {
    expect(imageType(Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0]))).toBe('png')
    expect(imageType(Buffer.from([0xff, 0xd8, 0xff, 0xe0]))).toBe('jpeg')
    expect(imageType(Buffer.from('GIF89a', 'latin1'))).toBe('gif')
    expect(imageType(Buffer.from('RIFF\0\0\0\0WEBPVP8 ', 'latin1'))).toBe('webp')
  })
  it('should reject anything else, such as an SVG or HTML body', () => {
    expect(imageType(Buffer.from('<svg xmlns="http://www.w3.org/2000/svg"/>'))).toBeUndefined()
    expect(imageType(Buffer.from('<!doctype html>'))).toBeUndefined()
  })
})

describe('exportImageFills', () => {
  const png = Buffer.from(
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR4nGNgAAIAAAUAAXpeqz8AAAAASUVORK5CYII=',
    'base64'
  )
  const fill = async (bytes: Buffer) => {
    // The image list and the download both go through the client.
    const client = new FigmaClient({ getToken: async () => 'test' }, (async (url: string | URL) =>
      String(url).includes('/v1/files/')
        ? Response.json({
            meta: { images: { photo: 'https://images.figmausercontent.com/photo' } }
          })
        : new Response(new Uint8Array(bytes))) as typeof fetch)
    return exportImageFills(client, 'FAKEKEY000example000000', ['photo'])
  }

  it('should store a complete PNG as a data URI', async () => {
    expect((await fill(png)).get('photo')).toBe(`data:image/png;base64,${png.toString('base64')}`)
  })

  it('should fail on a PNG that keeps its signature but is truncated', async () => {
    await expect(fill(png.subarray(0, 30))).rejects.toThrow(
      'Figma image fill photo is corrupt: truncated PNG chunk'
    )
  })
})

describe('when Figma renders exports', () => {
  const SVG = '<svg xmlns="http://www.w3.org/2000/svg"></svg>'
  const ASSET = 'https://figma-alpha-api.s3.us-west-2.amazonaws.com/images/a'
  afterEach(() => vi.restoreAllMocks())

  // Records the per-attempt timeout of every request alongside its URL.
  function client(answer: (url: string) => Response | Promise<Response>) {
    const timeouts = vi.spyOn(AbortSignal, 'timeout')
    const urls: string[] = []
    const fetchFn = (async (url: string | URL) => {
      urls.push(String(url))
      return answer(String(url))
    }) as unknown as typeof fetch
    const figma = new FigmaClient({ getToken: async () => 'token' }, fetchFn, {
      sleep: async () => {},
      log: () => {}
    })
    const timeoutOf = (match: string) =>
      urls.flatMap((url, index) => (url.includes(match) ? [timeouts.mock.calls[index]![0]] : []))
    return { figma, urls, timeoutOf }
  }

  it('should give render calls 240s per attempt and keep downloads at 60s', async () => {
    const { figma, timeoutOf } = client((url) =>
      url.startsWith(ASSET) ? new Response(SVG) : Response.json({ images: { '1:1': ASSET } })
    )

    await exportNodes(figma, 'key', 'v', ['1:1'], 'svg')
    expect(timeoutOf('/v1/images/')).toEqual([240000])
    expect(timeoutOf('amazonaws.com')).toEqual([60000])
  })

  it('should give the image fill list 240s per attempt', async () => {
    const { figma, timeoutOf } = client(() => Response.json({ meta: { images: {} } }))

    await expect(exportImageFills(figma, 'key', ['ref'])).rejects.toThrow('image fill ref')
    expect(timeoutOf('/v1/files/key/images')).toEqual([240000])
  })

  describe('when resolving image fills', () => {
    // A complete 1x1 PNG: the ingest rejects a truncated one (imageDataProblem).
    const PNG = Buffer.from(
      'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR4nGNgAAIAAAUAAXpeqz8AAAAASUVORK5CYII=',
      'base64'
    )
    const fills = (images: Record<string, string | null>) =>
      client((url) =>
        url.includes('/v1/files/') ? Response.json({ meta: { images } }) : new Response(PNG)
      )

    it('should download only the requested refs, keyed by image ref', async () => {
      const { figma, urls } = fills({
        'ref-a': `${ASSET}/a`,
        'ref-b': `${ASSET}/b`,
        unrelated: `${ASSET}/c`
      })

      const result = await exportImageFills(figma, 'key', ['ref-b', 'ref-a', 'ref-b'])
      expect([...result.keys()]).toEqual(['ref-b', 'ref-a'])
      expect(result.get('ref-a')).toBe(`data:image/png;base64,${PNG.toString('base64')}`)
      expect(urls).toEqual([
        'https://api.figma.com/v1/files/key/images',
        `${ASSET}/b`,
        `${ASSET}/a`
      ])
    })

    it.each([
      ['absent from the response', {}],
      ['answered with no URL', { 'ref-x': null }]
    ])('should fail on a ref %s instead of picking another image', async (_, images) => {
      const { figma, urls } = fills({ other: `${ASSET}/o`, ...images })

      await expect(exportImageFills(figma, 'key', ['ref-x'])).rejects.toThrow(
        'Figma did not provide image fill ref-x'
      )
      expect(urls).toEqual(['https://api.figma.com/v1/files/key/images'])
    })
  })

  it('should fail on any requested id Figma does not render', async () => {
    const { figma } = client(() => Response.json({ images: { '1:1': null } }))

    await expect(exportNodes(figma, 'key', 'v', ['1:1'], 'svg')).rejects.toThrow(
      'Figma did not render 1:1'
    )
  })

  it('should keep a render that answers 503 three times and then completes slowly', async () => {
    let renders = 0
    const { figma, urls } = client((url) => {
      if (url.startsWith(ASSET)) return new Response(SVG)
      renders += 1
      if (renders <= 3) return new Response('', { status: 503 })
      return new Promise<Response>((resolve) =>
        setTimeout(() => resolve(Response.json({ images: { '1:1': ASSET } })), 50)
      )
    })

    const result = await exportNodes(figma, 'key', 'v', ['1:1'], 'svg')
    expect(result.get('1:1')?.toString()).toBe(SVG)
    expect(urls.filter((url) => url.includes('/v1/images/'))).toHaveLength(4)
  })
})
