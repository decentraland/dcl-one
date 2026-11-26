import esbuild from 'esbuild'
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync, utimesSync, writeFileSync } from 'fs'
import os from 'os'
import path from 'path'
import { rebuildToDisk } from '../../../packages/@dcl/sdk-commands/src/logic/bundle'

describe('rebuildToDisk', () => {
  let directory: string
  let source: string
  let bundle: string
  let context: esbuild.BuildContext<{ write: false }>

  beforeEach(async () => {
    directory = mkdtempSync(path.join(os.tmpdir(), 'rebuild-to-disk-'))
    source = path.join(directory, 'index.ts')
    bundle = path.join(directory, 'bin', 'index.js')
    writeFileSync(source, 'export const scene = "first"\n')
    context = await esbuild.context({
      entryPoints: [source],
      bundle: true,
      format: 'cjs',
      outfile: bundle,
      write: false,
      logLevel: 'silent'
    })
  })

  afterEach(async () => {
    await context.dispose()
    rmSync(directory, { recursive: true, force: true })
  })

  it('writes the bundle of a build that passes, and makes its folder', async () => {
    expect(existsSync(bundle)).toBe(false)
    await rebuildToDisk(context)
    expect(readFileSync(bundle, 'utf8')).toContain('"first"')
  })

  it('leaves a bundle that did not change, with its date', async () => {
    await rebuildToDisk(context)
    const past = new Date(2000, 0, 1)
    utimesSync(bundle, past, past)
    await rebuildToDisk(context)
    expect(statSync(bundle).mtime).toEqual(past)
  })

  it('keeps the bundle of the last build that passed when a rebuild fails', async () => {
    await rebuildToDisk(context)
    const lastGoodBundle = readFileSync(bundle, 'utf8')

    writeFileSync(source, 'export const scene = \n')
    await expect(rebuildToDisk(context)).rejects.toThrow('Build failed')
    expect(readFileSync(bundle, 'utf8')).toBe(lastGoodBundle)

    writeFileSync(source, 'export const scene = "second"\n')
    await rebuildToDisk(context)
    expect(readFileSync(bundle, 'utf8')).toContain('"second"')
  })
})
