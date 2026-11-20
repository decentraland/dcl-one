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

describe('when the gallery is built for the hosted pull request preview', () => {
  const build = (mode?: string) =>
    resolveConfig(
      { configFile: path.join(library, 'gallery/vite.config.ts'), logLevel: 'silent', mode },
      'build'
    )

  it('should build the gallery alone into gallery/dist, without the Figma reference', async () => {
    const hosted = await build('hosted')
    expect(path.relative(library, hosted.build.outDir)).toBe('gallery/dist')
    expect(hosted.define?.__GENERATED_REVIEW_URL__).toBe('null')
    expect(hosted.define?.__GENERATED_COMPONENTS__).toBe('[]')
  })

  it('should keep the default build beside the generated Review, which it links', async () => {
    const combined = await build()
    expect(path.relative(library, combined.build.outDir)).toBe('figma/review/review-dist/gallery')
    expect(combined.define?.__GENERATED_REVIEW_URL__).toBe('"../dcl-ui-review/"')
    expect(JSON.parse(String(combined.define?.__GENERATED_COMPONENTS__)).length).toBeGreaterThan(0)
  })
})
