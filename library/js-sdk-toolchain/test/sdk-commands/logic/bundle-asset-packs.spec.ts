import esbuild from '../../../packages/@dcl/sdk-commands/node_modules/esbuild'
import fs from 'fs'
import os from 'os'
import path from 'path'
import { smartItemsResolver, smartItemsSpecifier } from '../../../packages/@dcl/sdk-commands/src/logic/bundle'

const sdkPackagePath = path.resolve(__dirname, '../../../packages/@dcl/sdk')
// where the plugin falls back to in a build: sdk-commands' own folder, which depends on @dcl/smart-items
const sdkCommandsLogicPath = path.resolve(__dirname, '../../../packages/@dcl/sdk-commands/src/logic')

describe('smartItemsSpecifier', () => {
  it.each([
    ['@dcl/asset-packs', '@dcl/smart-items'],
    ['@dcl/asset-packs/dist/scene-entrypoint', '@dcl/smart-items'],
    ['@dcl/asset-packs/dist/scene-entrypoint.js', '@dcl/smart-items'],
    ['@dcl/asset-packs/dist/helpers', '@dcl/smart-items/helpers'],
    ['@dcl/asset-packs/dist/helpers.js', '@dcl/smart-items/helpers'],
    ['@dcl/asset-packs/dist/index', '@dcl/smart-items/index'],
    ['@dcl/asset-packs/dist/admin-toolkit-ui/index.js', '@dcl/smart-items/admin-toolkit-ui/index']
  ])('maps %s to %s', (specifier, expected) => {
    expect(smartItemsSpecifier(specifier)).toBe(expected)
  })

  it('leaves specifiers outside dist alone', () => {
    expect(smartItemsSpecifier('@dcl/asset-packs/package.json')).toBeUndefined()
    expect(smartItemsSpecifier('@dcl/asset-packs/src/helpers')).toBeUndefined()
    expect(smartItemsSpecifier('@dcl/asset-packs/dist/')).toBeUndefined()
    expect(smartItemsSpecifier('@dcl/asset-packs-fork')).toBeUndefined()
  })
})

describe('smartItemsResolver', () => {
  let directory: string

  beforeEach(() => {
    directory = fs.mkdtempSync(path.join(os.tmpdir(), 'smart-items-resolver-'))
  })

  afterEach(() => {
    fs.rmSync(directory, { recursive: true, force: true })
  })

  // the scene build aliases `@dcl/sdk` to the package it bundles; an `external` pattern is matched after that alias
  async function bundle(contents: string, options: Pick<esbuild.BuildOptions, 'external' | 'alias'>) {
    const result = await esbuild.build({
      bundle: true,
      write: false,
      format: 'esm',
      logLevel: 'silent',
      absWorkingDir: directory,
      ...options,
      plugins: [smartItemsResolver([directory, sdkCommandsLogicPath])],
      stdin: { contents, resolveDir: directory, loader: 'ts' }
    })
    return result.outputFiles[0].text
  }

  it('rewrites @dcl/asset-packs imports to @dcl/smart-items specifiers', async () => {
    const output = await bundle(
      [
        "import { initAssetPacks } from '@dcl/asset-packs/dist/scene-entrypoint'",
        "import { getPlayerPosition } from '@dcl/asset-packs/dist/helpers.js'",
        "import * as pack from '@dcl/asset-packs'",
        'export { initAssetPacks, getPlayerPosition, pack }'
      ].join('\n'),
      { external: ['@dcl/*'] }
    )

    expect(output).toContain('"@dcl/smart-items/helpers"')
    expect(output).toContain('"@dcl/smart-items"')
    expect(output).not.toContain('@dcl/asset-packs')
  })

  it('resolves the rewritten specifiers to the package files', async () => {
    const output = await bundle(
      "import { LEGACY_PREFIX, PREFIX } from '@dcl/asset-packs/dist/legacy'\nexport const prefixes = [LEGACY_PREFIX, PREFIX]",
      { external: ['@dcl/ecs', '@dcl/ecs/*'], alias: { '@dcl/sdk': sdkPackagePath } }
    )

    expect(output).toContain('"asset-packs::"')
    expect(output).toContain('"smart-items::"')
    expect(output).not.toContain('@dcl/asset-packs')
  })

  it('resolves @dcl/smart-items imports a scene does not reach through its own folder', async () => {
    const output = await bundle("import { runScripts } from '@dcl/smart-items/scripts'\nexport { runScripts }", {
      external: ['@dcl/ecs', '@dcl/ecs/*', '@dcl/sdk', '@dcl/sdk/*', 'mitt']
    })

    expect(output).toContain('function runScripts(')
  })

  it('leaves other subpaths of @dcl/asset-packs to the default resolution', async () => {
    await expect(
      bundle("import data from '@dcl/asset-packs/package.json'\nexport { data }", {
        alias: { '@dcl/sdk': sdkPackagePath }
      })
    ).rejects.toThrow(/Could not resolve "@dcl\/asset-packs\/package.json"/)
  })
})
