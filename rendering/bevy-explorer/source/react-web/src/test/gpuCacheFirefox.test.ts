// What the render worker's GPU setup (deploy/web/engine/gpu_cache.js) changes for Firefox, with the
// browser's WebGPU classes faked: jsdom has none.
import { afterEach, describe, expect, it, vi } from 'vitest'
// plain JS the engine embeds as is (deploy/web/engine), so it has no declarations
// @ts-expect-error TS7016
import { patchPowerPreference, patchVideoFrameCopy } from '../../../deploy/web/engine/gpu_cache.js'

const FIREFOX_LINUX = 'Mozilla/5.0 (X11; Linux x86_64; rv:152.0) Gecko/20100101 Firefox/152.0'
const FIREFOX_WINDOWS = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:152.0) Gecko/20100101 Firefox/152.0'
const CHROME_LINUX = 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/157.0.0.0 Safari/537.36'

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

describe('the adapter the engine asks for', () => {
  /** The options the browser gets when the engine asks for an adapter with `options` in the browser `userAgent`. */
  function asked(userAgent: string, options: object): unknown {
    vi.spyOn(navigator, 'userAgent', 'get').mockReturnValue(userAgent)
    const browser = vi.fn()
    class GPU {
      requestAdapter(given: unknown): void {
        browser(given)
      }
    }
    vi.stubGlobal('GPU', GPU)
    patchPowerPreference()
    new GPU().requestAdapter(options)
    return browser.mock.calls[0][0]
  }

  it('has no "high-performance" preference in Firefox on Linux, and keeps its other options', () => {
    expect(asked(FIREFOX_LINUX, { powerPreference: 'high-performance', forceFallbackAdapter: false })).toEqual({ forceFallbackAdapter: false })
  })

  it('keeps a "low-power" preference in Firefox on Linux', () => {
    expect(asked(FIREFOX_LINUX, { powerPreference: 'low-power' })).toEqual({ powerPreference: 'low-power' })
  })

  it('keeps "high-performance" in Firefox on Windows', () => {
    expect(asked(FIREFOX_WINDOWS, { powerPreference: 'high-performance' })).toEqual({ powerPreference: 'high-performance' })
  })

  it('keeps "high-performance" in Chrome on Linux', () => {
    expect(asked(CHROME_LINUX, { powerPreference: 'high-performance' })).toEqual({ powerPreference: 'high-performance' })
  })
})

describe('a video frame copied to a texture', () => {
  /** A browser whose own copy is `browserCopy`; returns the engine's copy call and the frames and bitmaps it made. */
  function browser(browserCopy: (source: { source: unknown }) => void) {
    const closed: string[] = []
    class VideoFrame {
      clone(): VideoFrame {
        return new VideoFrame()
      }
      close(): void {
        closed.push('frame')
      }
    }
    class ImageBitmap {
      close(): void {
        closed.push('bitmap')
      }
    }
    const copies = vi.fn<(source: { source: unknown }, destination: unknown, size: unknown) => void>(browserCopy)
    class GPUQueue {
      copyExternalImageToTexture(source: { source: unknown }, destination: unknown, size: unknown): void {
        copies(source, destination, size)
      }
    }
    vi.stubGlobal('VideoFrame', VideoFrame)
    vi.stubGlobal('GPUQueue', GPUQueue)
    vi.stubGlobal('createImageBitmap', async () => new ImageBitmap())
    vi.spyOn(console, 'log').mockImplementation(() => {})
    patchVideoFrameCopy()
    const queue = new GPUQueue()
    const copy = (source: unknown): void => queue.copyExternalImageToTexture({ source, flipY: true } as never, 'texture', 'size')
    return { copy, copies, closed, VideoFrame, ImageBitmap }
  }
  /** A browser that takes no video frame, as Firefox. */
  const refusesFrames = () =>
    browser((source) => {
      if (source.source instanceof globalThis.VideoFrame) throw new TypeError('not an image source')
    })

  it('goes to the browser as it is where the browser takes video frames', () => {
    const { copy, copies, closed, VideoFrame } = browser(() => {})
    const frame = new VideoFrame()
    copy(frame)
    expect(copies).toHaveBeenCalledTimes(1)
    expect(copies).toHaveBeenCalledWith({ source: frame, flipY: true }, 'texture', 'size')
    expect(closed).toEqual([])
  })

  it('goes through an image bitmap where the browser refuses video frames, and leaves nothing open', async () => {
    const { copy, copies, closed, VideoFrame, ImageBitmap } = refusesFrames()
    copy(new VideoFrame())
    await vi.waitFor(() => expect(copies).toHaveBeenCalledTimes(2))
    expect(copies).toHaveBeenLastCalledWith({ source: expect.any(ImageBitmap), flipY: true, origin: { x: 0, y: 0 } }, 'texture', 'size')
    // its own copy of the frame and the bitmap; the frame it was given stays the caller's to close
    expect(closed).toEqual(['frame', 'bitmap'])
  })

  it('does not offer the browser a video frame again after one refusal', async () => {
    const { copy, copies, VideoFrame, ImageBitmap } = refusesFrames()
    copy(new VideoFrame())
    await vi.waitFor(() => expect(copies).toHaveBeenCalledTimes(2))
    copy(new VideoFrame())
    await vi.waitFor(() => expect(copies).toHaveBeenCalledTimes(3))
    expect(copies.mock.calls[2][0]).toMatchObject({ source: expect.any(ImageBitmap) })
  })

  it('lets any other failure of the browser reach the engine', () => {
    const { copy, VideoFrame } = browser(() => {
      throw new RangeError('the texture is too small')
    })
    expect(() => copy(new VideoFrame())).toThrow(RangeError)
  })
})
