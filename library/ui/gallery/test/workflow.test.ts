// @vitest-environment node
import { readFileSync } from 'node:fs'
import path from 'node:path'
import { resolveConfig } from 'vite'
import { describe, expect, it } from 'vitest'
import { parse } from 'yaml'

// The Shared UI library workflow, which builds and checks the gallery.
const library = path.resolve(import.meta.dirname, '../..')
const repository = path.resolve(library, '../..')
type Step = { id?: string; name?: string; run?: string; if?: string }
const workflow = parse(
  readFileSync(path.join(repository, '.github/workflows/ui-library.yml'), 'utf8')
) as { jobs: Record<string, { steps: Step[] }> }
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
