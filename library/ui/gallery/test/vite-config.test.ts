// @vitest-environment node
import path from 'node:path'
import { resolveConfig } from 'vite'
import { describe, expect, it } from 'vitest'

const library = path.resolve(import.meta.dirname, '../..')

describe('when the gallery and the generated Review dev servers run side by side', () => {
  it('should keep separate dependency caches, so neither re-optimizes the other away', async () => {
    const resolve = (file: string) =>
      resolveConfig({ configFile: path.join(library, file), logLevel: 'silent' }, 'serve')
    const gallery = await resolve('gallery/vite.config.ts')
    const review = await resolve('figma/review/review.vite.config.ts')
    expect(gallery.cacheDir).not.toBe(review.cacheDir)
    expect(path.relative(library, gallery.cacheDir)).toBe('node_modules/.vite-gallery')
  })
})
