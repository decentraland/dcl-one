import { afterEach, describe, expect, it, vi } from 'vitest'
import { copyToClipboard } from './clipboard'

describe('copyToClipboard', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('writes the text and reports success', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    vi.stubGlobal('navigator', { clipboard: { writeText } })
    await expect(copyToClipboard('urn:x')).resolves.toBe(true)
    expect(writeText).toHaveBeenCalledWith('urn:x')
  })

  it('reports failure instead of throwing when the clipboard is unavailable', async () => {
    vi.stubGlobal('navigator', { clipboard: { writeText: vi.fn().mockRejectedValue(new Error('denied')) } })
    await expect(copyToClipboard('urn:x')).resolves.toBe(false)
  })
})
