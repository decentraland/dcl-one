import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { dropUnusedSharedAssets } from '../../vite.config'
import { SHARED_SOURCES } from '../../vite.shared'

// The build drops the shared library's emitted assets that nothing in the output names: the shared entry reaches
// the images and videos of views the HUD never renders.
const library = join(SHARED_SOURCES['@dcl/one-ui'], '..')
const asset = (fileName: string, original: string, source: string | Uint8Array = new Uint8Array([1])) => ({
  type: 'asset' as const,
  fileName,
  originalFileNames: [original],
  source
})
const chunk = (fileName: string, code: string) => ({ type: 'chunk' as const, fileName, code })

function run(outputs: Array<ReturnType<typeof asset> | ReturnType<typeof chunk>>): string[] {
  const bundle = Object.fromEntries(outputs.map((output) => [output.fileName, output]))
  const plugin = dropUnusedSharedAssets() as unknown as {
    configResolved(config: { root: string }): void
    generateBundle(options: unknown, bundle: Record<string, unknown>): void
  }
  plugin.configResolved({ root: join(library, '..', '..', 'rendering') })
  plugin.generateBundle({}, bundle)
  return Object.keys(bundle).sort()
}

describe('when the HUD build emits the shared library assets', () => {
  it('should drop a shared asset that no output names', () => {
    expect(
      run([
        chunk('assets/app.js', 'console.log(1)'),
        asset('assets/hero-abc.mp4', join(library, 'apps/sites/assets/hero.mp4'))
      ])
    ).toEqual(['assets/app.js'])
  })

  it('should keep a shared asset a chunk or a stylesheet names', () => {
    expect(
      run([
        chunk('assets/app.js', 'const url = "/assets/logo-abc.webp"'),
        asset('assets/app.css', join(library, 'components/A/A.module.css'), '.a{background:url(/assets/sea-abc.webp)}'),
        asset('assets/logo-abc.webp', join(library, 'assets/logo.webp')),
        asset('assets/sea-abc.webp', join(library, 'assets/sea.webp'))
      ])
    ).toEqual(['assets/app.css', 'assets/app.js', 'assets/logo-abc.webp', 'assets/sea-abc.webp'])
  })

  it('should not count an asset naming itself as a reference', () => {
    expect(run([asset('assets/mark-abc.svg', join(library, 'assets/mark.svg'), '<svg id="mark-abc.svg"/>')])).toEqual([])
  })

  it('should leave unnamed assets of the HUD, the in-world Shop and the app itself', () => {
    expect(
      run([
        asset('assets/hud-abc.png', join(library, 'apps/hud/assets/hud.png')),
        asset('assets/shop-abc.png', join(library, 'apps/shop/assets/shop.png')),
        asset('assets/own-abc.png', 'src/assets/own.png'),
        asset('assets/none-abc.png', '')
      ])
    ).toEqual(['assets/hud-abc.png', 'assets/none-abc.png', 'assets/own-abc.png', 'assets/shop-abc.png'])
  })
})
