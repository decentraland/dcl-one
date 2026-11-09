import { execFile } from 'node:child_process'
import { existsSync } from 'node:fs'
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { promisify } from 'node:util'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { pinTestCapture } from '../test/snapshot.js'
import { PLACES, GEN, put, writeTree } from './guard/test-fixtures.js'

const execute = promisify(execFile)
const cli = path.join(import.meta.dirname, 'cli.ts')

describe('guard and retire commands', () => {
  let base: string
  let staged: string

  const run = async (cwd: string, args: string[]) => {
    // retire verifies the capture before it reads ownership, as fetch-source would have pinned it.
    // This guard fixture has no config of its own: retire needs one that names the capture to verify.
    if (args[0] === 'retire') {
      if (!existsSync(path.join(cwd, 'figma.config.json')))
        await writeFile(
          path.join(cwd, 'figma.config.json'),
          JSON.stringify({
            snapshotDir: 'figma-snapshot',
            outDir: 'review/src/generated',
            pages: {}
          })
        )
      await pinTestCapture(cwd)
    }
    try {
      const { stdout } = await execute(
        process.execPath,
        ['--import', import.meta.resolve('tsx'), cli, ...args],
        // These cases compare local trees; PR event provenance has dedicated integration coverage.
        { cwd, env: { ...process.env, CI: '', GITHUB_EVENT_NAME: 'local-fixture' } }
      )
      return { code: 0, out: stdout }
    } catch (error) {
      const failure = error as { code: number; stdout: string }
      return { code: failure.code, out: failure.stdout }
    }
  }

  beforeEach(async () => {
    base = await mkdtemp(path.join(os.tmpdir(), 'dcl-cli-base-'))
    staged = await mkdtemp(path.join(os.tmpdir(), 'dcl-cli-staged-'))
    await writeTree(base)
    await put(base, 'package.json', { name: 'fixture', version: '0.1.0' })
    await put(base, 'pnpm-lock.yaml', {
      name: 'fixture',
      version: '0.1.0',
      packages: { '': { name: 'fixture', version: '0.1.0' } }
    })
    await cp(base, staged, { recursive: true })
    await mkdir(path.join(base, 'figma'), { recursive: true })
  })

  afterEach(async () => {
    await rm(base, { recursive: true, force: true })
    await rm(staged, { recursive: true, force: true })
  })

  describe('when the staged tree matches the base', () => {
    it('should exit 0', async () => {
      const result = await run(base, ['guard', '--base', base, '--staged', staged])

      expect(result.code).toBe(0)
      expect(JSON.parse(result.out).ok).toBe(true)
    })
  })

  describe('when an automated run changed a frozen page', () => {
    it('should exit 1 and name the file', async () => {
      await put(staged, `${GEN}/components/places/PlaceCard.tsx`, 'place v2')

      const result = await run(base, [
        'guard',
        '--base',
        base,
        '--staged',
        staged,
        '--captured',
        '1:1'
      ])

      expect(result.code).toBe(1)
      expect(JSON.parse(result.out).violations[0].path).toBe(
        `${GEN}/components/places/PlaceCard.tsx`
      )
    })

    it('should pass when that page was captured in the run', async () => {
      await put(staged, `${GEN}/components/places/PlaceCard.tsx`, 'place v2')

      const result = await run(base, [
        'guard',
        '--base',
        base,
        '--staged',
        staged,
        '--captured',
        PLACES
      ])

      expect(result.code).toBe(0)
    })
  })

  describe('when a developer records a retirement', () => {
    it('should write the owned artifacts into the record', async () => {
      const result = await run(path.join(base, 'figma'), [
        'retire',
        '--page-id',
        PLACES,
        '--reason',
        'obsolete'
      ])

      expect(result.code).toBe(0)
      const saved = JSON.parse(
        await readFile(path.join(base, 'figma/ledger/retirements.json'), 'utf8')
      )
      expect(saved.retired[0].owned).toEqual(
        expect.arrayContaining([`${GEN}/components/places/PlaceCard.tsx`])
      )
      expect(saved.retired[0].reason).toBe('obsolete')
    })

    it('should refuse an unknown page', async () => {
      const result = await run(path.join(base, 'figma'), [
        'retire',
        '--page-id',
        '99:99',
        '--reason',
        'x'
      ])

      expect(result.code).toBe(1)
    })
  })
})
