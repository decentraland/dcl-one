import { afterEach, describe, expect, it, vi } from 'vitest'
import { probeGpu } from '@hud/lib/gpu'

// jsdom's navigator has no `gpu`; define/override it per case, restore after.
const original = (navigator as unknown as { gpu?: unknown }).gpu
function setGpu(gpu: unknown): void {
  Object.defineProperty(navigator, 'gpu', { value: gpu, configurable: true, writable: true })
}
afterEach(() => {
  setGpu(original)
  vi.useRealTimers()
})

describe('probeGpu', () => {
  it('is not usable when WebGPU is absent (navigator.gpu undefined)', async () => {
    setGpu(undefined)
    expect(await probeGpu()).toEqual({ usable: false, adapter: null })
  })

  it('is not usable when requestAdapter resolves null (no usable GPU)', async () => {
    setGpu({ requestAdapter: () => Promise.resolve(null) })
    expect(await probeGpu()).toEqual({ usable: false, adapter: null })
  })

  it('is usable, with no adapter info, when the adapter reports none', async () => {
    setGpu({ requestAdapter: () => Promise.resolve({}) })
    expect(await probeGpu()).toEqual({ usable: true, adapter: null })
  })

  it('keeps what the adapter reports about itself, missing fields as empty strings', async () => {
    setGpu({
      requestAdapter: () =>
        Promise.resolve({ info: { vendor: 'intel', architecture: 'gen-12lp', description: 'Intel(R) Iris(R) Xe Graphics', device: 42 } })
    })
    expect(await probeGpu()).toEqual({
      usable: true,
      adapter: { vendor: 'intel', architecture: 'gen-12lp', device: '', description: 'Intel(R) Iris(R) Xe Graphics' }
    })
  })

  it('is not usable when requestAdapter throws', async () => {
    setGpu({ requestAdapter: () => Promise.reject(new Error('boom')) })
    expect(await probeGpu()).toEqual({ usable: false, adapter: null })
  })

  it('fails open (usable) when the probe hangs past the timeout', async () => {
    vi.useFakeTimers()
    setGpu({ requestAdapter: () => new Promise(() => {}) }) // never resolves
    const result = probeGpu()
    await vi.advanceTimersByTimeAsync(5000)
    expect(await result).toEqual({ usable: true, adapter: null })
  })
})
