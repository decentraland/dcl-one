import { describe, expect, it } from 'vitest'
import { blobsAreEqual } from './blobs'

describe('blobsAreEqual', () => {
  it('compares bytes, skipping the read when sizes differ', async () => {
    expect(await blobsAreEqual(new Blob([new Uint8Array([1, 2, 3])]), new Blob([new Uint8Array([1, 2, 3])]))).toBe(true)
    expect(await blobsAreEqual(new Blob([new Uint8Array([1, 2, 3])]), new Blob([new Uint8Array([1, 2, 4])]))).toBe(
      false
    )
    expect(await blobsAreEqual(new Blob([new Uint8Array([1, 2, 3])]), new Blob([new Uint8Array([1, 2])]))).toBe(false)
  })
})
