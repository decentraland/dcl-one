import { afterEach, describe, expect, it, vi } from 'vitest'
import { measureDownlinkMbps, PROBE_URL } from './connectionSpeed'

const CHUNK = 65_536

// A body delivered in `chunks` reads, the clock advancing `msPerChunk` between reads; `failAfter` reads in,
// the stream errors the way an aborted fetch does. No read-ahead, so the clock moves only on reads.
const ON_DEMAND = { highWaterMark: 0 }
function serve(chunks: number, msPerChunk: number, failAfter?: number) {
  let now = 0
  vi.spyOn(performance, 'now').mockImplementation(() => now)
  let sent = 0
  const body = new ReadableStream<Uint8Array>(
    {
      pull(controller) {
        if (failAfter !== undefined && sent === failAfter) {
          controller.error(new DOMException('timed out', 'TimeoutError'))
          return
        }
        if (sent === chunks) {
          controller.close()
          return
        }
        sent += 1
        now += msPerChunk
        controller.enqueue(new Uint8Array(CHUNK))
      }
    },
    ON_DEMAND
  )
  const fetchMock = vi.fn(() =>
    Promise.resolve(new Response(body, { headers: { 'content-length': String(chunks * CHUNK) } }))
  )
  vi.stubGlobal('fetch', fetchMock)
  return fetchMock
}

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

describe('measureDownlinkMbps', () => {
  it('downloads the probe uncached and times the second half of the body', async () => {
    // 8 chunks, 50 ms apart: the first 4 cover the midpoint, the other 4 are the timed 200 ms window.
    const fetchMock = serve(8, 50)
    expect(await measureDownlinkMbps()).toBeCloseTo((4 * CHUNK * 8) / 1e6 / 0.2, 2)
    expect(fetchMock).toHaveBeenCalledWith(PROBE_URL, expect.objectContaining({ cache: 'no-store' }))
  })

  it('ignores the slow start: the same link reads the same however slow the first half was', async () => {
    serve(8, 50)
    const even = await measureDownlinkMbps()
    let now = 0
    vi.spyOn(performance, 'now').mockImplementation(() => now)
    let sent = 0
    const body = new ReadableStream<Uint8Array>(
      {
        pull(controller) {
          if (sent === 8) return controller.close()
          sent += 1
          now += sent <= 4 ? 400 : 50
          controller.enqueue(new Uint8Array(CHUNK))
        }
      },
      ON_DEMAND
    )
    vi.stubGlobal(
      'fetch',
      vi.fn(() => Promise.resolve(new Response(body, { headers: { 'content-length': String(8 * CHUNK) } })))
    )
    expect(await measureDownlinkMbps()).toBeCloseTo(even as number, 5)
  })

  it('reads a timed-out probe as a slow link, whether before or part-way through the body', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(() => Promise.reject(new DOMException('timed out', 'TimeoutError')))
    )
    expect(await measureDownlinkMbps()).toBe(0)
    serve(8, 50, 5)
    expect(await measureDownlinkMbps()).toBe(0)
  })

  it('reads a failed, rejected or truncated probe as unknown', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(() => Promise.reject(new TypeError('Failed to fetch')))
    )
    expect(await measureDownlinkMbps()).toBeNull()
    vi.stubGlobal(
      'fetch',
      vi.fn(() => Promise.resolve(new Response('', { status: 404 })))
    )
    expect(await measureDownlinkMbps()).toBeNull()
    vi.stubGlobal(
      'fetch',
      vi.fn(() => Promise.resolve(new Response(new Uint8Array(10), { headers: { 'content-length': '10' } })))
    )
    expect(await measureDownlinkMbps()).toBeNull()
  })
})
