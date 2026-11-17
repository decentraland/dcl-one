import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { LfsPointerError, readMaterialized } from './lfs.js'
import { createRuntime } from './runtime.js'
import { pinTestCapture } from '../test/snapshot.js'

const POINTER = 'version https://git-lfs.github.com/spec/v1\noid sha256:00\nsize 1\n'

let directory: string

beforeEach(async () => {
  directory = await mkdtemp(path.join(tmpdir(), 'lfs-test-'))
})

afterEach(async () => {
  await rm(directory, { recursive: true, force: true })
})

describe('readMaterialized', () => {
  describe('when the file holds content', () => {
    it('should return it', async () => {
      await writeFile(path.join(directory, 'a.json'), '{}')

      expect((await readMaterialized(path.join(directory, 'a.json'))).toString()).toBe('{}')
    })
  })

  describe('when the file is a Git LFS pointer', () => {
    it('should throw an LfsPointerError naming the file', async () => {
      await writeFile(path.join(directory, 'a.json'), POINTER)

      await expect(readMaterialized(path.join(directory, 'a.json'))).rejects.toBeInstanceOf(
        LfsPointerError
      )
    })
  })
})

describe('the MCP runtime', () => {
  describe('when the saved capture is an unmaterialized LFS pointer', () => {
    it('should fail instead of silently falling back to resolve-only mode', async () => {
      await mkdir(path.join(directory, 'snapshot/components'), { recursive: true })
      await writeFile(
        path.join(directory, 'figma.config.json'),
        JSON.stringify({
          outDir: 'generated',
          snapshotDir: 'snapshot',
          pages: { 'Nav bar': { kind: 'components' } },
          contract: { mappingRoot: 'components' }
        })
      )
      await writeFile(path.join(directory, 'snapshot/components/Nav bar.json'), POINTER)
      // A pointer that fetch-source would have pinned: the cache verifies, and reading it must still fail.
      await pinTestCapture(directory)

      const runtime = await createRuntime({ cwd: directory, env: {} })

      await expect(runtime.resolveContext()).rejects.toBeInstanceOf(LfsPointerError)
    })
  })
})
