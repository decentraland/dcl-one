// The shell's lazy load of the in-world Shop's purchase code (lib/loadOnce): kept once loaded, loaded
// again after a failure.
import { describe, expect, it, vi } from 'vitest'
import { loadOnce } from '../lib/loadOnce'

describe('a module loaded on first use', () => {
  it('loads once and keeps it', async () => {
    const load = vi.fn(async () => ({ ready: true }))
    const get = loadOnce(load)
    const first = await get()
    expect(await get()).toBe(first)
    expect(load).toHaveBeenCalledTimes(1)
  })

  it('loads again on the next call after a failed load', async () => {
    const load = vi.fn<() => Promise<string>>().mockRejectedValueOnce(new Error('Failed to fetch dynamically imported module')).mockResolvedValue('shop')
    const get = loadOnce(load)
    await expect(get()).rejects.toThrow('Failed to fetch')
    await expect(get()).resolves.toBe('shop')
    await expect(get()).resolves.toBe('shop')
    expect(load).toHaveBeenCalledTimes(2)
  })
})
