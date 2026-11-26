// @vitest-environment node
import { readFileSync } from 'node:fs'
import path from 'node:path'
import { resolveConfig } from 'vite'
import { describe, expect, it } from 'vitest'
import { parse } from 'yaml'

// The Shared UI library workflow, which builds and checks the gallery.
const library = path.resolve(import.meta.dirname, '../..')
const repository = path.resolve(library, '../..')
type Step = {
  id?: string
  name?: string
  run?: string
  if?: string
  uses?: string
  with?: Record<string, unknown>
}
const workflow = parse(
  readFileSync(path.join(repository, '.github/workflows/ui-library.yml'), 'utf8')
) as { jobs: Record<string, { steps: Step[]; if?: string; needs?: string }> }
const steps = (job: string) => workflow.jobs[job]!.steps

describe('when a change reaches what the gallery compiles', () => {
  it('should run the library job for every tree the gallery reads, the HUD sources included', async () => {
    const config = await resolveConfig(
      { configFile: path.join(library, 'gallery/vite.config.ts'), logLevel: 'silent' },
      'serve'
    )
    const scope = steps('key-guard').find((step) => step.id === 'scope')!.run!
    const scoped = scope.match(/git log [^\n]* -- ([^\n]+)/)![1]!.split(/\s+/)
    const trees = config.server.fs.allow.map((dir) => path.relative(repository, dir))
    expect(trees).toContain('library/explorer-core')
    for (const tree of trees)
      expect(scoped.some((entry) => tree === entry || tree.startsWith(`${entry}/`))).toBe(true)
  })
})

describe('when the HUD canvas is type-checked in CI', () => {
  it('should run after any failed check, but not after a failed install', () => {
    const library = steps('library')
    const install = library.find((step) => step.run === 'pnpm install --frozen-lockfile')!
    const check = library.find((step) => step.run?.includes('gallery/tsconfig.hud.json'))!
    expect(install.id).toBe('install')
    expect(check.if).toContain('!cancelled()')
    expect(check.if).toContain(`steps.${install.id}.outcome == 'success'`)
    expect(library.indexOf(check)).toBe(library.length - 1)
  })
})

describe('when Sites serves the hosted gallery', () => {
  it('should put the hosted build, refused with a hidden path, into the scanned artifact before the scans', () => {
    const library = steps('library')
    const index = library.findIndex((step) => step.run?.includes('--mode hosted'))
    const hosted = library[index]!.run!
    const urlScan = library.findIndex((step) => step.run?.includes('key-guard.mjs'))
    const upload = library.find((step) => step.uses?.startsWith('actions/upload-artifact@'))!
    expect(hosted).toContain('node gallery/scripts/hosted-digest.mjs gallery/dist\n')
    expect(hosted.indexOf('hosted-digest.mjs')).toBeLessThan(hosted.indexOf('cp -R'))
    expect(hosted).toContain(
      `cp -R gallery/dist ${String(upload.with!.path).replace('library/ui/', '')}/ui-gallery`
    )
    expect(index).toBeGreaterThan(-1)
    expect(index).toBeLessThan(urlScan)
  })

  it('should build the Nix package whenever the library job runs', () => {
    const nix = workflow.jobs['ui-gallery-nix']!
    expect(nix.if).toBe(workflow.jobs.library!.if)
    expect(nix.needs).toBe('key-guard')
    expect(
      nix.steps.some((step) => step.run === "nix build 'git+file:.?lfs=1#ui-gallery' --no-link -L")
    ).toBe(true)
  })

  it('should fetch from Git LFS every directory the Nix build refuses as LFS pointers', () => {
    const nix = readFileSync(path.join(repository, 'infra/packages/ui-gallery.nix'), 'utf8')
    const guarded = nix
      .match(/grep -rIl '\^version https:\/\/git-lfs\.github\.com\/spec\/v1\$' ([^;]+);/)![1]!
      .trim()
      .split(/\s+/)
    const pull = steps('ui-gallery-nix')
      .find((step) => step.run?.startsWith('git lfs pull'))!
      .run!.match(/--include "([^"]+)"/)![1]!
      .split(',')
    expect(guarded).toEqual(expect.arrayContaining(['foundations/typography/fonts', 'assets']))
    for (const dir of guarded) expect(pull).toContain(`library/ui/${dir}/**`)
  })
})
