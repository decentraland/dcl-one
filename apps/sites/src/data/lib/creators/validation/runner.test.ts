import { afterEach, describe, expect, it, vi } from 'vitest'

const validate = vi.fn().mockResolvedValue({
  findings: [{ check: 'thumbnail', severity: 'warning', message: 'Not transparent' }],
  checks: [{ check: 'thumbnail', status: 'passed' }]
})
vi.mock('@dcl-regenesislabs/wearable-validator', () => ({
  validate: (...args: unknown[]) => validate(...args),
  checks: { thumbnail: { title: 'Thumbnail' } }
}))
vi.mock('./ruleBook.worker?worker&url', () => ({ default: '/ruleBook.worker.js' }))
const captureError = vi.fn()
vi.mock('../monitoring', () => ({ captureError: (...args: unknown[]) => captureError(...args) }))

const job = () => ({ kind: 'thumbnail', path: 'thumbnail.png', bytes: new Uint8Array([1]) }) as const

/** A worker that never answers, like one stuck on a pathological model. */
class SilentWorker {
  static spawned = 0
  terminated = false
  onmessage: unknown = null
  onerror: unknown = null
  constructor() {
    SilentWorker.spawned++
  }
  postMessage() {}
  terminate() {
    this.terminated = true
  }
}

afterEach(() => {
  vi.unstubAllGlobals()
  vi.useRealTimers()
  vi.resetModules()
})

describe('validation runner', () => {
  it('runs the rule book inline where workers are unavailable', async () => {
    const { runOffMainThread } = await import('./runner')
    expect(typeof Worker).toBe('undefined')
    const output = await runOffMainThread(job())
    expect(validate).toHaveBeenCalled()
    expect(output).toMatchObject({
      findings: [{ check: 'thumbnail', message: 'Not transparent' }],
      titles: { thumbnail: 'Thumbnail' }
    })
  })

  it('gives up on a hung worker run and starts a fresh worker for the next one', async () => {
    vi.useFakeTimers()
    vi.stubGlobal('Worker', SilentWorker)
    vi.stubGlobal('URL', Object.assign(URL, { createObjectURL: () => 'blob:shim', revokeObjectURL: () => undefined }))
    SilentWorker.spawned = 0
    const { WORKER_TIMEOUT_MS, runOffMainThread } = await import('./runner')

    const hung = runOffMainThread(job())
    const outcome = expect(hung).rejects.toThrow(/timed out/)
    await vi.advanceTimersByTimeAsync(WORKER_TIMEOUT_MS)
    await outcome
    expect(captureError).toHaveBeenCalledWith(expect.any(Error), expect.objectContaining({ step: 'timeout' }))

    void runOffMainThread(job()).catch(() => undefined)
    await vi.advanceTimersByTimeAsync(0)
    expect(SilentWorker.spawned).toBe(2)
  })
})
