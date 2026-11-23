// @vitest-environment node
import path from 'node:path'
import ts from 'typescript'
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

describe('when the gallery compiles the Explorer HUD for its HUD canvas', () => {
  const config = (command: 'serve' | 'build', mode?: string) =>
    resolveConfig(
      { configFile: path.join(library, 'gallery/vite.config.ts'), logLevel: 'silent', mode },
      command
    )
  const target = (resolved: Awaited<ReturnType<typeof config>>, find: string) =>
    path.relative(
      library,
      String(resolved.resolve.alias.find((alias) => alias.find === find)?.replacement)
    )

  it('should give the HUD the names its hosts give it, with the library HUD test host', async () => {
    const served = await config('serve')
    expect(target(served, '@hud')).toBe('apps/hud')
    expect(target(served, '@explorer-core')).toBe('../explorer-core')
    expect(target(served, '@hud-host')).toBe('apps/hud/test/host.ts')
    expect(served.server.fs.allow).toContain(path.resolve(library, '../explorer-core'))
  })

  it('should build the HUD canvas in the combined build and in the hosted preview', async () => {
    for (const mode of [undefined, 'hosted']) {
      const built = await config('build', mode)
      expect(Object.keys(built.build.rolldownOptions.input ?? {})).toEqual([
        'index',
        'frame',
        'hud'
      ])
    }
  })
})

describe('when the HUD code the gallery builds reaches a package only the HUD hosts install', () => {
  type File = { fileName: string; type?: 'asset'; imports?: string[]; dynamicImports?: string[] }
  const generate = async (files: File[]) => {
    const { noHostPackages } = await import('../vite.config')
    const hook = noHostPackages().generateBundle as unknown as (
      options: object,
      bundle: object
    ) => void
    const bundle = Object.fromEntries(
      files.map(({ fileName, type = 'chunk', imports = [], dynamicImports = [] }) => [
        fileName,
        { fileName, type, imports, dynamicImports }
      ])
    )
    return () => hook.call({}, {}, bundle)
  }

  it("should leave the signer's packages out of the combined build and the hosted preview", async () => {
    const { HOST_PACKAGES } = await import('../vite.config')
    for (const mode of [undefined, 'hosted']) {
      const built = await resolveConfig(
        { configFile: path.join(library, 'gallery/vite.config.ts'), logLevel: 'silent', mode },
        'build'
      )
      expect(built.build.rolldownOptions.external).toEqual([HOST_PACKAGES])
      expect(built.plugins.map((plugin) => plugin.name)).toContain('gallery:no-host-packages')
    }
    for (const id of [
      '@noble/curves/secp256k1.js',
      '@noble/hashes/sha3.js',
      '@noble/hashes/utils.js'
    ])
      expect(HOST_PACKAGES.test(id)).toBe(true)
  })

  it('should fail the build when a page imports one, naming the chunk and the package', async () => {
    const run = await generate([
      { fileName: 'assets/hud.js', imports: ['assets/react.js', '@noble/curves/secp256k1.js'] }
    ])
    expect(run).toThrow(
      'assets/hud.js loads @noble/curves/secp256k1.js: only the HUD hosts install it, and the gallery signs nothing'
    )
  })

  it('should fail the build when a page would load one on demand', async () => {
    const run = await generate([
      { fileName: 'assets/places.js', dynamicImports: ['@noble/hashes/sha3.js'] }
    ])
    expect(run).toThrow('assets/places.js loads @noble/hashes/sha3.js')
  })

  it('should pass a build whose pages load none of them', async () => {
    const run = await generate([
      {
        fileName: 'assets/hud.js',
        imports: ['assets/react.js'],
        dynamicImports: ['assets/Showcase.js']
      },
      { fileName: 'assets/hud.css', type: 'asset' }
    ])
    expect(run).not.toThrow()
  })
})

describe('when the HUD names are declared for the bundler, the tests and the type check', () => {
  it('should map in tsconfig.hud.json exactly the names the Vite config and the hud tests resolve', async () => {
    const { hudAliases } = await import('../vite.config')
    const gallery = path.join(library, 'gallery')
    const { config } = ts.readConfigFile(path.join(gallery, 'tsconfig.hud.json'), ts.sys.readFile)
    const expected = Object.fromEntries(
      Object.entries(hudAliases).map(([name, target]) => {
        const relative = path.relative(gallery, target)
        return path.extname(target) ? [name, [relative]] : [`${name}/*`, [`${relative}/*`]]
      })
    )
    expect(config.compilerOptions.paths).toEqual(expected)
  })
})
